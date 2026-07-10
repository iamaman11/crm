#!/usr/bin/env python3
"""Validate module capability/event bindings against a Protobuf descriptor set."""

from __future__ import annotations

import argparse
import json
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from google.protobuf import descriptor_pb2
from google.protobuf.message import DecodeError
from ruamel.yaml import YAML

EXPECTED_SCHEMA_VERSION = "crm.contract-bindings/v1"


@dataclass(frozen=True, order=True)
class VersionedContract:
    contract_id: str
    version: str

    @classmethod
    def from_mapping(cls, value: dict[str, Any], location: str) -> "VersionedContract":
        contract_id = require_string(value, "id", location)
        version = require_string(value, "version", location)
        return cls(contract_id=contract_id, version=version)

    def display(self) -> str:
        return f"{self.contract_id}@{self.version}"


@dataclass(frozen=True)
class RpcDescriptor:
    request: str
    response: str


def require_string(value: dict[str, Any], key: str, location: str) -> str:
    candidate = value.get(key)
    if not isinstance(candidate, str) or not candidate:
        raise ValueError(f"{location}.{key} must be a non-empty string")
    return candidate


def require_list(value: dict[str, Any], key: str, location: str) -> list[Any]:
    candidate = value.get(key)
    if not isinstance(candidate, list):
        raise ValueError(f"{location}.{key} must be a list")
    return candidate


def load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read binding registry {path}: {error}") from error
    if not isinstance(value, dict):
        raise ValueError(f"binding registry {path} must contain a JSON object")
    return value


def load_descriptor_set(path: Path) -> descriptor_pb2.FileDescriptorSet:
    descriptor_set = descriptor_pb2.FileDescriptorSet()
    try:
        descriptor_set.ParseFromString(path.read_bytes())
    except (OSError, DecodeError) as error:
        raise ValueError(f"cannot read descriptor set {path}: {error}") from error
    if not descriptor_set.file:
        raise ValueError(f"descriptor set {path} is empty")
    return descriptor_set


def collect_message_names(
    package: str,
    prefix: str,
    messages: Any,
    output: set[str],
) -> None:
    for message in messages:
        local_name = f"{prefix}.{message.name}" if prefix else message.name
        qualified_name = f"{package}.{local_name}" if package else local_name
        output.add(qualified_name)
        collect_message_names(package, local_name, message.nested_type, output)


def descriptor_index(
    descriptor_set: descriptor_pb2.FileDescriptorSet,
) -> tuple[set[str], dict[str, RpcDescriptor]]:
    messages: set[str] = set()
    methods: dict[str, RpcDescriptor] = {}
    for file_descriptor in descriptor_set.file:
        package = file_descriptor.package
        collect_message_names(package, "", file_descriptor.message_type, messages)
        for service in file_descriptor.service:
            service_name = f"{package}.{service.name}" if package else service.name
            for method in service.method:
                method_name = f"{service_name}.{method.name}"
                if method_name in methods:
                    raise ValueError(f"duplicate RPC descriptor {method_name}")
                methods[method_name] = RpcDescriptor(
                    request=method.input_type.removeprefix("."),
                    response=method.output_type.removeprefix("."),
                )
    return messages, methods


def load_manifests(modules_root: Path) -> dict[str, dict[str, set[VersionedContract]]]:
    yaml = YAML(typ="safe")
    manifests: dict[str, dict[str, set[VersionedContract]]] = {}
    for path in sorted(modules_root.glob("*/module.yaml")):
        try:
            document = yaml.load(path.read_text(encoding="utf-8"))
        except Exception as error:  # ruamel exposes multiple parser exception types.
            raise ValueError(f"cannot read module manifest {path}: {error}") from error
        if not isinstance(document, dict):
            raise ValueError(f"module manifest {path} must contain a mapping")
        module_id = require_string(document, "module_id", str(path))
        if module_id in manifests:
            raise ValueError(f"duplicate module_id {module_id} in module manifests")
        provides = document.get("provides")
        if not isinstance(provides, dict):
            raise ValueError(f"{path}.provides must be a mapping")
        capabilities = {
            VersionedContract.from_mapping(item, f"{path}.provides.capabilities[{index}]")
            for index, item in enumerate(require_list(provides, "capabilities", str(path)))
            if isinstance(item, dict)
        }
        events = {
            VersionedContract.from_mapping(item, f"{path}.provides.events[{index}]")
            for index, item in enumerate(require_list(provides, "events", str(path)))
            if isinstance(item, dict)
        }
        if len(capabilities) != len(provides["capabilities"]):
            raise ValueError(f"{path}.provides.capabilities contains a non-object or duplicate")
        if len(events) != len(provides["events"]):
            raise ValueError(f"{path}.provides.events contains a non-object or duplicate")
        manifests[module_id] = {"capabilities": capabilities, "events": events}
    if not manifests:
        raise ValueError(f"no module manifests found below {modules_root}")
    return manifests


def validate(
    registry: dict[str, Any],
    manifests: dict[str, dict[str, set[VersionedContract]]],
    messages: set[str],
    methods: dict[str, RpcDescriptor],
) -> list[str]:
    errors: list[str] = []
    if registry.get("schema_version") != EXPECTED_SCHEMA_VERSION:
        errors.append(
            f"schema_version must be {EXPECTED_SCHEMA_VERSION!r}, "
            f"found {registry.get('schema_version')!r}"
        )

    try:
        module_bindings = require_list(registry, "modules", "registry")
    except ValueError as error:
        return [str(error)]

    seen_modules: set[str] = set()
    seen_capabilities: set[VersionedContract] = set()
    seen_events: set[VersionedContract] = set()

    for module_index, module_binding in enumerate(module_bindings):
        location = f"registry.modules[{module_index}]"
        if not isinstance(module_binding, dict):
            errors.append(f"{location} must be an object")
            continue
        try:
            module_id = require_string(module_binding, "module_id", location)
            capability_bindings = require_list(module_binding, "capabilities", location)
            event_bindings = require_list(module_binding, "events", location)
        except ValueError as error:
            errors.append(str(error))
            continue

        if module_id in seen_modules:
            errors.append(f"duplicate module binding {module_id}")
            continue
        seen_modules.add(module_id)
        manifest = manifests.get(module_id)
        if manifest is None:
            errors.append(f"binding references unknown module manifest {module_id}")
            continue

        bound_capabilities: set[VersionedContract] = set()
        for index, binding in enumerate(capability_bindings):
            item_location = f"{location}.capabilities[{index}]"
            if not isinstance(binding, dict):
                errors.append(f"{item_location} must be an object")
                continue
            try:
                contract = VersionedContract.from_mapping(binding, item_location)
                rpc = require_string(binding, "rpc", item_location)
                request = require_string(binding, "request", item_location)
                response = require_string(binding, "response", item_location)
            except ValueError as error:
                errors.append(str(error))
                continue

            if contract in seen_capabilities:
                errors.append(f"duplicate capability binding {contract.display()}")
            seen_capabilities.add(contract)
            bound_capabilities.add(contract)
            if request not in messages:
                errors.append(f"{contract.display()} request message does not exist: {request}")
            if response not in messages:
                errors.append(f"{contract.display()} response message does not exist: {response}")
            method = methods.get(rpc)
            if method is None:
                errors.append(f"{contract.display()} RPC does not exist: {rpc}")
            elif method != RpcDescriptor(request=request, response=response):
                errors.append(
                    f"{contract.display()} RPC {rpc} resolves to "
                    f"{method.request} -> {method.response}, expected {request} -> {response}"
                )

        bound_events: set[VersionedContract] = set()
        for index, binding in enumerate(event_bindings):
            item_location = f"{location}.events[{index}]"
            if not isinstance(binding, dict):
                errors.append(f"{item_location} must be an object")
                continue
            try:
                contract = VersionedContract.from_mapping(binding, item_location)
                message = require_string(binding, "message", item_location)
            except ValueError as error:
                errors.append(str(error))
                continue

            if contract in seen_events:
                errors.append(f"duplicate event binding {contract.display()}")
            seen_events.add(contract)
            bound_events.add(contract)
            if message not in messages:
                errors.append(f"{contract.display()} event message does not exist: {message}")

        for kind, bound in (
            ("capabilities", bound_capabilities),
            ("events", bound_events),
        ):
            declared = manifest[kind]
            for missing in sorted(declared - bound):
                errors.append(
                    f"{module_id} manifest {kind[:-1]} lacks a Protobuf binding: "
                    f"{missing.display()}"
                )
            for extra in sorted(bound - declared):
                errors.append(
                    f"{module_id} binding is not declared by the manifest: {extra.display()}"
                )

    return sorted(set(errors))


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--descriptor-set", type=Path, required=True)
    parser.add_argument(
        "--bindings",
        type=Path,
        default=Path("contracts/module-contract-bindings.json"),
    )
    parser.add_argument("--modules-root", type=Path, default=Path("modules"))
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        registry = load_json(args.bindings)
        descriptors = load_descriptor_set(args.descriptor_set)
        messages, methods = descriptor_index(descriptors)
        manifests = load_manifests(args.modules_root)
        errors = validate(registry, manifests, messages, methods)
    except ValueError as error:
        print(f"contract binding validation failed: {error}", file=sys.stderr)
        return 1

    if errors:
        print("contract binding validation failed:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    module_count = len(registry["modules"])
    capability_count = sum(len(module["capabilities"]) for module in registry["modules"])
    event_count = sum(len(module["events"]) for module in registry["modules"])
    print(
        "contract bindings valid: "
        f"{module_count} modules, {capability_count} capabilities, {event_count} events"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

"""Deterministic module-manifest to Protobuf contract binding compilation."""

from __future__ import annotations

from dataclasses import dataclass
import json
from pathlib import Path
from typing import Any, Iterable

from google.protobuf import descriptor_pb2
from google.protobuf.message import DecodeError

try:
    from scripts.validate_module_manifests import (
        DEFAULT_SCHEMA,
        ManifestError,
        load_schema,
        strict_yaml_load,
        validate_manifest_semantics,
        validate_manifest_set,
        validate_schema,
    )
except ModuleNotFoundError:  # Direct execution from the scripts directory.
    from validate_module_manifests import (
        DEFAULT_SCHEMA,
        ManifestError,
        load_schema,
        strict_yaml_load,
        validate_manifest_semantics,
        validate_manifest_set,
        validate_schema,
    )

SCHEMA_VERSION = "crm.contract-bindings/v1"


@dataclass(frozen=True, order=True)
class VersionedContract:
    contract_id: str
    version: str

    @classmethod
    def from_mapping(cls, value: dict[str, Any], location: str) -> "VersionedContract":
        return cls(
            contract_id=require_string(value, "id", location),
            version=require_string(value, "version", location),
        )

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


def collect_message_names(package: str, prefix: str, messages: Any, output: set[str]) -> None:
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


def load_descriptor_set(path: Path) -> descriptor_pb2.FileDescriptorSet:
    descriptor_set = descriptor_pb2.FileDescriptorSet()
    try:
        descriptor_set.ParseFromString(path.read_bytes())
    except (OSError, DecodeError) as error:
        raise ValueError(f"cannot read descriptor set {path}: {error}") from error
    if not descriptor_set.file:
        raise ValueError(f"descriptor set {path} is empty")
    return descriptor_set


def load_authoring_manifests(
    modules_root: Path,
    schema_path: Path = DEFAULT_SCHEMA,
) -> list[dict[str, Any]]:
    try:
        schema = load_schema(schema_path)
    except Exception as error:
        raise ValueError(f"cannot load module manifest schema {schema_path}: {error}") from error

    entries: list[tuple[Path, dict[str, Any]]] = []
    errors: list[str] = []
    for path in sorted(modules_root.glob("*/module.yaml")):
        try:
            manifest = strict_yaml_load(path.read_text(encoding="utf-8"), str(path))
        except (OSError, ManifestError) as error:
            errors.append(str(error))
            continue
        schema_errors = validate_schema(manifest, schema, str(path))
        errors.extend(schema_errors)
        if schema_errors:
            continue
        errors.extend(validate_manifest_semantics(manifest, str(path)))
        entries.append((path, manifest))

    errors.extend(validate_manifest_set(entries))
    if not entries and not errors:
        errors.append(f"no module manifests found below {modules_root}")
    if errors:
        raise ValueError("invalid module manifests:\n- " + "\n- ".join(sorted(set(errors))))
    return [manifest for _, manifest in entries]


def build_registry(
    manifests: Iterable[dict[str, Any]],
    messages: set[str],
    methods: dict[str, RpcDescriptor],
) -> tuple[dict[str, Any], list[str]]:
    errors: list[str] = []
    modules: list[dict[str, Any]] = []
    seen_modules: set[str] = set()
    seen_capabilities: set[VersionedContract] = set()
    seen_events: set[VersionedContract] = set()

    for manifest in sorted(manifests, key=lambda item: str(item.get("module_id", ""))):
        module_id = manifest.get("module_id")
        if not isinstance(module_id, str) or not module_id:
            errors.append("manifest module_id must be a non-empty string")
            continue
        if module_id in seen_modules:
            errors.append(f"duplicate module manifest {module_id}")
            continue
        seen_modules.add(module_id)

        provides = manifest.get("provides")
        if not isinstance(provides, dict):
            errors.append(f"{module_id} manifest provides must be an object")
            continue

        capabilities: list[dict[str, str]] = []
        raw_capabilities = provides.get("capabilities")
        if not isinstance(raw_capabilities, list):
            errors.append(f"{module_id} manifest capabilities must be a list")
            raw_capabilities = []
        for index, item in enumerate(raw_capabilities):
            location = f"{module_id}.provides.capabilities[{index}]"
            if not isinstance(item, dict):
                errors.append(f"{location} must be an object")
                continue
            try:
                contract = VersionedContract.from_mapping(item, location)
                binding = item.get("binding")
                if not isinstance(binding, dict):
                    raise ValueError(f"{location}.binding must be an object")
                kind = require_string(binding, "kind", f"{location}.binding")
                if kind != "protobuf_rpc":
                    raise ValueError(f"{location}.binding.kind must be 'protobuf_rpc'")
                rpc = require_string(binding, "rpc", f"{location}.binding")
                request = require_string(binding, "request", f"{location}.binding")
                response = require_string(binding, "response", f"{location}.binding")
            except ValueError as error:
                errors.append(str(error))
                continue
            if contract in seen_capabilities:
                errors.append(f"duplicate capability provider {contract.display()}")
            seen_capabilities.add(contract)
            if request not in messages:
                errors.append(f"{contract.display()} request message does not exist: {request}")
            if response not in messages:
                errors.append(f"{contract.display()} response message does not exist: {response}")
            method = methods.get(rpc)
            expected = RpcDescriptor(request=request, response=response)
            if method is None:
                errors.append(f"{contract.display()} RPC does not exist: {rpc}")
            elif method != expected:
                errors.append(
                    f"{contract.display()} RPC {rpc} resolves to {method.request} -> {method.response}, "
                    f"expected {request} -> {response}"
                )
            capabilities.append(
                {"id": contract.contract_id, "version": contract.version, "rpc": rpc, "request": request, "response": response}
            )

        events: list[dict[str, str]] = []
        raw_events = provides.get("events")
        if not isinstance(raw_events, list):
            errors.append(f"{module_id} manifest events must be a list")
            raw_events = []
        for index, item in enumerate(raw_events):
            location = f"{module_id}.provides.events[{index}]"
            if not isinstance(item, dict):
                errors.append(f"{location} must be an object")
                continue
            try:
                contract = VersionedContract.from_mapping(item, location)
                binding = item.get("binding")
                if not isinstance(binding, dict):
                    raise ValueError(f"{location}.binding must be an object")
                kind = require_string(binding, "kind", f"{location}.binding")
                if kind != "protobuf_message":
                    raise ValueError(f"{location}.binding.kind must be 'protobuf_message'")
                message = require_string(binding, "message", f"{location}.binding")
            except ValueError as error:
                errors.append(str(error))
                continue
            if contract in seen_events:
                errors.append(f"duplicate event provider {contract.display()}")
            seen_events.add(contract)
            if message not in messages:
                errors.append(f"{contract.display()} event message does not exist: {message}")
            events.append({"id": contract.contract_id, "version": contract.version, "message": message})

        capabilities.sort(key=lambda item: (item["id"], item["version"]))
        events.sort(key=lambda item: (item["id"], item["version"]))
        modules.append({"module_id": module_id, "capabilities": capabilities, "events": events})

    registry = {"schema_version": SCHEMA_VERSION, "modules": modules}
    return registry, sorted(set(errors))


def render_registry(registry: dict[str, Any]) -> bytes:
    return (json.dumps(registry, ensure_ascii=False, indent=2) + "\n").encode("utf-8")


def registry_counts(registry: dict[str, Any]) -> tuple[int, int, int]:
    modules = registry["modules"]
    return (
        len(modules),
        sum(len(module["capabilities"]) for module in modules),
        sum(len(module["events"]) for module in modules),
    )

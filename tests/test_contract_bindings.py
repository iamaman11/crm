from __future__ import annotations

from google.protobuf import descriptor_pb2
import unittest

from scripts.contract_bindings import RpcDescriptor, build_registry, descriptor_index, render_registry
from scripts.module_manifest_projection import runtime_manifest_projection


def capability(contract_id: str, rpc: str, request: str, response: str) -> dict:
    return {
        "id": contract_id,
        "version": "1.0.0",
        "binding": {
            "kind": "protobuf_rpc",
            "rpc": rpc,
            "request": request,
            "response": response,
        },
    }


def event(contract_id: str, message: str) -> dict:
    return {
        "id": contract_id,
        "version": "1.0.0",
        "binding": {"kind": "protobuf_message", "message": message},
    }


class ContractBindingTests(unittest.TestCase):
    def descriptor(self) -> descriptor_pb2.FileDescriptorSet:
        descriptor_set = descriptor_pb2.FileDescriptorSet()
        file_descriptor = descriptor_set.file.add()
        file_descriptor.name = "crm/example/v1/example.proto"
        file_descriptor.package = "crm.example.v1"
        file_descriptor.message_type.add(name="CreateExampleRequest")
        file_descriptor.message_type.add(name="CreateExampleResponse")
        file_descriptor.message_type.add(name="ExampleCreatedEvent")
        service = file_descriptor.service.add(name="ExampleService")
        method = service.method.add(name="CreateExample")
        method.input_type = ".crm.example.v1.CreateExampleRequest"
        method.output_type = ".crm.example.v1.CreateExampleResponse"
        return descriptor_set

    def manifests(self) -> list[dict]:
        return [
            {
                "module_id": "crm.empty-link",
                "provides": {"capabilities": [], "events": []},
            },
            {
                "module_id": "crm.example",
                "provides": {
                    "capabilities": [
                        capability(
                            "example.create",
                            "crm.example.v1.ExampleService.CreateExample",
                            "crm.example.v1.CreateExampleRequest",
                            "crm.example.v1.CreateExampleResponse",
                        )
                    ],
                    "events": [event("example.created", "crm.example.v1.ExampleCreatedEvent")],
                },
            },
        ]

    def test_registry_is_complete_sorted_and_includes_empty_modules(self) -> None:
        messages, methods = descriptor_index(self.descriptor())
        registry, errors = build_registry(reversed(self.manifests()), messages, methods)
        self.assertEqual(errors, [])
        self.assertEqual([module["module_id"] for module in registry["modules"]], ["crm.empty-link", "crm.example"])
        self.assertEqual(registry["modules"][0]["capabilities"], [])
        self.assertTrue(render_registry(registry).endswith(b"\n"))

    def test_descriptor_input_output_drift_fails(self) -> None:
        messages, methods = descriptor_index(self.descriptor())
        methods["crm.example.v1.ExampleService.CreateExample"] = RpcDescriptor(
            request="crm.example.v1.CreateExampleResponse",
            response="crm.example.v1.CreateExampleRequest",
        )
        _, errors = build_registry(self.manifests(), messages, methods)
        self.assertTrue(any("resolves to" in error for error in errors))

    def test_runtime_projection_removes_only_build_time_bindings(self) -> None:
        authoring = self.manifests()[1]
        projected = runtime_manifest_projection(authoring)
        self.assertNotIn("binding", projected["provides"]["capabilities"][0])
        self.assertNotIn("binding", projected["provides"]["events"][0])
        self.assertIn("binding", authoring["provides"]["capabilities"][0])
        self.assertEqual(projected["provides"]["capabilities"][0]["id"], "example.create")


if __name__ == "__main__":
    unittest.main()

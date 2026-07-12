import { describe, expect, it } from "vitest";
import {
  createObjectMetadataDefinitionInput,
  MetadataAuthoringError,
  METADATA_DEFINITION_SCHEMA_VERSION,
} from "./metadataAuthoring";

const textDecoder = new TextDecoder();

describe("createObjectMetadataDefinitionInput", () => {
  it("emits the strict v1 object document shape with deterministic tag ordering", () => {
    const input = createObjectMetadataDefinitionInput({
      id: "crm.custom.asset",
      ownerModuleId: "crm.custom",
      label: "Asset",
      pluralLabel: "Assets",
      description: "Customer-owned asset",
      tags: ["operations", "customer"],
    });

    expect(input.schemaVersion).toBe(METADATA_DEFINITION_SCHEMA_VERSION);
    expect(JSON.parse(textDecoder.decode(input.definitionJson))).toEqual({
      kind: "object",
      definition: {
        id: "crm.custom.asset",
        owner_module_id: "crm.custom",
        label: "Asset",
        plural_label: "Assets",
        description: "Customer-owned asset",
        tags: ["customer", "operations"],
      },
    });
  });

  it("normalizes an empty description to the backend schema null representation", () => {
    const input = createObjectMetadataDefinitionInput({
      id: "crm.custom.asset",
      ownerModuleId: "crm.custom",
      label: "Asset",
      pluralLabel: "Assets",
      description: "",
      tags: [],
    });

    expect(JSON.parse(textDecoder.decode(input.definitionJson))).toMatchObject({
      definition: { description: null },
    });
  });

  it("rejects duplicate set-like tags rather than silently changing author intent", () => {
    expect(() =>
      createObjectMetadataDefinitionInput({
        id: "crm.custom.asset",
        ownerModuleId: "crm.custom",
        label: "Asset",
        pluralLabel: "Assets",
        description: "",
        tags: ["customer", "customer"],
      }),
    ).toThrowError(
      expect.objectContaining({
        name: "MetadataAuthoringError",
        field: "tags",
        safeCode: "DUPLICATE_TAG",
      }) satisfies Partial<MetadataAuthoringError>,
    );
  });

  it("rejects control characters and oversized UTF-8 labels before transport", () => {
    expect(() =>
      createObjectMetadataDefinitionInput({
        id: "crm.custom.asset",
        ownerModuleId: "crm.custom",
        label: "Asset\nInjected",
        pluralLabel: "Assets",
        description: "",
        tags: [],
      }),
    ).toThrowError(
      expect.objectContaining({
        field: "label",
        safeCode: "INVALID_LABEL",
      }) satisfies Partial<MetadataAuthoringError>,
    );

    expect(() =>
      createObjectMetadataDefinitionInput({
        id: "crm.custom.asset",
        ownerModuleId: "crm.custom",
        label: "Ж".repeat(101),
        pluralLabel: "Assets",
        description: "",
        tags: [],
      }),
    ).toThrowError(
      expect.objectContaining({
        field: "label",
        safeCode: "INVALID_LABEL",
      }) satisfies Partial<MetadataAuthoringError>,
    );
  });
});

import { create } from "@bufbuild/protobuf";
import {
  MetadataDefinitionInputSchema,
  type MetadataDefinitionInput,
} from "../gen/crm/metadata/v1/metadata_pb";

export const METADATA_DEFINITION_SCHEMA_VERSION = "crm.metadata.definition/v1";

const MAX_IDENTIFIER_BYTES = 180;
const MAX_LABEL_BYTES = 200;
const MAX_DESCRIPTION_BYTES = 4_000;
const MAX_COLLECTION_MEMBERS = 2_000;

const textEncoder = new TextEncoder();

export interface ObjectMetadataDraft {
  id: string;
  ownerModuleId: string;
  label: string;
  pluralLabel: string;
  description: string;
  tags: readonly string[];
}

export type MetadataAuthoringField =
  | "id"
  | "ownerModuleId"
  | "label"
  | "pluralLabel"
  | "description"
  | "tags";

export class MetadataAuthoringError extends Error {
  public readonly field: MetadataAuthoringField;
  public readonly safeCode: string;

  public constructor(
    field: MetadataAuthoringField,
    safeCode: string,
    message: string,
  ) {
    super(message);
    this.name = "MetadataAuthoringError";
    this.field = field;
    this.safeCode = safeCode;
  }
}

export function createObjectMetadataDefinitionInput(
  draft: ObjectMetadataDraft,
): MetadataDefinitionInput {
  const id = requireIdentifier(draft.id, "id", "Object ID");
  const ownerModuleId = requireIdentifier(
    draft.ownerModuleId,
    "ownerModuleId",
    "Owner module ID",
  );
  const label = requireLabel(draft.label, "label", "Label");
  const pluralLabel = requireLabel(
    draft.pluralLabel,
    "pluralLabel",
    "Plural label",
  );
  const description = normalizeDescription(draft.description);
  const tags = normalizeTags(draft.tags);

  const definition = {
    kind: "object",
    definition: {
      id,
      owner_module_id: ownerModuleId,
      label,
      plural_label: pluralLabel,
      description,
      tags,
    },
  } as const;

  return create(MetadataDefinitionInputSchema, {
    schemaVersion: METADATA_DEFINITION_SCHEMA_VERSION,
    definitionJson: textEncoder.encode(JSON.stringify(definition)),
  });
}

function requireIdentifier(
  value: string,
  field: Extract<MetadataAuthoringField, "id" | "ownerModuleId">,
  label: string,
): string {
  if (
    value.length === 0 ||
    textEncoder.encode(value).length > MAX_IDENTIFIER_BYTES ||
    containsControlCharacter(value)
  ) {
    throw new MetadataAuthoringError(
      field,
      "INVALID_IDENTIFIER",
      `${label} must be non-empty, control-free, and at most ${MAX_IDENTIFIER_BYTES} UTF-8 bytes.`,
    );
  }
  return value;
}

function requireLabel(
  value: string,
  field: Extract<MetadataAuthoringField, "label" | "pluralLabel">,
  label: string,
): string {
  if (
    value.trim().length === 0 ||
    textEncoder.encode(value).length > MAX_LABEL_BYTES ||
    containsControlCharacter(value)
  ) {
    throw new MetadataAuthoringError(
      field,
      "INVALID_LABEL",
      `${label} must be non-empty, control-free, and at most ${MAX_LABEL_BYTES} UTF-8 bytes.`,
    );
  }
  return value;
}

function normalizeDescription(value: string): string | null {
  if (
    textEncoder.encode(value).length > MAX_DESCRIPTION_BYTES ||
    containsControlCharacter(value)
  ) {
    throw new MetadataAuthoringError(
      "description",
      "INVALID_DESCRIPTION",
      `Description must be control-free and at most ${MAX_DESCRIPTION_BYTES} UTF-8 bytes.`,
    );
  }
  return value.length === 0 ? null : value;
}

function normalizeTags(values: readonly string[]): string[] {
  if (values.length > MAX_COLLECTION_MEMBERS) {
    throw new MetadataAuthoringError(
      "tags",
      "TOO_MANY_TAGS",
      `Tags must not contain more than ${MAX_COLLECTION_MEMBERS} members.`,
    );
  }

  const seen = new Set<string>();
  const normalized: string[] = [];
  for (const value of values) {
    if (value.trim().length === 0 || containsControlCharacter(value)) {
      throw new MetadataAuthoringError(
        "tags",
        "INVALID_TAG",
        "Tags must be non-empty and control-free.",
      );
    }
    if (seen.has(value)) {
      throw new MetadataAuthoringError(
        "tags",
        "DUPLICATE_TAG",
        `Tag “${value}” is duplicated.`,
      );
    }
    seen.add(value);
    normalized.push(value);
  }

  return normalized.sort((left, right) =>
    left < right ? -1 : left > right ? 1 : 0,
  );
}

function containsControlCharacter(value: string): boolean {
  for (const character of value) {
    const codePoint = character.codePointAt(0);
    if (codePoint !== undefined && (codePoint <= 0x1f || codePoint === 0x7f)) {
      return true;
    }
  }
  return false;
}

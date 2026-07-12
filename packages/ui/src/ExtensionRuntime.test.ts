import { describe, expect, it } from "vitest";
import {
  UiExtensionRegistrationError,
  createUiExtensionRegistry,
  uiExtensionCoordinate,
  type UiExtensionDefinition,
} from "./ExtensionRuntime";

interface TestContext {
  recordId: string;
}

const load = async () => ({ default: () => null });

function extension(
  overrides: Partial<UiExtensionDefinition<TestContext>> = {},
): UiExtensionDefinition<TestContext> {
  return {
    id: "deal.health",
    ownerModuleId: "crm.sales",
    surface: "record.detail.sidebar",
    order: 0,
    load,
    ...overrides,
  };
}

describe("UiExtensionRegistry", () => {
  it("filters by surface and sorts deterministically by order then coordinate", () => {
    const registry = createUiExtensionRegistry<TestContext>([
      extension({ id: "deal.timeline", order: 20 }),
      extension({ id: "deal.health", order: 10 }),
      extension({
        id: "deal.summary",
        surface: "record.detail.main",
        order: 5,
      }),
      extension({ id: "deal.actions", order: 10 }),
    ]);

    const sidebar = registry.forSurface("record.detail.sidebar");
    expect(sidebar.map((definition) => definition.id)).toEqual([
      "deal.actions",
      "deal.health",
      "deal.timeline",
    ]);
    expect(registry.forSurface("record.detail.main")).toHaveLength(1);
  });

  it("rejects duplicate exact extension coordinates", () => {
    expect(() =>
      createUiExtensionRegistry<TestContext>([
        extension(),
        extension(),
      ]),
    ).toThrowError(UiExtensionRegistrationError);

    try {
      createUiExtensionRegistry<TestContext>([extension(), extension()]);
    } catch (error) {
      expect(error).toBeInstanceOf(UiExtensionRegistrationError);
      expect((error as UiExtensionRegistrationError).code).toBe(
        "DUPLICATE_COORDINATE",
      );
    }
  });

  it("rejects invalid identifiers, unsupported surfaces, and unsafe order values", () => {
    const invalidCases = [
      extension({ id: "Deal Health" }),
      extension({ ownerModuleId: "CRM Sales" }),
      extension({ surface: "record.unknown" as "record.detail.sidebar" }),
      extension({ order: 10_001 }),
    ];

    for (const definition of invalidCases) {
      expect(() => createUiExtensionRegistry([definition])).toThrowError(
        UiExtensionRegistrationError,
      );
    }
  });

  it("builds a stable safe coordinate from owner, extension id, and surface", () => {
    expect(uiExtensionCoordinate(extension())).toBe(
      "crm.sales:deal.health@record.detail.sidebar",
    );
  });
});

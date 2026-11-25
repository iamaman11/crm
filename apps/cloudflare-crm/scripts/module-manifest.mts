// Authoring YAML is data, not an executable policy or another type registry.
import assert from "node:assert/strict";
import {createHash} from "node:crypto";
import {readFileSync, globSync, realpathSync} from "node:fs";
import {isAbsolute, relative, resolve, sep} from "node:path";
import {parseDocument, visit, isAlias, isMap, isPair, isScalar, isSeq} from "yaml";
import {Ajv2020} from "ajv/dist/2020.js";
import formatsModule from "ajv-formats";
import {record, records, string, strings} from "./json-boundary.mts";

type Json = null | boolean | string | number | Json[] | {[key: string]: Json};

export function strictManifestYaml(source: string, path = "<memory>"): {[key: string]: Json} {
  const doc = parseDocument(source, {version: "1.2", schema: "core",
    strict: true, uniqueKeys: true, merge: false, intAsBigInt: true, customTags: ["timestamp"]});
  assert(!doc.errors.length && !doc.warnings.length,
    path + ": invalid strict YAML: " + [...doc.errors, ...doc.warnings].map(e => e.message).join("; "));
  assert(doc.directives?.yaml.version === "1.2", path + ": expected YAML 1.2");
  assert(isMap(doc.contents), path + ": manifest root must be an object");
  visit(doc, (_key, node) => {
    assert(!isAlias(node), path + ": YAML anchors and aliases are forbidden");
    if (isMap(node) || isSeq(node) || isScalar(node)) {
      assert(!node.anchor, path + ": YAML anchors and aliases are forbidden");
      assert(!node.tag, path + ": YAML tags are forbidden");
    }
    if (isPair(node)) {
      assert(isScalar(node.key) && typeof node.key.value === "string",
        path + ": non-string object key");
      assert(/^[\x00-\x7F]*$/.test(node.key.value), path + ": canonical object keys must be ASCII");
    }
    if (isScalar(node)) assert(node.value !== "<<", path + ": YAML merge keys are forbidden");
  });
  function normalize(value: unknown): Json {
    if (value === null || typeof value === "string" || typeof value === "boolean") return value;
    if (typeof value === "bigint") {
      assert(value >= BigInt(Number.MIN_SAFE_INTEGER) && value <= BigInt(Number.MAX_SAFE_INTEGER),
        path + ": integer outside safe canonical range");
      return Number(value);
    }
    // The native parser uses bigint for integer tokens, number for floating
    // tokens; 1.0 and 1e0 must not silently become accepted integers.
    if (typeof value === "number") throw new Error(path + ": floating-point values are forbidden");
    if (value instanceof Date) throw new Error(path + ": implicit date/time values are forbidden; quote them");
    if (Array.isArray(value)) return value.map(normalize);
    if (value instanceof Map) return Object.fromEntries([...value].map(([k, v]) => [string(k), normalize(v)]));
    throw new Error(path + ": unsupported YAML value");
  }
  const value = normalize(doc.toJS({mapAsMap: true, maxAliasCount: 0}));
  assert(value !== null && typeof value === "object" && !Array.isArray(value), path + ": expected object");
  return value;
}

// crm.cjson/v1 sorts ASCII mapping keys, including numeric-looking keys.
// JSON.stringify(object) alone would reorder integer-index keys differently.
export function canonicalJson(value: Json): string {
  if (value === null || typeof value !== "object") return JSON.stringify(value);
  if (Array.isArray(value)) return "[" + value.map(canonicalJson).join(",") + "]";
  return "{" + Object.entries(value).sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0)
    .map(([key, item]) => JSON.stringify(key) + ":" + canonicalJson(item)).join(",") + "}";
}
export function manifestDigest(value: Json): string {
  return createHash("sha256").update(canonicalJson(value)).digest("hex");
}

function unique(values: string[], label: string): void {
  assert.equal(new Set(values).size, values.length, "duplicate " + label);
}
const coordinate = (item: Record<string, unknown>) => JSON.stringify([string(item.id), string(item.version)]);

export function validateManifestSemantics(value: unknown): void {
  const manifest = record(value);
  const id = string(manifest.module_id);
  const deps = record(manifest.dependencies);
  const required = records(deps.required).map(dep => string(dep.module_id));
  const optional = records(deps.optional).map(dep => string(dep.module_id));
  const conflicts = strings(deps.conflicts);
  unique(required, "required dependencies");
  unique(optional, "optional dependencies");
  unique(conflicts, "conflicts");
  assert(![...required, ...optional, ...conflicts].includes(id), "module must not depend on or conflict with itself");
  assert(!required.some(dep => optional.includes(dep)), "dependencies cannot be both required and optional");
  const provides = record(manifest.provides);
  for (const category of ["capabilities", "events"]) unique(records(provides[category]).map(coordinate), "provided " + category);
  const objects = new Set(strings(provides.objects));
  const storage = record(manifest.storage);
  const recordTypes = new Set(strings(storage.record_types));
  assert([...recordTypes].every(type => objects.has(type)), "storage record types not owned by module");
  assert(strings(record(manifest.lifecycle).retained_record_types).every(type => recordTypes.has(type)),
    "retained record types are not declared storage");
  assert(strings(storage.private_state_namespaces).every(namespace => namespace.startsWith(id)),
    "private state namespace must start with module_id");
}

export function validateManifestSet(entries: readonly {path: string; manifest: unknown}[]): void {
  assert(entries.length > 0, "no canonical module manifests");
  const byId = new Map<string, string[]>();
  const providers = new Set<string>();
  const objects = new Set<string>();
  for (const {path, manifest: value} of entries) {
    const manifest = record(value), id = string(manifest.module_id);
    assert(!byId.has(id), path + ": duplicate module_id " + id);
    byId.set(id, records(record(manifest.dependencies).required).map(dep => string(dep.module_id)));
    const provides = record(manifest.provides);
    for (const category of ["capabilities", "events"]) {
      for (const item of records(provides[category])) {
        const key = category + coordinate(item);
        assert(!providers.has(key), path + ": contract already provided " + key);
        providers.add(key);
      }
    }
    for (const object of strings(provides.objects)) {
      assert(!objects.has(object), path + ": object already owned " + object);
      objects.add(object);
    }
  }
  const visiting = new Set<string>(), visited = new Set<string>();
  function visitRequired(id: string): void {
    assert(!visiting.has(id), "required module dependency cycle: " + [...visiting, id].join(" -> "));
    if (visited.has(id)) return;
    const deps = byId.get(id);
    assert(deps, "required module missing: " + id);
    visiting.add(id);
    for (const dep of deps) visitRequired(dep);
    visiting.delete(id);
    visited.add(id);
  }
  for (const id of [...byId.keys()].sort()) visitRequired(id);
}

export function loadManifestValidator(schema: unknown) {
  // Standard Draft 2020-12 validator; the existing schema remains authoritative.
  const ajv = new Ajv2020({strict: true, allErrors: true, ownProperties: true, validateFormats: true});
  formatsModule.default(ajv);
  const validate = ajv.compile(record(schema));
  return (value: unknown, path: string): void => {
    assert(validate(value), path + ": " + ajv.errorsText(validate.errors, {separator: "; "}));
    validateManifestSemantics(value);
  };
}

export function insideRepo(root: string, path: string): string {
  const target = realpathSync(resolve(root, path));
  const rel = relative(root, target);
  assert(!isAbsolute(rel) && rel !== ".." && !rel.startsWith(".." + sep), "module path outside repository: " + path);
  return target;
}

export function loadCanonicalManifests(repository: string) {
  const root = realpathSync(repository);
  const schema: unknown = JSON.parse(readFileSync(resolve(root, "schemas/module.schema.json"), "utf8"));
  const validate = loadManifestValidator(schema);
  const paths = globSync("modules/*/module.yaml", {cwd: root}).sort();
  const entries = paths.map(path => {
    const manifest = strictManifestYaml(readFileSync(insideRepo(root, path), "utf8"), path);
    validate(manifest, path);
    return {path, manifest};
  });
  validateManifestSet(entries);
  return entries;
}

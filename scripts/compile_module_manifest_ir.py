#!/usr/bin/env python3
"""Compile governed module authoring YAML into normalized runtime JSON IR."""

from __future__ import annotations

import argparse
from pathlib import Path
import sys

from module_manifest_projection import runtime_manifest_projection
from validate_module_manifests import (
    DEFAULT_SCHEMA,
    ROOT,
    ManifestError,
    canonical_digest,
    canonical_json_bytes,
    discover_paths,
    load_schema,
    strict_yaml_load,
    validate_manifest_semantics,
    validate_manifest_set,
    validate_schema,
)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("paths", nargs="*", help="module.yaml paths; defaults to modules/*/module.yaml")
    parser.add_argument("--schema", type=Path, default=DEFAULT_SCHEMA)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args(argv)

    paths = discover_paths(args.paths)
    if not paths:
        print("No module manifests found", file=sys.stderr)
        return 2

    try:
        schema = load_schema(args.schema)
    except Exception as exc:
        print(f"Schema load failed: {exc}", file=sys.stderr)
        return 2

    entries: list[tuple[Path, dict]] = []
    errors: list[str] = []
    for path in paths:
        try:
            manifest = strict_yaml_load(path.read_text(encoding="utf-8"), str(path))
        except (OSError, ManifestError) as exc:
            errors.append(str(exc))
            continue
        schema_errors = validate_schema(manifest, schema, str(path))
        errors.extend(schema_errors)
        if schema_errors:
            continue
        errors.extend(validate_manifest_semantics(manifest, str(path)))
        entries.append((path, manifest))

    errors.extend(validate_manifest_set(entries))
    if errors:
        print("Module manifest compilation FAILED:")
        for error in errors:
            print(f"- {error}")
        return 1

    output_dir = args.output_dir.resolve()
    output_dir.mkdir(parents=True, exist_ok=True)
    for path, authoring_manifest in entries:
        runtime_manifest = runtime_manifest_projection(authoring_manifest)
        module_id = runtime_manifest["module_id"]
        json_path = output_dir / f"{module_id}.json"
        digest_path = output_dir / f"{module_id}.sha256"
        json_path.write_bytes(canonical_json_bytes(runtime_manifest))
        digest_path.write_text(canonical_digest(runtime_manifest) + "\n", encoding="ascii")
        relative_source = path.relative_to(ROOT) if path.is_relative_to(ROOT) else path
        print(f"WROTE {json_path} and {digest_path} from {relative_source}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())

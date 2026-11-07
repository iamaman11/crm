#!/usr/bin/env python3
"""Record exact installed source, local OCI image IDs and migration journal digest."""
from __future__ import annotations
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
HERE = ROOT / "deploy"

def run(*args: str) -> str:
    result = subprocess.run(args, text=True, capture_output=True, check=False)
    if result.returncode:
        raise RuntimeError("Evidence command failed")  # never echo private Compose env
    return result.stdout.strip()

def compose(*args: str) -> str:
    return run("docker", "compose", "--env-file", str(HERE / ".env"),
               "-f", str(HERE / "compose.yaml"), *args)

def main() -> None:
    p = argparse.ArgumentParser()
    p.add_argument("--output", required=True, type=Path)
    out = p.parse_args().output.expanduser().resolve()
    if not (HERE / ".env").is_file():
        raise ValueError("Missing deploy/.env")
    source = run("git", "-C", str(ROOT), "rev-parse", "HEAD")
    if not re.fullmatch(r"[0-9a-f]{40}", source):
        raise ValueError("Unverifiable source revision")
    if run("git", "-C", str(ROOT), "status", "--porcelain", "--untracked-files=no"):
        raise ValueError("Dirty tracked source")
    images = {}
    for service in ("database", "api", "web"):
        container = compose("ps", "-q", service)
        if not container:
            raise ValueError("Expected installed service unavailable")
        identity = run("docker", "inspect", "--format", "{{.Image}}", container)
        if not re.fullmatch(r"sha256:[0-9a-f]{64}", identity):
            raise ValueError("Missing immutable local image ID")
        digests = json.loads(run("docker", "image", "inspect",
                                "--format", "{{json .RepoDigests}}", identity) or "[]")
        images[service] = {"local_oci_image_id": identity,
                           "registry_repo_digests": digests or []}
    records = compose("exec", "-T", "database", "psql", "-X", "-qAt",
                      "-v", "ON_ERROR_STOP=1", "-U", "postgres", "-d", "crm",
                      "-c", "SELECT file_name || '|' || sha256 FROM public.crm_schema_migrations ORDER BY file_name")
    rows = records.splitlines()
    if not rows or any(not re.fullmatch(r"[A-Za-z0-9_.-]+\\|[0-9a-f]{64}", row) for row in rows):
        raise ValueError("Invalid schema migration journal")
    data = {
        "schema": "crm.g3-installation-evidence/v1",
        "generated_utc": datetime.now(timezone.utc).isoformat(),
        "git_commit": source,
        "compose_project": os.environ.get("COMPOSE_PROJECT_NAME", "crm-pilot"),
        "images": images,
        "migration_count": len(rows),
        "migration_journal_sha256": hashlib.sha256(("\n".join(rows) + "\n").encode()).hexdigest(),
        "caveat": "Local image IDs are not registry pull digests; real public TLS and prior binary rollback need separate proof."
    }
    if not out.parent.is_dir() or out.exists():
        raise ValueError("Output file must be new in an existing private directory")
    fd = os.open(out, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    with os.fdopen(fd, "w", encoding="utf-8") as handle:
        json.dump(data, handle, sort_keys=True, indent=2)
        handle.write("\n")
    print(f"PASS: private installation evidence at {out}")

if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, RuntimeError, json.JSONDecodeError) as error:
        print(f"Installation evidence failed: {error}", file=sys.stderr)
        sys.exit(1)

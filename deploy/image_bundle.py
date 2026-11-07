#!/usr/bin/env python3
"""Export or verify the exact five Compose service images as a portable offline bundle.

Usage:
  python3 deploy/image_bundle.py export --directory /private/crm-bundle
  python3 deploy/image_bundle.py verify --directory /private/crm-bundle

The bundle contains no .env or database backup. CI uses the same bundle to
start an independently restored Compose stack with --no-build.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
DEPLOY = ROOT / "deploy"
SERVICES = ("database", "migrate", "provision", "api", "web")
HEX_SHA = re.compile(r"[0-9a-f]{40}\Z")
IMAGE_SHA = re.compile(r"sha256:[0-9a-f]{64}\Z")


def run(*args: str) -> str:
    proc = subprocess.run(args, capture_output=True, text=True, check=False)
    if proc.returncode != 0:
        # Compose subprocess errors may expose secrets: never echo their stderr.
        raise RuntimeError("Docker or Git command failed")
    return proc.stdout.strip()


def compose(*args: str) -> str:
    return run("docker", "compose", "--env-file", str(DEPLOY / ".env"),
               "-f", str(DEPLOY / "compose.yaml"), *args)


def source_revision() -> str:
    sha = run("git", "-C", str(ROOT), "rev-parse", "HEAD")
    if not HEX_SHA.fullmatch(sha):
        raise ValueError("Unverifiable Git revision")
    if run("git", "-C", str(ROOT), "status", "--porcelain", "--untracked-files=no"):
        raise ValueError("Tracked source files are modified")
    return sha


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def pinned_yaml(sha: str) -> str:
    return "services:\n" + "".join(
        f"  {service}:\n    image: crm-pilot/{service}:{sha}\n"
        for service in SERVICES
    )


def export(directory: Path) -> None:
    if not (DEPLOY / ".env").is_file():
        raise ValueError("Missing private deploy/.env")
    source = source_revision()
    if directory.exists():
        raise ValueError("Export directory must not exist")
    directory.mkdir(mode=0o700, parents=False)
    images: dict[str, str] = {}
    tags = []
    for service in SERVICES:
        container_ids = compose("ps", "--all", "--quiet", service).splitlines()
        if len(container_ids) != 1:
            raise ValueError(f"Expected one installed {service} container")
        image_id = run("docker", "inspect", "--format", "{{.Image}}", container_ids[0])
        if not IMAGE_SHA.fullmatch(image_id):
            raise ValueError(f"Bad local OCI image identity: {service}")
        tag = f"crm-pilot/{service}:{source}"
        run("docker", "image", "tag", image_id, tag)
        images[service] = image_id
        tags.append(tag)

    archive = directory / "images.tar"
    with archive.open("xb") as output:
        os.chmod(archive, 0o600)
        result = subprocess.run(["docker", "save", *tags], stdout=output,
                                stderr=subprocess.DEVNULL, check=False)
    if result.returncode != 0 or archive.stat().st_size == 0:
        raise RuntimeError("Docker image archive creation failed")
    evidence = {
        "schema": "crm.g3-portable-images/v1",
        "git_commit": source,
        "image_ids": images,
        "images_archive_sha256": sha256_file(archive),
        "note": "Local OCI IDs, not remote registry digests. Preserve the verified tar bundle.",
    }
    (directory / "manifest.json").write_text(
        json.dumps(evidence, sort_keys=True, indent=2) + "\n", encoding="utf-8"
    )
    os.chmod(directory / "manifest.json", 0o600)
    (directory / "compose.images.yaml").write_text(pinned_yaml(source), encoding="utf-8")
    os.chmod(directory / "compose.images.yaml", 0o600)
    print(f"PASS: immutable, SHA-256 checked five-image bundle: {directory}")


def verify(directory: Path) -> None:
    manifest = json.loads((directory / "manifest.json").read_text(encoding="utf-8"))
    if manifest.get("schema") != "crm.g3-portable-images/v1":
        raise ValueError("Unknown image bundle format")
    sha = manifest.get("git_commit")
    if not isinstance(sha, str) or not HEX_SHA.fullmatch(sha):
        raise ValueError("Invalid source Git revision")
    if source_revision() != sha:
        raise ValueError("Bundle source differs from checkout; refuse mixed-version install")
    images = manifest.get("image_ids")
    if not isinstance(images, dict) or set(images) != set(SERVICES):
        raise ValueError("Image list must match exact Compose services")
    if any(not isinstance(value, str) or not IMAGE_SHA.fullmatch(value)
           for value in images.values()):
        raise ValueError("Invalid immutable image identity")
    if (directory / "compose.images.yaml").read_text(encoding="utf-8") != pinned_yaml(sha):
        raise ValueError("Pinned Compose override was changed")
    digest = manifest.get("images_archive_sha256")
    if not isinstance(digest, str) or not re.fullmatch(r"[0-9a-f]{64}", digest):
        raise ValueError("Invalid archive checksum")
    archive = directory / "images.tar"
    if sha256_file(archive) != digest:
        raise ValueError("Image archive checksum mismatch")
    run("docker", "load", "--input", str(archive))
    for service in SERVICES:
        actual = run("docker", "image", "inspect", "--format", "{{.Id}}",
                     f"crm-pilot/{service}:{sha}")
        if actual != images[service]:
            raise ValueError(f"Imported {service} digest mismatch")
    print(f"PASS: imported five pinned service images from verified source {sha}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("export", "verify"))
    parser.add_argument("--directory", required=True, type=Path)
    args = parser.parse_args()
    directory = args.directory.expanduser().resolve()
    if args.action == "export":
        export(directory)
    else:
        verify(directory)


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, ValueError, KeyError, json.JSONDecodeError) as exc:
        print(f"Image bundle acceptance failed: {exc}", file=sys.stderr)
        sys.exit(1)

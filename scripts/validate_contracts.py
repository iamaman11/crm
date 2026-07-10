#!/usr/bin/env python3
from pathlib import Path
import hashlib, subprocess, sys, tempfile

root = Path(__file__).resolve().parents[1]
proto_root = root / "proto"
files = sorted(str(p.relative_to(proto_root)) for p in proto_root.rglob("*.proto"))
if not files:
    raise SystemExit("No .proto files found")
with tempfile.TemporaryDirectory() as tmp:
    descriptor = Path(tmp) / "contracts.pb"
    command = [sys.executable, "-m", "grpc_tools.protoc", f"-I{proto_root}", f"--descriptor_set_out={descriptor}", "--include_imports", *files]
    result = subprocess.run(command, cwd=proto_root, text=True)
    if result.returncode:
        raise SystemExit(result.returncode)
    digest = hashlib.sha256(descriptor.read_bytes()).hexdigest()
print(f"Contract compile PASS: {len(files)} files")
print(f"Descriptor SHA-256: {digest}")

"""Run the host-side owner tests and record the exact dependency features."""
import json
import os
import subprocess
import tomllib
from pathlib import Path
report = json.loads(Path(os.environ["CRM_MODULE_COVERAGE_REPORT"]).read_text(encoding="utf-8"))
worker = Path("apps/cloudflare-crm/rust-party")
cargo = tomllib.loads((worker / "Cargo.toml").read_text(encoding="utf-8"))
by_path = {(worker / dep["path"]).resolve(): dep
           for dep in cargo["dependencies"].values()
           if isinstance(dep, dict) and "path" in dep}
linked = [module for module in report["modules"] if module["linked"]]
if not linked:
    raise SystemExit("No linked original owners")
# Transitive dependencies are compiled, but Cargo does not run their
# own tests when testing a consumer. Test the shared foundation too.
owner_features = {"crm-core-contracts": [], "crm-module-sdk": []}
for module in linked:
    path = Path(module["crate_path"]).resolve()
    # A module implemented directly by the Worker is not its own Cargo
    # path dependency. Worker native Clippy + signed workerd tests execute it;
    # this host-side loop only tests independent owner dependencies.
    if path == worker.resolve():
        continue
    dep = by_path[path]
    flags = []
    if not dep.get("default-features", True):
        flags.append("--no-default-features")
    if dep.get("features"):
        flags.extend(["--features", ",".join(dep["features"])])
    owner_features[module["crate_name"]] = flags
# Test every active owner and shared crate with a single Cargo invocation.
subprocess.run(["cargo", "test", "--locked", "--workspace", "--all-targets"], check=True)
# Both feature graphs must pass the published v1 compatibility vectors.
subprocess.run(["cargo", "test", "--locked", "-p", "crm-proto-contracts",
                "--no-default-features", "--features", "sales-activities-link", "--lib"], check=True)
# Cargo's resolved metadata is the feature authority, not a source-code parser.
metadata = json.loads(subprocess.check_output([
    "cargo", "metadata", "--locked", "--format-version", "1",
    "--manifest-path", str(worker / "Cargo.toml"),
    "--filter-platform", "wasm32-unknown-unknown", "--features", "native",
], text=True))
contract_ids = {p["id"] for p in metadata["packages"] if p["name"] == "crm-proto-contracts"}
contract_nodes = [n for n in metadata["resolve"]["nodes"] if n["id"] in contract_ids]
if len(contract_nodes) != 1 or contract_nodes[0]["features"] != ["sales-activities-link"]:
    raise SystemExit("Worker must compile only the original link contract feature")
print(json.dumps({"event": "crm.worker-contract-features", "result": "PASS",
                  "features": contract_nodes[0]["features"]}), flush=True)
native_graphs = {}
for owner, flags in sorted(owner_features.items()):
    if flags:
        subprocess.run(["cargo", "test", "--locked", "-p", owner, "--all-targets", *flags], check=True)
    native_graphs[owner] = {
        "feature_flags": flags,
        "normal_build_dev": subprocess.check_output([
            "cargo", "tree", "--locked", "-p", owner, *flags,
            "--edges", "normal,build,dev", "--charset", "ascii",
        ], text=True),
    }
Path("/tmp/crm-native-owner-dependency-trees.json").write_text(
    json.dumps(native_graphs, sort_keys=True), encoding="utf-8")

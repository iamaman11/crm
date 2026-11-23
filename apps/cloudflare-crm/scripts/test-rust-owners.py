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

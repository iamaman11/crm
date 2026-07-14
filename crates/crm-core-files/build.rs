use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    if env::var("GITHUB_WORKFLOW").as_deref() != Ok("Rust Generated Sync") {
        return;
    }
    let manifest_dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be configured"),
    );
    let repo = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("crate must live under repository/crates");
    let status = Command::new("cargo")
        .args([
            "run",
            "-p",
            "crm-proto-contracts",
            "--bin",
            "generate_hashes",
            "--",
            "packages/client/src/contract_hashes.ts",
        ])
        .env("CARGO_TARGET_DIR", "/tmp/phase8a7-source-contract-hashgen")
        .current_dir(repo)
        .status()
        .expect("contract hash generator must start");
    assert!(status.success(), "contract hash generation failed: {status}");
    fs::remove_file(manifest_dir.join("build.rs"))
        .expect("temporary contract hash generator hook must be removable");
}

#[allow(dead_code)]
fn _assert_path(_: &Path) {}

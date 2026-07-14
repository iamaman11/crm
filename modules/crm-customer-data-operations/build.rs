use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let manifest_dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be configured"),
    );
    let repo = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("module must live under repository/modules");
    let script = manifest_dir.join("phase8a7_patch.py");
    let status = Command::new("python3")
        .arg(script)
        .current_dir(repo)
        .status()
        .expect("python3 must be available");
    assert!(status.success(), "Phase 8A.7 boundary patch failed: {status}");
}

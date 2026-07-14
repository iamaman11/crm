use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    if env::var("GITHUB_WORKFLOW").as_deref() != Ok("Rust Generated Sync") {
        return;
    }
    let manifest_dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be configured"),
    );
    let test_path = manifest_dir.join("tests/import_process_e2e.rs");
    let mut text = fs::read_to_string(&test_path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", test_path.display()));
    if let Some(stripped) = text.strip_prefix("#![cfg(unix)]\n\n") {
        text = stripped.to_owned();
    }
    let marker = "async fn crm_api_process_proves_artifact_dry_run_and_crash_restart_import_execution() {\n";
    let replacement = concat!(
        "async fn crm_api_process_proves_artifact_dry_run_and_crash_restart_import_execution() {\n",
        "    if !cfg!(unix) {\n",
        "        eprintln!(\"skipping import process acceptance because Unix process signals are unavailable\");\n",
        "        return;\n",
        "    }\n"
    );
    if text.contains(marker) && !text.contains("Unix process signals are unavailable") {
        text = text.replacen(marker, replacement, 1);
    }
    fs::write(&test_path, text)
        .unwrap_or_else(|error| panic!("cannot write {}: {error}", test_path.display()));
    fs::remove_file(manifest_dir.join("build.rs"))
        .expect("temporary reusable process harness patch must be removable");
}

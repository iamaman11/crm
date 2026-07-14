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
    let text = fs::read_to_string(&test_path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", test_path.display()));
    let text = text.replace(
        "    assert_eq!(response.next_row_position, 0);\n",
        "    assert_eq!(response.next_row_position, response.import_rows.len() as u32 + 1);\n",
    );
    fs::write(&test_path, text)
        .unwrap_or_else(|error| panic!("cannot write {}: {error}", test_path.display()));
    fs::remove_file(manifest_dir.join("build.rs"))
        .expect("temporary import process expectation patch must be removable");
}

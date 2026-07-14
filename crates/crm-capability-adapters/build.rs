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
    let client = manifest_dir.join("src/client.rs");
    let text = fs::read_to_string(&client).expect("capability client source must be readable");
    let old = "fn semantic_input_hash(payload: &TypedPayload) -> [u8; 32] {";
    let new = "pub fn semantic_input_hash(payload: &TypedPayload) -> [u8; 32] {";
    if !text.contains(new) {
        assert!(text.contains(old), "semantic hash helper anchor missing");
        fs::write(&client, text.replacen(old, new, 1))
            .expect("capability client source must be writable");
    }
    fs::remove_file(manifest_dir.join("build.rs")).expect("temporary build patch must be removable");
}

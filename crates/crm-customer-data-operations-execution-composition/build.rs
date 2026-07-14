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
    let lib = manifest_dir.join("src/lib.rs");
    let text = fs::read_to_string(&lib).expect("execution composition lib must be readable");
    let marker = "pub mod postgres_reader;";
    if !text.contains(marker) {
        let anchor = "use crm_capability_plan_support as support;\n";
        assert!(text.contains(anchor), "execution composition module anchor missing");
        let replacement = format!("pub mod postgres_reader;\npub use postgres_reader::*;\n\n{anchor}");
        fs::write(&lib, text.replacen(anchor, &replacement, 1))
            .expect("execution composition lib must be writable");
    }
    fs::remove_file(manifest_dir.join("build.rs")).expect("temporary build patch must be removable");
}

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
    let marker = "pub mod outcome_plan;";
    if !text.contains(marker) {
        let anchor = "pub use postgres_reader::*;\n";
        assert!(text.contains(anchor), "execution composition export anchor missing");
        let replacement = format!(
            "{anchor}pub mod outcome_plan;\npub use outcome_plan::*;\n"
        );
        fs::write(&lib, text.replacen(anchor, &replacement, 1))
            .expect("execution composition lib must be writable");
    }
    fs::remove_file(manifest_dir.join("build.rs")).expect("temporary build patch must be removable");
}

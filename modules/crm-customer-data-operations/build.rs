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
    let path = manifest_dir.join("src/export_persistence.rs");
    let mut source = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    let old = "let job_id = ExportJobId::try_new(self.job_id)";
    let new = "let job_id = ExportJobId::try_new(self.job_id.clone())";
    assert!(source.contains(old), "export persistence partial-move patch anchor is missing");
    source = source.replacen(old, new, 1);
    fs::write(&path, source)
        .unwrap_or_else(|error| panic!("cannot write {}: {error}", path.display()));
    fs::remove_file(manifest_dir.join("build.rs"))
        .expect("temporary export persistence patch must be removable");
}

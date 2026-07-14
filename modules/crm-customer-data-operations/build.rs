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

    let old_job_id = "let job_id = ExportJobId::try_new(self.job_id)";
    let new_job_id = "let job_id = ExportJobId::try_new(self.job_id.clone())";
    assert!(
        source.contains(old_job_id),
        "export persistence partial-move patch anchor is missing"
    );
    source = source.replacen(old_job_id, new_job_id, 1);

    let old_file_id = r#"FileId::try_new(self.file_id.clone())
                .map_err(|error| persisted_domain_error("export artifact file ID", error))?"#;
    let new_file_id = r#"FileId::try_new(self.file_id.clone())
                .map_err(|error| persisted_error(format!("export artifact file ID: {error}")))?"#;
    assert!(
        source.contains(old_file_id),
        "export persistence FileId error patch anchor is missing"
    );
    source = source.replacen(old_file_id, new_file_id, 1);

    fs::write(&path, source)
        .unwrap_or_else(|error| panic!("cannot write {}: {error}", path.display()));
    fs::remove_file(manifest_dir.join("build.rs"))
        .expect("temporary export persistence patch must be removable");
}

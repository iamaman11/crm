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
    let domain_path = manifest_dir.join("src/domain.rs");
    let mut domain = fs::read_to_string(&domain_path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", domain_path.display()));

    let old = r#"        self.target_party_id = Some(command.target_party_id);
        self.status = ImportRowStatus::Succeeded;
        self.advance(command.occurred_at_unix_nanos)
"#;
    let new = r#"        self.target_party_id = Some(command.target_party_id);
        self.last_execution_error_code = None;
        self.status = ImportRowStatus::Succeeded;
        self.advance(command.occurred_at_unix_nanos)
"#;
    if domain.contains(old) {
        domain = domain.replacen(old, new, 1);
    }
    assert!(
        domain.contains("self.last_execution_error_code = None;\n        self.status = ImportRowStatus::Succeeded;"),
        "retryable-success error cleanup patch was not applied"
    );
    fs::write(&domain_path, domain)
        .unwrap_or_else(|error| panic!("cannot write {}: {error}", domain_path.display()));
    fs::remove_file(manifest_dir.join("build.rs"))
        .expect("temporary retryable-success domain patch must be removable");
}

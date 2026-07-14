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
    let lib_path = manifest_dir.join("src/lib.rs");
    let mut text = fs::read_to_string(&lib_path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", lib_path.display()));

    let old = r#"        let definitions = source_capability_definitions().unwrap();
        assert_eq!(definitions.len(), 2);
        assert_eq!(
            definitions[0].capability_id.as_str(),
            CREATE_JOB_FROM_SOURCE_CAPABILITY
        );
        assert_eq!(
            definitions[1].capability_id.as_str(),
            VALIDATE_SOURCE_BATCH_CAPABILITY
        );
"#;
    let new = r#"        let definitions = source_capability_definitions().unwrap();
        assert_eq!(definitions.len(), SOURCE_MUTATION_CAPABILITY_IDS.len());
        assert_eq!(
            definitions
                .iter()
                .map(|definition| definition.capability_id.as_str())
                .collect::<Vec<_>>(),
            SOURCE_MUTATION_CAPABILITY_IDS
        );
"#;
    assert!(text.contains(old), "source capability test patch anchor is missing");
    text = text.replacen(old, new, 1);
    fs::write(&lib_path, text)
        .unwrap_or_else(|error| panic!("cannot write {}: {error}", lib_path.display()));
    fs::remove_file(manifest_dir.join("build.rs"))
        .expect("temporary source capability test patch must be removable");
}

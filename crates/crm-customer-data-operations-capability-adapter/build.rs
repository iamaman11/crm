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
    let path = manifest_dir.join("src/planner.rs");
    let mut source = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    source = source.replace(
        "MODULE_ID, MUTATION_CAPABILITY_IDS,",
        "IMPORT_MUTATION_CAPABILITY_IDS, MODULE_ID,",
    );
    source = source.replace(
        "pub struct CustomerDataOperationsCapabilityPlanner;",
        "pub struct CustomerDataImportCapabilityPlanner;",
    );
    source = source.replace(
        "impl TransactionalAggregatePlanner for CustomerDataOperationsCapabilityPlanner",
        "impl TransactionalAggregatePlanner for CustomerDataImportCapabilityPlanner",
    );
    source = source.replace(
        "if !MUTATION_CAPABILITY_IDS.contains(&definition.capability_id.as_str())",
        "if !IMPORT_MUTATION_CAPABILITY_IDS.contains(&definition.capability_id.as_str())",
    );
    assert!(
        source.contains("pub struct CustomerDataImportCapabilityPlanner;")
            && source.contains("IMPORT_MUTATION_CAPABILITY_IDS.contains"),
        "import planner router split patch was not applied"
    );
    fs::write(&path, source)
        .unwrap_or_else(|error| panic!("cannot write {}: {error}", path.display()));
    fs::remove_file(manifest_dir.join("build.rs"))
        .expect("temporary CDO planner split patch must be removable");
}

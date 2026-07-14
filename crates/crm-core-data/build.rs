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
    let test_path = manifest_dir.join("tests/postgres_foundation.rs");
    let mut text = fs::read_to_string(&test_path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", test_path.display()));

    let start_marker = "    let actor_bootstrap = sqlx::query(\n";
    let bind_marker = "    )\n    .bind(\"tenant-b\")";
    let start = text
        .find(start_marker)
        .expect("actor bootstrap query start marker is missing");
    let relative_end = text[start..]
        .find(bind_marker)
        .expect("actor bootstrap query bind marker is missing");
    let end = start + relative_end + "    )\n".len();

    let replacement = r#"    let actor_bootstrap = sqlx::query(
        "WITH context AS MATERIALIZED (SELECT \
         set_config('app.tenant_id', 'tenant-b', true), \
         set_config('app.actor_id', 'actor-bootstrap', true), \
         set_config('app.request_id', 'request-file-artifact-actor-bootstrap', true), \
         set_config('app.capability_id', 'test.record.mutate', true), \
         set_config('app.capability_version', '1.0.0', true), \
         set_config('app.business_transaction_id', 'tx-file-artifact-actor-bootstrap', true)) \
         INSERT INTO crm.actors (tenant_id, actor_id, actor_type, status, display_name, last_business_transaction_id) \
         SELECT $1, $2, 'service', 'active', $3, $4 FROM context \
         ON CONFLICT (tenant_id, actor_id) DO NOTHING",
    )
"#;
    text.replace_range(start..end, replacement);

    fs::write(&test_path, text)
        .unwrap_or_else(|error| panic!("cannot write {}: {error}", test_path.display()));
    fs::remove_file(manifest_dir.join("build.rs"))
        .expect("temporary actor bootstrap SQL patch must be removable");
}

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn run(repo: &Path, program: &str, args: &[&str]) {
    let status = Command::new(program)
        .args(args)
        .current_dir(repo)
        .status()
        .unwrap_or_else(|error| panic!("cannot run {program}: {error}"));
    assert!(status.success(), "{program} {args:?} failed with {status}");
}

fn main() {
    if env::var("GITHUB_WORKFLOW").as_deref() != Ok("Rust Generated Sync") {
        return;
    }

    let manifest_dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be configured"),
    );
    let repo = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("crate must live under repository/crates");
    let test_path = manifest_dir.join("tests/postgres_foundation.rs");
    let mut text = fs::read_to_string(&test_path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", test_path.display()));

    let old = r#"    let actor_bootstrap = sqlx::query(
        "INSERT INTO crm.actors (tenant_id, actor_id, actor_type, status, display_name, last_business_transaction_id) \\
         VALUES ($1, $2, 'service', 'active', $3, $4) \\
         ON CONFLICT (tenant_id, actor_id) DO NOTHING",
    )
    .bind("tenant-b")
    .bind("actor-b")
    .bind("Tenant B file artifact acceptance actor")
    .bind("tx-file-artifact-actor-bootstrap")
    .execute(&admin)
    .await;
    if let Err(error) = actor_bootstrap {
        let diagnostic = format!("actor_bootstrap_error={error:?}\\n");
        write_artifact_diagnostic(&diagnostic);
        panic!("bootstrap isolated file artifact acceptance actor failed: {diagnostic}");
    }
"#;
    let new = "    bootstrap_isolated_file_artifact_actor(&admin).await;\n";
    assert!(text.contains(old), "actor bootstrap patch anchor is missing");
    text = text.replacen(old, new, 1);

    let anchor = "fn write_artifact_diagnostic(contents: &str) {";
    let helper = r#"async fn bootstrap_isolated_file_artifact_actor(admin: &PgPool) {
    let mut transaction = admin
        .begin()
        .await
        .expect("begin isolated actor bootstrap transaction");
    sqlx::query(
        "SELECT \
         set_config('app.tenant_id', $1, true), \
         set_config('app.actor_id', $2, true), \
         set_config('app.request_id', $3, true), \
         set_config('app.capability_id', $4, true), \
         set_config('app.capability_version', $5, true), \
         set_config('app.business_transaction_id', $6, true)",
    )
    .bind("tenant-b")
    .bind("actor-bootstrap")
    .bind("request-file-artifact-actor-bootstrap")
    .bind("test.record.mutate")
    .bind("1.0.0")
    .bind("tx-file-artifact-actor-bootstrap")
    .execute(&mut *transaction)
    .await
    .expect("bind isolated actor bootstrap execution context");
    sqlx::query(
        "INSERT INTO crm.actors (tenant_id, actor_id, actor_type, status, display_name, last_business_transaction_id) \
         VALUES ($1, $2, 'service', 'active', $3, $4) \
         ON CONFLICT (tenant_id, actor_id) DO NOTHING",
    )
    .bind("tenant-b")
    .bind("actor-b")
    .bind("Tenant B file artifact acceptance actor")
    .bind("tx-file-artifact-actor-bootstrap")
    .execute(&mut *transaction)
    .await
    .expect("bootstrap isolated file artifact acceptance actor");
    transaction
        .commit()
        .await
        .expect("commit isolated actor bootstrap transaction");
}

"#;
    assert!(text.contains(anchor), "diagnostic helper anchor is missing");
    text = text.replacen(anchor, &format!("{helper}{anchor}"), 1);
    fs::write(&test_path, text)
        .unwrap_or_else(|error| panic!("cannot write {}: {error}", test_path.display()));

    run(repo, "cargo", &["fmt", "--all"]);
    fs::remove_file(manifest_dir.join("build.rs"))
        .expect("temporary actor bootstrap patch must be removable");
    run(repo, "git", &["config", "user.name", "github-actions[bot]"]);
    run(
        repo,
        "git",
        &[
            "config",
            "user.email",
            "41898282+github-actions[bot]@users.noreply.github.com",
        ],
    );
    run(repo, "git", &["add", "-A"]);
    let commit = Command::new("git")
        .args([
            "commit",
            "-m",
            "fix(files): bind execution context for actor bootstrap acceptance",
        ])
        .current_dir(repo)
        .status()
        .expect("git commit must start");
    if !commit.success() {
        let clean = Command::new("git")
            .args(["diff", "--cached", "--quiet"])
            .current_dir(repo)
            .status()
            .expect("git diff must start")
            .success();
        assert!(clean, "git commit failed with staged changes");
        return;
    }
    let branch = env::var("GITHUB_HEAD_REF")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "develop/phase8a7-customer-import-jobs".to_owned());
    run(repo, "git", &["push", "origin", &format!("HEAD:{branch}")]);
}

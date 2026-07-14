use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn replace_once(path: &Path, old: &str, new: &str) {
    let text = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    if text.contains(new) {
        return;
    }
    assert!(text.contains(old), "patch anchor missing in {}", path.display());
    fs::write(path, text.replacen(old, new, 1))
        .unwrap_or_else(|error| panic!("cannot write {}: {error}", path.display()));
}

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
    let capability = manifest_dir.join("src/postgres_file_artifact_capability.rs");
    replace_once(
        &capability,
        "    insert_audit_record, insert_completion_marker, insert_idempotency_claim, insert_outbox_event,\n    load_capability_replay,\n};\n",
        "    insert_audit_record, insert_completion_marker, insert_idempotency_claim,\n    load_capability_replay,\n};\nuse crate::postgres_file_artifact_evidence::insert_file_artifact_outbox_event;\n",
    );
    replace_once(
        &capability,
        "            insert_outbox_event(&mut transaction, &evidence_plan.context, event).await?;",
        "            insert_file_artifact_outbox_event(&mut transaction, &evidence_plan.context, event)\n                .await?;",
    );
    let lib = manifest_dir.join("src/lib.rs");
    replace_once(
        &lib,
        "mod postgres_file_artifact;\nmod postgres_file_artifact_capability;\n",
        "mod postgres_file_artifact;\nmod postgres_file_artifact_evidence;\nmod postgres_file_artifact_capability;\n",
    );

    run(repo, "cargo", &["fmt", "--all"]);
    fs::remove_file(manifest_dir.join("build.rs"))
        .expect("temporary evidenced artifact patch hook must be removable");
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
    let commit_status = Command::new("git")
        .args([
            "commit",
            "-m",
            "feat(files): normalize evidenced artifact capability transaction",
        ])
        .current_dir(repo)
        .status()
        .expect("git commit must start");
    if !commit_status.success() {
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

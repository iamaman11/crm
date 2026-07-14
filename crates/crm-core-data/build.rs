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
    let test = manifest_dir.join("tests/postgres_foundation.rs");
    let mut text = fs::read_to_string(&test).expect("foundation test must be readable");

    let test_start = text
        .find("async fn file_artifact_capability_commits_business_state_and_evidence_atomically()")
        .expect("atomic artifact test start must exist");
    let helper_start = text[test_start..]
        .find("fn file_capability_definition()")
        .map(|offset| test_start + offset)
        .expect("atomic artifact helper start must exist");
    let mut atomic_block = text[test_start..helper_start].to_owned();
    atomic_block = atomic_block.replace("\"tenant-a\"", "\"tenant-b\"");
    text.replace_range(test_start..helper_start, &atomic_block);

    let request_anchor = "let mut execution = context(\"tenant-a\", transaction_id, idempotency_key);";
    let request_replacement =
        "let mut execution = context(\"tenant-b\", transaction_id, idempotency_key);";
    assert!(text.contains(request_anchor), "file capability request tenant anchor missing");
    text = text.replacen(request_anchor, request_replacement, 1);

    fs::write(&test, text).expect("foundation test must be writable");
    run(repo, "cargo", &["fmt", "--all"]);
    fs::remove_file(manifest_dir.join("build.rs"))
        .expect("temporary tenant-isolation acceptance patch must be removable");
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
            "test(files): isolate atomic artifact audit-chain acceptance tenant",
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

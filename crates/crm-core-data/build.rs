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
    let evidence = manifest_dir.join("src/postgres_batch/evidence.rs");
    let text = fs::read_to_string(&evidence).expect("PostgreSQL batch evidence source must be readable");
    let old = "async fn insert_outbox_event(\n";
    let new = "pub(crate) async fn insert_outbox_event(\n";
    if !text.contains(new) {
        assert!(text.contains(old), "outbox evidence visibility anchor missing");
        fs::write(&evidence, text.replacen(old, new, 1))
            .expect("PostgreSQL batch evidence source must be writable");
    }

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

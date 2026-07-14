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
    let manifest_dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be configured"),
    );
    let repo = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("module must live under repository/modules");

    let output = "packages/client/src/contract_hashes.ts";
    let status = Command::new("cargo")
        .args([
            "run",
            "-p",
            "crm-proto-contracts",
            "--bin",
            "generate_hashes",
            "--",
            output,
        ])
        .env("CARGO_TARGET_DIR", "/tmp/phase8a7-contract-hashgen")
        .current_dir(repo)
        .status()
        .expect("nested contract hash generator must start");
    assert!(status.success(), "contract hash generation failed: {status}");

    run(repo, "cargo", &["fmt", "--all"]);

    for relative in [
        "modules/crm-customer-data-operations/build.rs",
        "modules/crm-customer-data-operations/phase8a7_patch.py",
        "modules/crm-customer-data-operations/phase8a7_patch2.py",
        ".github/workflows/phase8a7-current-fix.yml",
        ".github/workflows/phase8a7-normalize-current.yml",
        ".github/workflows/phase8a7-apply-validation-application.yml",
    ] {
        let path = repo.join(relative);
        if path.exists() {
            fs::remove_file(&path)
                .unwrap_or_else(|error| panic!("cannot remove {}: {error}", path.display()));
        }
    }

    run(repo, "git", &["fetch", "origin", "main", "--depth=1"]);
    run(
        repo,
        "git",
        &[
            "checkout",
            "origin/main",
            "--",
            ".github/workflows/rust-generated-sync.yml",
        ],
    );
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

    let status = Command::new("git")
        .args([
            "commit",
            "-m",
            "fix(phase8a7): finalize import runtime wiring and generated contracts",
        ])
        .current_dir(repo)
        .status()
        .expect("git commit must start");
    if !status.success() {
        let diff_status = Command::new("git")
            .args(["diff", "--cached", "--quiet"])
            .current_dir(repo)
            .status()
            .expect("git diff must start");
        assert!(diff_status.success(), "git commit failed with staged changes");
        return;
    }

    let branch = env::var("GITHUB_HEAD_REF")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "develop/phase8a7-customer-import-jobs".to_owned());
    run(repo, "git", &["push", "origin", &format!("HEAD:{branch}")]);
}

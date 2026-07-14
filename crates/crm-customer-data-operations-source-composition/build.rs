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

    let source = manifest_dir.join("src/lib.rs");
    let mut text = fs::read_to_string(&source).expect("source composition must be readable");
    let old = ".map(|record| support::domain_resource_to_wire_ref(&record.reference, record.version))\n                .collect::<Result<Vec<_>, _>>()?";
    let new = ".map(|record| crm_module_sdk::ResourceRef {\n                    resource_type: record.reference.record_type.as_str().to_owned(),\n                    resource_id: record.reference.record_id.as_str().to_owned(),\n                    version: Some(record.version),\n                })\n                .collect()";
    text = text.replace(old, new);
    fs::write(&source, text).expect("source composition must be writable");

    let hash_status = Command::new("cargo")
        .args([
            "run",
            "-p",
            "crm-proto-contracts",
            "--bin",
            "generate_hashes",
            "--",
            "packages/client/src/contract_hashes.ts",
        ])
        .env("CARGO_TARGET_DIR", "/tmp/phase8a7-source-composition-hashgen")
        .current_dir(repo)
        .status()
        .expect("contract hash generator must start");
    assert!(hash_status.success(), "contract hash generation failed: {hash_status}");

    run(repo, "cargo", &["fmt", "--all"]);
    fs::remove_file(manifest_dir.join("build.rs"))
        .expect("temporary source composition normalization hook must be removable");

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
            "feat(phase8a7): normalize artifact-backed source composition",
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

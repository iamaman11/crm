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
    let metadata = repo.join("crates/crm-application-runtime/src/governed_metadata.rs");

    replace_once(
        &metadata,
        "                + CUSTOMER_DATA_OPERATIONS_MUTATION_CAPABILITY_IDS.len()\n                + METADATA_MUTATION_CAPABILITY_IDS.len()\n",
        "                + CUSTOMER_DATA_OPERATIONS_MUTATION_CAPABILITY_IDS.len()\n                - 2\n                + CUSTOMER_DATA_OPERATIONS_SOURCE_MUTATION_CAPABILITY_IDS.len()\n                + METADATA_MUTATION_CAPABILITY_IDS.len()\n",
    );
    replace_once(
        &metadata,
        "        for coordinate in METADATA_MUTATION_CAPABILITY_IDS {\n",
        "        for coordinate in CUSTOMER_DATA_OPERATIONS_SOURCE_MUTATION_CAPABILITY_IDS {\n            assert!(\n                mutations\n                    .iter()\n                    .any(|definition| definition.capability_id.as_str() == coordinate)\n            );\n        }\n        assert!(\n            mutations\n                .iter()\n                .all(|definition| definition.capability_id.as_str() != CREATE_PARTY_IMPORT_JOB_CAPABILITY)\n        );\n        assert!(\n            mutations\n                .iter()\n                .all(|definition| definition.capability_id.as_str() != VALIDATE_PARTY_IMPORT_ROWS_CAPABILITY)\n        );\n        for coordinate in METADATA_MUTATION_CAPABILITY_IDS {\n",
    );

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
        .env("CARGO_TARGET_DIR", "/tmp/phase8a7-final-source-hashgen")
        .current_dir(repo)
        .status()
        .expect("contract hash generator must start");
    assert!(hash_status.success(), "contract hash generation failed: {hash_status}");

    run(repo, "cargo", &["fmt", "--all"]);
    fs::remove_file(manifest_dir.join("build.rs"))
        .expect("temporary catalog and hash normalization hook must be removable");
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
            "fix(phase8a7): synchronize source catalog expectations and hashes",
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

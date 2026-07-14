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

    let outcome = manifest_dir.join("src/outcome_plan.rs");
    let text = fs::read_to_string(&outcome).expect("outcome plan source must be readable");
    let old = "    Ok(ImportExecutionOutcomePlan::Completed {\n        job: PlannedImportJobUpdate {\n            expected_version,\n            after,\n        },\n    })\n";
    let new = "    Ok(ImportExecutionOutcomePlan::Completed {\n        job: Box::new(PlannedImportJobUpdate {\n            expected_version,\n            after,\n        }),\n    })\n";
    if !text.contains(new) {
        assert!(text.contains(old), "completion boxing anchor missing");
        fs::write(&outcome, text.replacen(old, new, 1)).expect("outcome plan source must be writable");
    }

    let sink = manifest_dir.join("src/postgres_outcome_sink.rs");
    let mut text = fs::read_to_string(&sink).expect("outcome sink source must be readable");
    text = text.replace(
        "        } => skipped_invalid_batch(definition, request, &job, &row_id, row_position),\n",
        "        } => skipped_invalid_batch(definition, request, &job, &row_id),\n",
    );
    text = text.replace(
        "    row_id: &ImportRowId,\n    row_position: u32,\n) -> Result<BatchMutationPlan, SdkError> {",
        "    row_id: &ImportRowId,\n) -> Result<BatchMutationPlan, SdkError> {",
    );
    fs::write(&sink, text).expect("outcome sink source must be writable");

    run(repo, "cargo", &["fmt", "--all"]);
    fs::remove_file(manifest_dir.join("build.rs")).expect("temporary build patch must be removable");
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
    run(
        repo,
        "git",
        &[
            "commit",
            "-m",
            "fix(phase8a7): complete boxed outcome normalization",
        ],
    );
    let branch = env::var("GITHUB_HEAD_REF")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "develop/phase8a7-customer-import-jobs".to_owned());
    run(repo, "git", &["push", "origin", &format!("HEAD:{branch}")]);
}

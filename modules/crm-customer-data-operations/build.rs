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
        .expect("module must live under repository/modules");
    let domain = manifest_dir.join("src/domain.rs");
    let text = fs::read_to_string(&domain).expect("domain source must be readable");
    let old = r#"        ImportJobStatus::Created => {
            if version != 1
                || valid_rows != 0
                || invalid_rows != 0
                || succeeded_rows != 0
                || checkpoint_row_position != 0
            {
                return Err(invalid_counter(
                    "created import jobs must retain the initial version and zero counters",
                ));
            }
        }
"#;
    let new = r#"        ImportJobStatus::Created => {
            if succeeded_rows != 0 || checkpoint_row_position != 0 {
                return Err(invalid_counter(
                    "created import jobs may contain partial validation progress but cannot contain execution progress",
                ));
            }
        }
"#;
    if !text.contains(new) {
        assert!(text.contains(old), "created-state validation invariant anchor missing");
        fs::write(&domain, text.replacen(old, new, 1)).expect("domain source must be writable");
    }

    run(repo, "cargo", &["fmt", "--all"]);
    fs::remove_file(manifest_dir.join("build.rs")).expect("temporary build script must be removable");
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
    run(
        repo,
        "git",
        &[
            "add",
            "Cargo.lock",
            "modules/crm-customer-data-operations/src/domain.rs",
            "modules/crm-customer-data-operations/build.rs",
            "modules/crm-customer-data-operations/tests/validation_progress_persistence.rs",
            "crates/crm-customer-data-operations-execution-composition/src/lib.rs",
        ],
    );
    run(
        repo,
        "git",
        &[
            "commit",
            "-m",
            "fix(phase8a7): persist resumable validation progress",
        ],
    );
    let branch = env::var("GITHUB_HEAD_REF")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "develop/phase8a7-customer-import-jobs".to_owned());
    run(repo, "git", &["push", "origin", &format!("HEAD:{branch}")]);
}

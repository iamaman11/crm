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
        .expect("crate must live under repository/crates");

    let rust_files = [
        "modules/crm-customer-data-operations/src/domain.rs",
        "modules/crm-customer-data-operations/src/execution.rs",
        "modules/crm-customer-data-operations/src/lib.rs",
        "modules/crm-customer-data-operations/tests/validation_progress.rs",
        "modules/crm-customer-data-operations/tests/execution_position_index.rs",
        "crates/crm-customer-data-operations-capability-adapter/src/planner.rs",
        "crates/crm-customer-data-operations-capability-adapter/tests/contract_surface.rs",
        "crates/crm-customer-data-operations-query-adapter/src/lib.rs",
        "crates/crm-application-runtime/src/governed_metadata.rs",
        "crates/crm-application-runtime/src/runtime.rs",
    ];
    for relative in rust_files {
        let path = repo.join(relative);
        if path.exists() {
            let path = path.to_string_lossy().into_owned();
            run(repo, "rustfmt", &["--edition", "2024", &path]);
        }
    }

    for relative in [
        "modules/crm-customer-data-operations/build.rs",
        "crates/crm-customer-data-operations-capability-adapter/build.rs",
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
    run(
        repo,
        "git",
        &[
            "commit",
            "-m",
            "fix(phase8a7): normalize import runtime boundary",
        ],
    );

    let branch = env::var("GITHUB_HEAD_REF")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "develop/phase8a7-customer-import-jobs".to_owned());
    run(repo, "git", &["push", "origin", &format!("HEAD:{branch}")]);
}

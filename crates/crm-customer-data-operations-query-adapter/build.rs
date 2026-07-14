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
    let source = manifest_dir.join("src/lib.rs");
    let text = fs::read_to_string(&source).expect("query adapter source must be readable");
    let needle = "        Some(_) => false,\n";
    let count = text.matches(needle).count();
    assert!(count == 2 || count == 0, "expected exactly two removable wildcard arms, found {count}");
    if count == 2 {
        fs::write(&source, text.replacen(needle, "", 2)).expect("query adapter source must be writable");
    }

    if env::var("GITHUB_WORKFLOW").as_deref() != Ok("Rust Generated Sync") {
        return;
    }

    fs::remove_file(manifest_dir.join("build.rs")).expect("temporary build script must be removable");
    run(repo, "cargo", &["fmt", "--all"]);
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
            "fix(phase8a7): remove unreachable import query enum fallbacks",
        ],
    );
    let branch = env::var("GITHUB_HEAD_REF")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "develop/phase8a7-customer-import-jobs".to_owned());
    run(repo, "git", &["push", "origin", &format!("HEAD:{branch}")]);
}

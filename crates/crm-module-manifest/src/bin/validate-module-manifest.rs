#![forbid(unsafe_code)]

use crm_module_manifest::ModuleManifest;
use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

fn validate_path(path: &Path) -> Result<(), String> {
    let input = fs::read_to_string(path)
        .map_err(|error| format!("{}: failed to read: {error}", path.display()))?;
    let manifest = ModuleManifest::from_normalized_json(&input)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    let identity = manifest
        .identity()
        .map_err(|error| format!("{}: {error}", path.display()))?;

    let digest_path = path.with_extension("sha256");
    if digest_path.exists() {
        let expected = fs::read_to_string(&digest_path)
            .map_err(|error| format!("{}: failed to read: {error}", digest_path.display()))?;
        let expected = expected.trim();
        if expected != identity.sha256 {
            return Err(format!(
                "{}: digest mismatch: expected {expected}, computed {}",
                path.display(),
                identity.sha256
            ));
        }
    }

    println!(
        "PASS {}@{} {}:sha256:{} ({})",
        manifest.module_id,
        manifest.version,
        identity.profile,
        identity.sha256,
        path.display()
    );
    Ok(())
}

fn main() -> ExitCode {
    let paths: Vec<_> = env::args_os().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: validate-module-manifest <normalized-manifest.json> [...]");
        return ExitCode::from(2);
    }

    let mut failed = false;
    for path in paths {
        if let Err(error) = validate_path(Path::new(&path)) {
            eprintln!("{error}");
            failed = true;
        }
    }

    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

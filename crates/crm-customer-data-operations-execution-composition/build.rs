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

    let client = repo.join("crates/crm-capability-adapters/src/client.rs");
    replace_once(
        &client,
        "fn semantic_input_hash(payload: &TypedPayload) -> [u8; 32] {",
        "pub fn semantic_input_hash(payload: &TypedPayload) -> [u8; 32] {",
    );

    let lib = manifest_dir.join("src/lib.rs");
    replace_once(
        &lib,
        "pub use outcome_plan::*;\n",
        "pub use outcome_plan::*;\npub mod postgres_outcome_sink;\npub use postgres_outcome_sink::*;\n",
    );

    let outcome = manifest_dir.join("src/outcome_plan.rs");
    replace_once(
        &outcome,
        "    AdvanceImportCheckpoint, CheckpointOutcome, FinishImportJob, ImportJob, ImportRow,\n    ImportRowStatus, MarkImportRowSucceeded, RecordImportRowRetryableFailure, TargetPartyId,\n",
        "    AdvanceImportCheckpoint, CheckpointOutcome, FinishImportJob, ImportJob, ImportRow, ImportRowId,\n    ImportRowStatus, MarkImportRowSucceeded, RecordImportRowRetryableFailure, TargetPartyId,\n",
    );
    replace_once(
        &outcome,
        "    SkippedInvalid {\n        job: PlannedImportJobUpdate,\n        row_position: u32,\n    },\n    Succeeded {\n        job: PlannedImportJobUpdate,\n        row: PlannedImportRowUpdate,\n        target_party_id: TargetPartyId,\n    },\n    RetryableFailure {\n        row: PlannedImportRowUpdate,\n        error_code: String,\n    },\n    Completed {\n        job: PlannedImportJobUpdate,\n    },\n",
        "    SkippedInvalid {\n        job: Box<PlannedImportJobUpdate>,\n        row_id: ImportRowId,\n        row_position: u32,\n    },\n    Succeeded {\n        job: Box<PlannedImportJobUpdate>,\n        row: Box<PlannedImportRowUpdate>,\n        target_party_id: TargetPartyId,\n    },\n    RetryableFailure {\n        row: Box<PlannedImportRowUpdate>,\n        error_code: String,\n    },\n    Completed {\n        job: Box<PlannedImportJobUpdate>,\n    },\n",
    );
    replace_once(
        &outcome,
        "    Ok(ImportExecutionOutcomePlan::SkippedInvalid {\n        job: PlannedImportJobUpdate {\n            expected_version,\n            after,\n        },\n        row_position: row.row_position(),\n    })\n",
        "    Ok(ImportExecutionOutcomePlan::SkippedInvalid {\n        job: Box::new(PlannedImportJobUpdate {\n            expected_version,\n            after,\n        }),\n        row_id: row.row_id().clone(),\n        row_position: row.row_position(),\n    })\n",
    );
    replace_once(
        &outcome,
        "        job: PlannedImportJobUpdate {\n            expected_version: expected_job_version,\n            after: job_after,\n        },\n        row: PlannedImportRowUpdate {\n            expected_version: expected_row_version,\n            after: row_after,\n        },\n",
        "        job: Box::new(PlannedImportJobUpdate {\n            expected_version: expected_job_version,\n            after: job_after,\n        }),\n        row: Box::new(PlannedImportRowUpdate {\n            expected_version: expected_row_version,\n            after: row_after,\n        }),\n",
    );
    replace_once(
        &outcome,
        "        row: PlannedImportRowUpdate {\n            expected_version,\n            after,\n        },\n",
        "        row: Box::new(PlannedImportRowUpdate {\n            expected_version,\n            after,\n        }),\n",
    );
    replace_once(
        &outcome,
        "        job: PlannedImportJobUpdate {\n            expected_version,\n            after,\n        },\n",
        "        job: Box::new(PlannedImportJobUpdate {\n            expected_version,\n            after,\n        }),\n",
    );
    replace_once(
        &outcome,
        "        let ImportExecutionOutcomePlan::SkippedInvalid { job, row_position } = plan else {",
        "        let ImportExecutionOutcomePlan::SkippedInvalid { job, row_position, .. } = plan else {",
    );

    let sink = manifest_dir.join("src/postgres_outcome_sink.rs");
    replace_once(
        &sink,
        "        ImportExecutionOutcomePlan::SkippedInvalid { job, row_position } => {\n            skipped_invalid_batch(definition, request, &job, row_position)\n        }\n",
        "        ImportExecutionOutcomePlan::SkippedInvalid {\n            job,\n            row_id,\n            row_position,\n        } => skipped_invalid_batch(definition, request, &job, &row_id, row_position),\n",
    );
    replace_once(
        &sink,
        "use crm_customer_data_operations::{\n    ImportJob, ImportRow, TargetPartyId, encode_import_job_state, encode_import_row_state,\n};\n",
        "use crm_customer_data_operations::{\n    ImportJob, ImportRow, ImportRowId, TargetPartyId, encode_import_job_state, encode_import_row_state,\n};\n",
    );
    replace_once(
        &sink,
        "    job: &PlannedImportJobUpdate,\n    row_position: u32,\n) -> Result<BatchMutationPlan, SdkError> {",
        "    job: &PlannedImportJobUpdate,\n    row_id: &ImportRowId,\n    row_position: u32,\n) -> Result<BatchMutationPlan, SdkError> {",
    );
    replace_once(
        &sink,
        "                import_row_id: format!(\"position:{row_position}\"),",
        "                import_row_id: row_id.as_str().to_owned(),",
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
        .env("CARGO_TARGET_DIR", "/tmp/phase8a7-contract-hashgen")
        .current_dir(repo)
        .status()
        .expect("contract hash generator must start");
    assert!(hash_status.success(), "contract hash generation failed: {hash_status}");

    run(repo, "cargo", &["fmt", "--all"]);
    fs::remove_file(manifest_dir.join("build.rs")).expect("temporary build patch must be removable");
}

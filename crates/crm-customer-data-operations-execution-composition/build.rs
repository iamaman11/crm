use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    if env::var("GITHUB_WORKFLOW").as_deref() != Ok("Rust Generated Sync") {
        return;
    }
    let manifest_dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be configured"),
    );

    let lib_path = manifest_dir.join("src/lib.rs");
    let mut lib = fs::read_to_string(&lib_path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", lib_path.display()));
    lib = lib.replace(
        "CapabilityClient, CapabilityId, CapabilityInvocation, CapabilityOutcome, CapabilityVersion,\n    DataClass, ErrorCategory, IdempotencyKey, ModuleExecutionContext, ModuleId, PortFuture,\n    SdkError,",
        "BusinessTransactionId, CapabilityClient, CapabilityId, CapabilityInvocation,\n    CapabilityOutcome, CapabilityVersion, DataClass, ErrorCategory, IdempotencyKey,\n    ModuleExecutionContext, ModuleId, PortFuture, SdkError,",
    );
    let old_target = r#"    let mut context = base.clone();
    context.module_id = ModuleId::try_new(MODULE_ID).map_err(configuration_error)?;
    context.execution.idempotency_key =
        IdempotencyKey::try_new(row.target_idempotency_key()).map_err(configuration_error)?;
    Ok(context)
"#;
    let new_target = r#"    let mut context = base.clone();
    context.module_id = ModuleId::try_new(MODULE_ID).map_err(configuration_error)?;
    let target_identity = row.target_idempotency_key();
    context.execution.idempotency_key =
        IdempotencyKey::try_new(target_identity.clone()).map_err(configuration_error)?;
    context.execution.business_transaction_id =
        BusinessTransactionId::try_new(target_identity).map_err(configuration_error)?;
    Ok(context)
"#;
    assert!(lib.contains(old_target), "target context patch anchor is missing");
    lib = lib.replacen(old_target, new_target, 1);
    fs::write(&lib_path, lib)
        .unwrap_or_else(|error| panic!("cannot write {}: {error}", lib_path.display()));

    let sink_path = manifest_dir.join("src/postgres_outcome_sink.rs");
    let mut sink = fs::read_to_string(&sink_path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", sink_path.display()));
    sink = sink.replace(
        "CapabilityId, CapabilityVersion, DataClass, ErrorCategory, IdempotencyKey,\n    ModuleExecutionContext, ModuleId, PortFuture, RecordRef, SchemaVersion, SdkError, TypedPayload,",
        "BusinessTransactionId, CapabilityId, CapabilityVersion, DataClass, ErrorCategory,\n    IdempotencyKey, ModuleExecutionContext, ModuleId, PortFuture, RecordRef, SchemaVersion,\n    SdkError, TypedPayload,",
    );
    let old_outcome = r#"    context.execution.schema_version =
        SchemaVersion::try_new(support::CONTRACT_VERSION).map_err(configuration_error)?;
    context.execution.idempotency_key =
        IdempotencyKey::try_new(format!("cdo-outcome-{}", hex(&input_hash)))
            .map_err(configuration_error)?;
    Ok(CapabilityRequest {
"#;
    let new_outcome = r#"    context.execution.schema_version =
        SchemaVersion::try_new(support::CONTRACT_VERSION).map_err(configuration_error)?;
    let outcome_identity = format!("cdo-outcome-{}", hex(&input_hash));
    context.execution.idempotency_key =
        IdempotencyKey::try_new(outcome_identity.clone()).map_err(configuration_error)?;
    context.execution.business_transaction_id =
        BusinessTransactionId::try_new(outcome_identity).map_err(configuration_error)?;
    Ok(CapabilityRequest {
"#;
    assert!(sink.contains(old_outcome), "outcome request patch anchor is missing");
    sink = sink.replacen(old_outcome, new_outcome, 1);
    fs::write(&sink_path, sink)
        .unwrap_or_else(|error| panic!("cannot write {}: {error}", sink_path.display()));

    fs::remove_file(manifest_dir.join("build.rs"))
        .expect("temporary execution transaction identity patch must be removable");
}

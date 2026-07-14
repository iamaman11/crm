use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn write(path: &Path, contents: String) {
    fs::write(path, contents)
        .unwrap_or_else(|error| panic!("cannot write {}: {error}", path.display()));
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

    let domain_path = manifest_dir.join("src/domain.rs");
    let mut domain = fs::read_to_string(&domain_path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", domain_path.display()));
    let old_success = r#"        self.target_party_id = Some(command.target_party_id);
        self.status = ImportRowStatus::Succeeded;
        self.advance(command.occurred_at_unix_nanos)
"#;
    let new_success = r#"        self.target_party_id = Some(command.target_party_id);
        self.last_execution_error_code = None;
        self.status = ImportRowStatus::Succeeded;
        self.advance(command.occurred_at_unix_nanos)
"#;
    if domain.contains(old_success) {
        domain = domain.replacen(old_success, new_success, 1);
    }
    assert!(
        domain.contains("self.last_execution_error_code = None;\n        self.status = ImportRowStatus::Succeeded;"),
        "retryable-success error cleanup patch was not applied"
    );
    write(&domain_path, domain);

    let process_path = repo.join("services/crm-api/tests/import_process_e2e.rs");
    let mut process = fs::read_to_string(&process_path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", process_path.display()));

    if !process.contains("import-source-retryable-") {
        let insertion_marker = r#"    assert_eq!(final_effects.audits, baseline.audits + 1);

    let cross_tenant_job = query_message(
"#;
        let scenario = r#"    assert_eq!(final_effects.audits, baseline.audits + 1);

    // Retryable target failure: a test-only PostgreSQL trigger returns SQLSTATE 40001 for the
    // Party insert. The worker must persist failed_retryable without advancing the checkpoint,
    // then retry the same authoritative row after the trigger is removed and complete exactly once.
    let retry_source_id = format!("import-source-retryable-{suffix}");
    let retry_job_id = format!("import-job-retryable-{suffix}");
    let retry_csv = format!(
        "kind,display_name,external_id\nperson,Retryable Party {suffix},retryable-{suffix}\n"
    )
    .into_bytes();
    upload_source(
        &mut restarted_grpc,
        TENANT_A,
        &retry_source_id,
        &retry_csv,
        "retryable-source",
    )
    .await;
    create_job_from_source(
        &mut restarted_grpc,
        &retry_job_id,
        &retry_source_id,
        cdo::PartialExecutionPolicy::AllValidRows,
        "retryable-job-create",
    )
    .await;
    let retry_rows =
        validate_source(&mut restarted_grpc, &retry_job_id, "retryable-job-validate").await;
    assert_eq!(retry_rows.len(), 1);
    assert_eq!(retry_rows[0].status, cdo::ImportRowStatus::Valid as i32);
    let retry_target_party_id = retry_rows[0]
        .prepared_party
        .as_ref()
        .and_then(|prepared| prepared.party_ref.as_ref())
        .expect("retryable row must prepare target Party identity")
        .party_id
        .clone();
    let retry_job = get_job(&mut restarted_grpc, TENANT_A, &retry_job_id).await;
    let retry_finalized = finalize_validation(
        &mut restarted_grpc,
        &retry_job_id,
        resource_version(&retry_job),
        "retryable-job-finalize",
    )
    .await;

    install_party_retryable_failure_trigger(&admin).await;
    let retry_started = start_execution(
        &mut restarted_grpc,
        &retry_job_id,
        resource_version(&retry_finalized),
        "retryable-job-start",
    )
    .await;
    assert_eq!(retry_started.status, cdo::ImportJobStatus::Executing as i32);

    let failed_retryable = wait_for_retryable_row(&mut restarted_grpc, &retry_job_id).await;
    assert_eq!(
        failed_retryable.status,
        cdo::ImportRowStatus::FailedRetryable as i32
    );
    assert!(failed_retryable.execution_attempts >= 1);
    assert_eq!(
        failed_retryable.last_execution_error_code,
        "CAPABILITY_STORAGE_UNAVAILABLE"
    );
    let failed_retryable_job = get_job(&mut restarted_grpc, TENANT_A, &retry_job_id).await;
    assert_eq!(
        failed_retryable_job.status,
        cdo::ImportJobStatus::Executing as i32
    );
    assert_eq!(failed_retryable_job.checkpoint_row_position, 0);
    assert_eq!(failed_retryable_job.succeeded_rows, 0);
    assert_eq!(party_record_count(&admin, &retry_target_party_id).await, 0);

    drop_party_retryable_failure_trigger(&admin).await;
    let retry_completed = wait_for_completed_job(&mut restarted_grpc, &retry_job_id).await;
    assert_eq!(retry_completed.status, cdo::ImportJobStatus::Completed as i32);
    assert_eq!(retry_completed.checkpoint_row_position, 1);
    assert_eq!(retry_completed.succeeded_rows, 1);
    let retry_completed_rows = list_rows(&mut restarted_grpc, TENANT_A, &retry_job_id).await;
    assert_eq!(retry_completed_rows.len(), 1);
    assert_eq!(
        retry_completed_rows[0].status,
        cdo::ImportRowStatus::Succeeded as i32
    );
    assert!(retry_completed_rows[0].execution_attempts >= 2);
    assert!(retry_completed_rows[0].last_execution_error_code.is_empty());
    assert_eq!(
        retry_completed_rows[0]
            .target_party_ref
            .as_ref()
            .expect("retried row target Party reference")
            .party_id,
        retry_target_party_id
    );
    assert_eq!(party_record_count(&admin, &retry_target_party_id).await, 1);

    let effects_after_retry = party_target_effects(&admin, TENANT_A).await;
    assert_eq!(effects_after_retry.records, baseline.records + 2);
    assert_eq!(effects_after_retry.idempotency, baseline.idempotency + 2);
    assert_eq!(effects_after_retry.events, baseline.events + 2);
    assert_eq!(effects_after_retry.audits, baseline.audits + 2);

    let cross_tenant_job = query_message(
"#;
        assert!(
            process.contains(insertion_marker),
            "retryable process scenario insertion marker is missing"
        );
        process = process.replacen(insertion_marker, scenario, 1);
    }

    if !process.contains("async fn install_party_retryable_failure_trigger") {
        let helper_marker = "async fn install_import_outcome_delay_trigger(admin: &PgPool) {";
        let helpers = r#"async fn install_party_retryable_failure_trigger(admin: &PgPool) {
    admin
        .execute(sqlx::raw_sql(
            r#"
            CREATE OR REPLACE FUNCTION crm.test_fail_party_create_retryable()
            RETURNS trigger
            LANGUAGE plpgsql
            AS $$
            BEGIN
              RAISE EXCEPTION USING
                ERRCODE = '40001',
                MESSAGE = 'synthetic retryable Party create failure';
            END;
            $$;

            DROP TRIGGER IF EXISTS test_fail_party_create_retryable ON crm.records;
            CREATE TRIGGER test_fail_party_create_retryable
            BEFORE INSERT ON crm.records
            FOR EACH ROW
            WHEN (NEW.record_type = 'parties.party')
            EXECUTE FUNCTION crm.test_fail_party_create_retryable();
            "#,
        ))
        .await
        .expect("install test-only retryable Party failure trigger");
}

async fn drop_party_retryable_failure_trigger(admin: &PgPool) {
    admin
        .execute(sqlx::raw_sql(
            r#"
            DROP TRIGGER IF EXISTS test_fail_party_create_retryable ON crm.records;
            DROP FUNCTION IF EXISTS crm.test_fail_party_create_retryable();
            "#,
        ))
        .await
        .expect("remove test-only retryable Party failure trigger");
}

async fn wait_for_retryable_row(
    grpc: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
    job_id: &str,
) -> cdo::ImportRow {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let rows = list_rows(grpc, TENANT_A, job_id).await;
        if let Some(row) = rows
            .into_iter()
            .find(|row| row.status == cdo::ImportRowStatus::FailedRetryable as i32)
        {
            return row;
        }
        assert!(
            Instant::now() < deadline,
            "retryable target failure was not durably recorded"
        );
        sleep(Duration::from_millis(100)).await;
    }
}

"#;
        let position = process
            .find(helper_marker)
            .expect("retryable process helper insertion marker is missing");
        process.insert_str(position, helpers);
    }

    write(&process_path, process);
    fs::remove_file(manifest_dir.join("build.rs"))
        .expect("temporary retryable process acceptance patch must be removable");
}

mod retryable_process {
    include!("import_process_e2e.rs");

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn crm_api_process_persists_retryable_target_failure_without_advancing_checkpoint_and_recovers()
     {
        let Ok(database_url) = std::env::var("DATABASE_URL") else {
            eprintln!(
                "skipping retryable import process acceptance because DATABASE_URL is absent"
            );
            return;
        };
        let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
            .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
        let admin = PgPool::connect(&admin_database_url)
            .await
            .expect("connect retryable process acceptance evidence reader");
        let http = reqwest::Client::builder()
            .build()
            .expect("build retryable process acceptance HTTP client");
        let suffix = format!("{}-{}", std::process::id(), unique_suffix());
        let source_id = format!("import-source-retry-{suffix}");
        let job_id = format!("import-job-retry-{suffix}");
        let csv = format!(
            "kind,display_name,external_id\nperson,Retryable Party {suffix},retry-{suffix}\n"
        )
        .into_bytes();
        let baseline = party_target_effects(&admin, TENANT_A).await;

        let (mut process, http_addr, grpc_addr) = spawn_api(&database_url).await;
        wait_until_ready(&http, &mut process, &http_addr).await;
        let mut grpc = connect_grpc(&grpc_addr).await;

        upload_source(&mut grpc, TENANT_A, &source_id, &csv, "retry-source").await;
        create_job_from_source(
            &mut grpc,
            &job_id,
            &source_id,
            cdo::PartialExecutionPolicy::AllValidRows,
            "retry-job-create",
        )
        .await;
        let rows = validate_source(&mut grpc, &job_id, "retry-job-validate").await;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, cdo::ImportRowStatus::Valid as i32);
        let target_party_id = rows[0]
            .prepared_party
            .as_ref()
            .and_then(|prepared| prepared.party_ref.as_ref())
            .expect("retryable import row must prepare target Party identity")
            .party_id
            .clone();
        let job = get_job(&mut grpc, TENANT_A, &job_id).await;
        let finalized = finalize_validation(
            &mut grpc,
            &job_id,
            resource_version(&job),
            "retry-job-finalize",
        )
        .await;

        install_party_retryable_failure_trigger(&admin).await;
        let started = start_execution(
            &mut grpc,
            &job_id,
            resource_version(&finalized),
            "retry-job-start",
        )
        .await;
        assert_eq!(started.status, cdo::ImportJobStatus::Executing as i32);

        let failed_retryable = wait_for_retryable_row(&mut grpc, &job_id).await;
        assert_eq!(
            failed_retryable.status,
            cdo::ImportRowStatus::FailedRetryable as i32
        );
        assert!(failed_retryable.execution_attempts >= 1);
        assert!(!failed_retryable.last_execution_error_code.is_empty());
        let failed_attempts = failed_retryable.execution_attempts;
        let failed_job = get_job(&mut grpc, TENANT_A, &job_id).await;
        assert_eq!(failed_job.status, cdo::ImportJobStatus::Executing as i32);
        assert_eq!(failed_job.checkpoint_row_position, 0);
        assert_eq!(failed_job.succeeded_rows, 0);

        process
            .kill()
            .await
            .expect("stop crm-api after durable retryable import failure");
        let _ = process.wait().await;
        drop_party_retryable_failure_trigger(&admin).await;
        assert_eq!(party_record_count(&admin, &target_party_id).await, 0);
        assert_eq!(party_target_effects(&admin, TENANT_A).await, baseline);

        let (mut recovered, recovered_http_addr, recovered_grpc_addr) =
            spawn_api(&database_url).await;
        wait_until_ready(&http, &mut recovered, &recovered_http_addr).await;
        let mut recovered_grpc = connect_grpc(&recovered_grpc_addr).await;
        let completed = wait_for_completed_job(&mut recovered_grpc, &job_id).await;
        assert_eq!(completed.status, cdo::ImportJobStatus::Completed as i32);
        assert_eq!(completed.succeeded_rows, 1);
        assert_eq!(completed.checkpoint_row_position, 1);
        let completed_rows = list_rows(&mut recovered_grpc, TENANT_A, &job_id).await;
        assert_eq!(completed_rows.len(), 1);
        assert_eq!(
            completed_rows[0].status,
            cdo::ImportRowStatus::Succeeded as i32
        );
        assert_eq!(completed_rows[0].execution_attempts, failed_attempts + 1);
        assert!(completed_rows[0].last_execution_error_code.is_empty());
        assert_eq!(party_record_count(&admin, &target_party_id).await, 1);

        let recovered_effects = party_target_effects(&admin, TENANT_A).await;
        assert_eq!(recovered_effects.records, baseline.records + 1);
        assert_eq!(recovered_effects.idempotency, baseline.idempotency + 1);
        assert_eq!(recovered_effects.events, baseline.events + 1);
        assert_eq!(recovered_effects.audits, baseline.audits + 1);

        send_sigint(&recovered).await;
        let exit = timeout(Duration::from_secs(15), recovered.wait())
            .await
            .expect("recovered crm-api must stop within graceful-shutdown budget")
            .expect("wait for recovered retryable process acceptance crm-api process");
        assert!(
            exit.success(),
            "recovered crm-api exited unsuccessfully: {exit}"
        );
    }

    async fn install_party_retryable_failure_trigger(admin: &PgPool) {
        admin
            .execute(sqlx::raw_sql(
                r#"
                CREATE OR REPLACE FUNCTION crm.test_fail_retryable_party_target()
                RETURNS trigger
                LANGUAGE plpgsql
                AS $$
                BEGIN
                  RAISE EXCEPTION 'synthetic retryable Party target failure'
                    USING ERRCODE = '40001';
                END;
                $$;

                DROP TRIGGER IF EXISTS test_fail_retryable_party_target ON crm.records;
                CREATE TRIGGER test_fail_retryable_party_target
                BEFORE INSERT ON crm.records
                FOR EACH ROW
                WHEN (
                  NEW.tenant_id = 'tenant-a'
                  AND NEW.record_type = 'parties.party'
                )
                EXECUTE FUNCTION crm.test_fail_retryable_party_target();
                "#,
            ))
            .await
            .expect("install test-only retryable Party target failure trigger");
    }

    async fn drop_party_retryable_failure_trigger(admin: &PgPool) {
        admin
            .execute(sqlx::raw_sql(
                r#"
                DROP TRIGGER IF EXISTS test_fail_retryable_party_target ON crm.records;
                DROP FUNCTION IF EXISTS crm.test_fail_retryable_party_target();
                "#,
            ))
            .await
            .expect("remove test-only retryable Party target failure trigger");
    }

    async fn wait_for_retryable_row(
        grpc: &mut ApplicationGatewayServiceClient<tonic::transport::Channel>,
        job_id: &str,
    ) -> cdo::ImportRow {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let rows = list_rows(grpc, TENANT_A, job_id).await;
            if let Some(row) = rows.into_iter().find(|row| {
                row.status == cdo::ImportRowStatus::FailedRetryable as i32
                    && row.execution_attempts > 0
            }) {
                return row;
            }
            assert!(
                Instant::now() < deadline,
                "import row did not persist FailedRetryable without checkpoint advancement"
            );
            sleep(Duration::from_millis(100)).await;
        }
    }
}

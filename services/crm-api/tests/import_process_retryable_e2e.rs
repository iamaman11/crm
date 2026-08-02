#[path = "support/generic_worker_conformance.rs"]
mod worker_conformance;

mod retryable_process {
    include!("import_process_e2e.rs");

    use super::worker_conformance::WorkerConformanceSuite;

    const CONTENTION_LOCK_KEY: i64 = 9_164_220_616;

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
        admin
            .execute(sqlx::raw_sql(include_str!(
                "../../../database/tests/0005_party_adapter.sql"
            )))
            .await
            .expect("publish Party capability registry fixture");
        admin
            .execute(sqlx::raw_sql(include_str!(
                "../../../database/tests/0012_customer_data_operations_adapter.sql"
            )))
            .await
            .expect("publish customer-data operations registry and worker fixture");

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
        let conformance =
            WorkerConformanceSuite::new("crm.customer-data-operations/party-import-execution");

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
        assert_eq!(failed_job.succeeded_rows, 0);
        let failed_effects = party_target_effects(&admin, TENANT_A).await;
        conformance.assert_retryable_failure_preserves_progress(
            &0_u32,
            &failed_job.checkpoint_row_position,
            &baseline,
            &failed_effects,
        );

        process
            .kill()
            .await
            .expect("stop crm-api after durable retryable import failure");
        let _ = process.wait().await;
        drop_party_retryable_failure_trigger(&admin).await;
        assert_eq!(party_record_count(&admin, &target_party_id).await, 0);
        conformance.assert_no_side_effects(
            "retryable failure before restart",
            &baseline,
            &party_target_effects(&admin, TENANT_A).await,
        );

        install_party_contention_barrier(&admin).await;
        let mut contention_lock = admin
            .acquire()
            .await
            .expect("acquire dedicated contention-lock connection");
        sqlx::query("SELECT pg_advisory_lock($1)")
            .bind(CONTENTION_LOCK_KEY)
            .execute(&mut *contention_lock)
            .await
            .expect("hold Party target contention barrier");

        let (mut recovered_a, recovered_a_http_addr, recovered_a_grpc_addr) =
            spawn_api(&database_url).await;
        let (mut recovered_b, recovered_b_http_addr, _recovered_b_grpc_addr) =
            spawn_api(&database_url).await;
        wait_until_ready(&http, &mut recovered_a, &recovered_a_http_addr).await;
        wait_until_ready(&http, &mut recovered_b, &recovered_b_http_addr).await;
        wait_for_party_contention_waiters(&admin, 2).await;

        let unlocked: bool = sqlx::query_scalar("SELECT pg_advisory_unlock($1)")
            .bind(CONTENTION_LOCK_KEY)
            .fetch_one(&mut *contention_lock)
            .await
            .expect("release Party target contention barrier");
        assert!(unlocked, "dedicated contention lock must be released");
        drop(contention_lock);

        let mut recovered_grpc = connect_grpc(&recovered_a_grpc_addr).await;
        let completed = wait_for_completed_job(&mut recovered_grpc, &job_id).await;
        assert_eq!(completed.status, cdo::ImportJobStatus::Completed as i32);
        assert_eq!(completed.succeeded_rows, 1);
        assert_eq!(completed.checkpoint_row_position, 1);

        drop_party_contention_barrier(&admin).await;
        let completed_rows = list_rows(&mut recovered_grpc, TENANT_A, &job_id).await;
        assert_eq!(completed_rows.len(), 1);
        assert_eq!(
            completed_rows[0].status,
            cdo::ImportRowStatus::Succeeded as i32
        );
        assert!(completed_rows[0].execution_attempts > failed_attempts);
        assert!(completed_rows[0].last_execution_error_code.is_empty());
        assert_eq!(party_record_count(&admin, &target_party_id).await, 1);

        let recovered_effects = party_target_effects(&admin, TENANT_A).await;
        conformance.assert_exact_recovery(
            &(
                baseline.records + 1,
                baseline.idempotency + 1,
                baseline.events + 1,
                baseline.audits + 1,
            ),
            &(
                recovered_effects.records,
                recovered_effects.idempotency,
                recovered_effects.events,
                recovered_effects.audits,
            ),
        );

        sleep(Duration::from_millis(500)).await;
        let post_contention_replay_effects = party_target_effects(&admin, TENANT_A).await;
        conformance.assert_no_side_effects(
            "post-contention completed replay",
            &recovered_effects,
            &post_contention_replay_effects,
        );

        send_sigint(&recovered_a).await;
        send_sigint(&recovered_b).await;
        let exit_a = timeout(Duration::from_secs(15), recovered_a.wait())
            .await
            .expect("competing recovered executor A must stop within graceful-shutdown budget")
            .expect("wait for competing recovered executor A");
        let exit_b = timeout(Duration::from_secs(15), recovered_b.wait())
            .await
            .expect("competing recovered executor B must stop within graceful-shutdown budget")
            .expect("wait for competing recovered executor B");
        assert!(
            exit_a.success(),
            "competing recovered executor A exited unsuccessfully: {exit_a}"
        );
        assert!(
            exit_b.success(),
            "competing recovered executor B exited unsuccessfully: {exit_b}"
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

    async fn install_party_contention_barrier(admin: &PgPool) {
        admin
            .execute(sqlx::raw_sql(
                r#"
                CREATE OR REPLACE FUNCTION crm.test_block_party_target_for_contention()
                RETURNS trigger
                LANGUAGE plpgsql
                AS $$
                BEGIN
                  PERFORM pg_advisory_xact_lock(9164220616);
                  RETURN NEW;
                END;
                $$;

                DROP TRIGGER IF EXISTS test_block_party_target_for_contention ON crm.records;
                CREATE TRIGGER test_block_party_target_for_contention
                BEFORE INSERT ON crm.records
                FOR EACH ROW
                WHEN (
                  NEW.tenant_id = 'tenant-a'
                  AND NEW.record_type = 'parties.party'
                )
                EXECUTE FUNCTION crm.test_block_party_target_for_contention();
                "#,
            ))
            .await
            .expect("install test-only Party target contention barrier");
    }

    async fn drop_party_contention_barrier(admin: &PgPool) {
        admin
            .execute(sqlx::raw_sql(
                r#"
                DROP TRIGGER IF EXISTS test_block_party_target_for_contention ON crm.records;
                DROP FUNCTION IF EXISTS crm.test_block_party_target_for_contention();
                "#,
            ))
            .await
            .expect("remove test-only Party target contention barrier");
    }

    async fn wait_for_party_contention_waiters(admin: &PgPool, expected: i64) {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let (direct_party_waiters, transitive_executor_waiters): (i64, i64) =
                sqlx::query_as(
                    r#"
                    WITH RECURSIVE direct_party_waiters AS (
                      SELECT
                        activity.pid AS waiter_pid,
                        blocker.pid AS root_blocker_pid
                      FROM pg_stat_activity AS activity
                      CROSS JOIN LATERAL
                        unnest(pg_blocking_pids(activity.pid)) AS blocker(pid)
                      WHERE activity.datname = current_database()
                        AND activity.wait_event_type = 'Lock'
                        AND activity.wait_event = 'advisory'
                        AND activity.query ILIKE '%crm.records%'
                    ),
                    blocking_chain(waiter_pid, blocker_pid) AS (
                      SELECT
                        activity.pid,
                        blocker.pid
                      FROM pg_stat_activity AS activity
                      CROSS JOIN LATERAL
                        unnest(pg_blocking_pids(activity.pid)) AS blocker(pid)
                      WHERE activity.datname = current_database()
                      UNION
                      SELECT
                        chain.waiter_pid,
                        blocker.pid
                      FROM blocking_chain AS chain
                      CROSS JOIN LATERAL
                        unnest(pg_blocking_pids(chain.blocker_pid)) AS blocker(pid)
                    )
                    SELECT
                      (SELECT count(*)::bigint FROM direct_party_waiters),
                      (
                        SELECT count(DISTINCT chain.waiter_pid)::bigint
                        FROM blocking_chain AS chain
                        WHERE chain.blocker_pid IN (
                          SELECT root_blocker_pid FROM direct_party_waiters
                        )
                      )
                    "#,
                )
                .fetch_one(admin)
                .await
                .expect("observe Party target contention chain");
            if direct_party_waiters >= 1 && transitive_executor_waiters >= expected {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "expected one direct Party waiter and {expected} competing executors in the same blocking chain, observed {direct_party_waiters} direct and {transitive_executor_waiters} transitive waiters"
            );
            sleep(Duration::from_millis(50)).await;
        }
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

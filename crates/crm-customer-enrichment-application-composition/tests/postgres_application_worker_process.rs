mod process {
    include!("postgres_application_orchestration_process.rs");

    use crm_application_composition::{
        ActivationGatedBackgroundWorker, ModuleActivationPort, TenantBackgroundWorker,
    };
    use crm_customer_enrichment_application_composition::{
        CustomerEnrichmentPartyApplicationWorker, PARTY_DISPLAY_NAME_APPLICATION_PROJECTION_ID,
    };
    use crm_module_sdk::ModuleId;
    use std::collections::BTreeSet;

    const NOW: i64 = 60_000_000;

    #[derive(Debug, Clone)]
    struct StoreActivation(PostgresDataStore);

    impl ModuleActivationPort for StoreActivation {
        fn is_active<'a>(
            &'a self,
            tenant_id: &'a TenantId,
            module_id: &'a ModuleId,
        ) -> PortFuture<'a, Result<bool, SdkError>> {
            Box::pin(async move { self.0.is_module_active(tenant_id, module_id).await })
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn accepted_review_worker_checkpoints_and_applies_exactly_once() {
        let Ok(url) = std::env::var("DATABASE_URL") else {
            return;
        };
        let admin_url = std::env::var("ADMIN_DATABASE_URL").unwrap();
        let store = PostgresDataStore::connect(&url, 6).await.unwrap();
        let admin = PgPool::connect(&admin_url).await.unwrap();
        let (suggestion, review) = evidence();
        seed(
            &store,
            suggestion_record_ref(suggestion.suggestion_id().as_str()).unwrap(),
            suggestion_persisted_payload(&suggestion).unwrap(),
            "customer_enrichment.suggestion.materialized",
            "crm.customer_enrichment.v1.SuggestionMaterializedEvent",
            wire::SuggestionMaterializedEvent {
                suggestion: Some(suggestion_to_wire(&suggestion, None, 30).unwrap()),
            },
            "worker-suggestion",
            30_000_000,
        )
        .await;
        seed(
            &store,
            review_decision_record_ref(&review).unwrap(),
            review_decision_persisted_payload(&review).unwrap(),
            "customer_enrichment.suggestion.reviewed",
            "crm.customer_enrichment.v1.SuggestionReviewedEvent",
            wire::SuggestionReviewedEvent {
                suggestion: Some(suggestion_to_wire(&suggestion, Some(&review), 40).unwrap()),
                review_decision: Some(review_decision_to_wire(&review).unwrap()),
            },
            "worker-review",
            40_000_000,
        )
        .await;
        store
            .bootstrap_activate_published_modules(
                &BTreeSet::from([TenantId::try_new(TENANT).unwrap()]),
                &BTreeSet::from([MODULE_ID.to_owned()]),
            )
            .await
            .unwrap();

        let policy = Arc::new(AllowedPolicy::default());
        let owner = Arc::new(ReplayOwner::default());
        let orchestrator = Arc::new(
            CustomerEnrichmentPartyApplicationOrchestrator::postgres(
                store.clone(),
                policy.clone(),
                owner.clone(),
                Arc::new(FixedClock::new(NOW)),
            )
            .unwrap(),
        );
        let worker = Arc::new(
            CustomerEnrichmentPartyApplicationWorker::new(
                store.clone(),
                orchestrator,
                ActorId::try_new(ACTOR).unwrap(),
            )
            .unwrap(),
        );
        let gated = ActivationGatedBackgroundWorker::new(
            Arc::new(StoreActivation(store.clone())),
            ModuleId::try_new(MODULE_ID).unwrap(),
            worker.clone(),
        );

        set_status(&admin, "suspended").await;
        gated
            .run_tenant_cycle(TenantId::try_new(TENANT).unwrap(), NOW)
            .await
            .unwrap();
        assert_eq!(attempt_count(&admin).await, 0);
        assert!(
            store
                .projection_checkpoint(
                    &TenantId::try_new(TENANT).unwrap(),
                    PARTY_DISPLAY_NAME_APPLICATION_PROJECTION_ID,
                )
                .await
                .unwrap()
                .is_none()
        );

        set_status(&admin, "active").await;
        let cycle = worker
            .run_cycle(TenantId::try_new(TENANT).unwrap(), NOW)
            .await
            .unwrap();
        assert_eq!(cycle.reviewed_events, 1);
        assert_eq!(cycle.accepted_events, 1);
        assert_eq!(cycle.skipped_events, 0);
        assert_eq!(attempt_count(&admin).await, 1);
        assert_eq!(attempt_version(&admin).await, 2);
        assert_eq!(policy.0.lock().unwrap().len(), 1);
        assert_eq!(owner.0.lock().unwrap().len(), 1);
        assert_eq!(
            store
                .projection_checkpoint(
                    &TenantId::try_new(TENANT).unwrap(),
                    PARTY_DISPLAY_NAME_APPLICATION_PROJECTION_ID,
                )
                .await
                .unwrap()
                .unwrap()
                .applied_event_count,
            1
        );

        let replay = worker
            .run_cycle(TenantId::try_new(TENANT).unwrap(), NOW)
            .await
            .unwrap();
        assert_eq!(replay.reviewed_events, 0);
        assert_eq!(attempt_count(&admin).await, 1);
        assert_eq!(owner.0.lock().unwrap().len(), 1);

        set_status(&admin, "uninstalling").await;
        gated
            .run_tenant_cycle(TenantId::try_new(TENANT).unwrap(), NOW)
            .await
            .unwrap();
        assert_eq!(attempt_count(&admin).await, 1);
    }

    async fn set_status(admin: &PgPool, status: &str) {
        let request_id = format!("application-worker-status-{status}");
        sqlx::query(
            r#"
            WITH current_installation AS (
              SELECT last_business_transaction_id
              FROM crm.module_installations
              WHERE tenant_id = $2 AND module_id = $3
            ),
            context AS (
              SELECT
                set_config('app.tenant_id', $2, true),
                set_config('app.actor_id', $4, true),
                set_config('app.request_id', $5, true),
                set_config('app.capability_id', $6, true),
                set_config('app.capability_version', '1.0.0', true),
                set_config(
                  'app.business_transaction_id',
                  current_installation.last_business_transaction_id,
                  true
                )
              FROM current_installation
            )
            UPDATE crm.module_installations
            SET status = $1,
                generation = generation + 1,
                updated_at = clock_timestamp()
            FROM context
            WHERE tenant_id = $2 AND module_id = $3
            "#,
        )
        .bind(status)
        .bind(TENANT)
        .bind(MODULE_ID)
        .bind(ACTOR)
        .bind(request_id)
        .bind(SEED)
        .execute(admin)
        .await
        .unwrap();
    }

    async fn attempt_count(admin: &PgPool) -> i64 {
        sqlx::query_scalar(
            "SELECT count(*)::bigint FROM crm.records WHERE tenant_id = $1 AND record_type = 'customer_enrichment.application_attempt'",
        )
        .bind(TENANT)
        .fetch_one(admin)
        .await
        .unwrap()
    }

    async fn attempt_version(admin: &PgPool) -> i64 {
        sqlx::query_scalar(
            "SELECT version::bigint FROM crm.records WHERE tenant_id = $1 AND record_type = 'customer_enrichment.application_attempt'",
        )
        .bind(TENANT)
        .fetch_one(admin)
        .await
        .unwrap()
    }
}

from pathlib import Path


def replace_once(path: Path, old: str, new: str) -> None:
    text = path.read_text()
    if old not in text:
        raise SystemExit(f"marker not found in {path}: {old[:160]!r}")
    path.write_text(text.replace(old, new, 1))


persistence = Path(
    "crates/crm-customer-enrichment-provider-process-composition/src/conflict_persistence.rs"
)
replace_once(
    persistence,
    '''    pub async fn unresolved_for_request(
        &self,
        tenant_id: TenantId,
        request_id: RecordId,
    ) -> Result<Option<ProviderResponseConflict>, SdkError> {
        let page = self
''',
    '''    pub async fn recovery_conflict_for_request(
        &self,
        tenant_id: TenantId,
        request_id: RecordId,
    ) -> Result<Option<ProviderResponseConflict>, SdkError> {
        let page = self
''',
)
replace_once(
    persistence,
    '''        let mut unresolved = None;
        for snapshot in page.records {
''',
    '''        let mut found = None;
        for snapshot in page.records {
''',
)
replace_once(
    persistence,
    '''            if conflict.resolution().is_none() && unresolved.replace(conflict).is_some() {
                return Err(conflict_state_invalid(
                    "request has more than one unresolved provider-response conflict",
                ));
            }
        }
        Ok(unresolved)
    }
}
''',
    '''            if found.replace(conflict).is_some() {
                return Err(conflict_state_invalid(
                    "request has more than one provider-response conflict for process recovery",
                ));
            }
        }
        Ok(found)
    }

    pub async fn unresolved_for_request(
        &self,
        tenant_id: TenantId,
        request_id: RecordId,
    ) -> Result<Option<ProviderResponseConflict>, SdkError> {
        Ok(self
            .recovery_conflict_for_request(tenant_id, request_id)
            .await?
            .filter(|conflict| conflict.resolution().is_none()))
    }
}
''',
)

worker = Path("crates/crm-customer-enrichment-provider-process-composition/src/worker.rs")
replace_once(
    worker,
    '''use crm_customer_enrichment::{
    ENRICHMENT_REQUEST_RECORD_TYPE, EnrichmentRequest, PartySnapshot, ProviderProfileVersion,
};
''',
    '''use crm_customer_enrichment::{
    ENRICHMENT_REQUEST_RECORD_TYPE, EnrichmentRequest, PartySnapshot, ProviderProfileVersion,
    ProviderResponseConflictDecision,
};
''',
)
replace_once(
    worker,
    '''    pub response_replays: u32,
}
''',
    '''    pub response_replays: u32,
    pub retained_first_receipts: u32,
}
''',
)
replace_once(
    worker,
    '''                        Ok(DeliveryDisposition::Skipped) => {
                            cycle.skipped = cycle.skipped.saturating_add(1);
                        }
''',
    '''                        Ok(DeliveryDisposition::Skipped) => {
                            cycle.skipped = cycle.skipped.saturating_add(1);
                        }
                        Ok(DeliveryDisposition::RetainedFirstReceipt) => {
                            cycle.retained_first_receipts =
                                cycle.retained_first_receipts.saturating_add(1);
                        }
''',
)
replace_once(
    worker,
    '''        if let Some(conflict) = self
            .conflict_store
            .unresolved_for_request(tenant_id.clone(), request_id.clone())
            .await?
        {
            return Err(unresolved_provider_conflict(
                conflict.conflict_id().as_str(),
            ));
        }
''',
    '''        if let Some(conflict) = self
            .conflict_store
            .recovery_conflict_for_request(tenant_id.clone(), request_id.clone())
            .await?
        {
            let Some(resolution) = conflict.resolution() else {
                return Err(unresolved_provider_conflict(
                    conflict.conflict_id().as_str(),
                ));
            };
            return match resolution.decision() {
                ProviderResponseConflictDecision::RetainFirstReceipt => {
                    Ok(DeliveryDisposition::RetainedFirstReceipt)
                }
                ProviderResponseConflictDecision::RejectRequest => Err(
                    reject_request_resolution_pending(conflict.conflict_id().as_str()),
                ),
            };
        }
''',
)
replace_once(
    worker,
    '''enum DeliveryDisposition {
    Executed(Box<ProviderDispatchWorkerResult>),
    Skipped,
}
''',
    '''enum DeliveryDisposition {
    Executed(Box<ProviderDispatchWorkerResult>),
    Skipped,
    RetainedFirstReceipt,
}
''',
)
replace_once(
    worker,
    '''fn unresolved_provider_conflict(conflict_id: &str) -> SdkError {
''',
    '''fn reject_request_resolution_pending(conflict_id: &str) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_REJECT_TRANSITION_PENDING",
        crm_module_sdk::ErrorCategory::Conflict,
        true,
        "The approved provider-response conflict rejection has not reached terminal request state.",
    )
    .with_internal_reference(format!("provider_response_conflict_id={conflict_id}"))
}

fn unresolved_provider_conflict(conflict_id: &str) -> SdkError {
''',
)

test = Path(
    "crates/crm-customer-enrichment-provider-process-composition/tests/postgres_conflict_process_hold.rs"
)
replace_once(
    test,
    '''    PartySnapshot, ProviderProfileDraft, ProviderProfileVersion, ProviderResponseConflictDraft,
    ProviderResponseReceiptId, RawPayloadPolicy, RequestPolicyEvidence, TargetField,
    TargetSnapshot,
''',
    '''    PartySnapshot, ProviderProfileDraft, ProviderProfileVersion, ProviderResponseConflictDecision,
    ProviderResponseConflictDraft, ProviderResponseConflictResolutionPolicyDecision,
    ProviderResponseConflictResolutionPolicyPort, ProviderResponseConflictResolutionPolicyRequest,
    ProviderResponseReceiptId, RawPayloadPolicy, RequestPolicyEvidence, TargetField, TargetSnapshot,
''',
)
replace_once(
    test,
    '''    CustomerEnrichmentProviderProcessWorker, PROVIDER_PROCESS_PROJECTION_ID,
    ProviderDispatchExecutorPort, ProviderDispatchSourceDisposition, ProviderDispatchSourcePort,
    ProviderDispatchSourceSnapshot,
''',
    '''    CustomerEnrichmentProviderProcessWorker, PROVIDER_PROCESS_PROJECTION_ID,
    PostgresProviderResponseConflictResolutionExecutor, PostgresProviderResponseConflictStore,
    ProviderDispatchExecutorPort, ProviderDispatchSourceDisposition, ProviderDispatchSourcePort,
    ProviderDispatchSourceSnapshot, ProviderResponseConflictResolutionCommand,
''',
)
replace_once(
    test,
    '''    assert_eq!(conflict_count(&admin).await, 1);
    assert_eq!(conflict_relationship_count(&admin).await, 1);
    assert_eq!(evidence_counts(&admin).await, baseline);
}
''',
    '''    assert_eq!(conflict_count(&admin).await, 1);
    assert_eq!(conflict_relationship_count(&admin).await, 1);
    assert_eq!(evidence_counts(&admin).await, baseline);

    let conflict_store = PostgresProviderResponseConflictStore::new(store.clone());
    let conflict = conflict_store
        .recovery_conflict_for_request(
            TenantId::try_new(TENANT_ID).unwrap(),
            RecordId::try_new(fixture.request.request_id().as_str().to_owned()).unwrap(),
        )
        .await
        .expect("load held conflict for governed resolution")
        .expect("held conflict exists");
    let first_receipt_id = conflict.first_receipt_id().as_str().to_owned();
    let resolver = PostgresProviderResponseConflictResolutionExecutor::new(
        store.clone(),
        Arc::new(AllowRetainFirstPolicy),
    );
    let resolution = resolver
        .execute(ProviderResponseConflictResolutionCommand {
            tenant_id: TenantId::try_new(TENANT_ID).unwrap(),
            conflict_id: RecordId::try_new(conflict.conflict_id().as_str().to_owned()).unwrap(),
            actor_id: ActorId::try_new(ACTOR_ID).unwrap(),
            decision: ProviderResponseConflictDecision::RetainFirstReceipt,
            safe_reason_code: "retain-first-receipt".to_owned(),
            approval_evidence_reference: "approval/provider-conflict/retain-first".to_owned(),
            causation_id: CausationId::try_new("provider-conflict-retain-first-command").unwrap(),
            correlation_id: CorrelationId::try_new("provider-conflict-retain-first-correlation")
                .unwrap(),
            trace_id: TraceId::try_new("provider-conflict-retain-first-trace").unwrap(),
            resolved_at_unix_ms: 70,
        })
        .await
        .expect("persist governed retain-first resolution");
    assert!(!resolution.replayed);
    assert_eq!(
        resolution
            .conflict
            .resolution()
            .expect("resolution exists")
            .decision(),
        ProviderResponseConflictDecision::RetainFirstReceipt
    );
    assert_eq!(resolution.conflict.first_receipt_id().as_str(), first_receipt_id);
    let resolved_baseline = evidence_counts(&admin).await;

    let resumed = restarted
        .run_cycle(TenantId::try_new(TENANT_ID).unwrap(), 80_000_000)
        .await
        .expect("retain-first resolution must resume the held checkpoint");
    assert_eq!(resumed.created_events, 1);
    assert_eq!(resumed.retained_first_receipts, 1);
    assert_eq!(resumed.dispatched, 0);
    assert_eq!(source_calls.load(Ordering::SeqCst), 1);
    assert_eq!(executor_calls.load(Ordering::SeqCst), 1);
    assert!(
        ProjectionStore::projection_checkpoint(
            &store,
            TenantId::try_new(TENANT_ID).unwrap(),
            PROVIDER_PROCESS_PROJECTION_ID.to_owned(),
        )
        .await
        .expect("read resumed provider checkpoint")
        .is_some()
    );
    assert_eq!(evidence_counts(&admin).await, resolved_baseline);

    let resolved = conflict_store
        .recovery_conflict_for_request(
            TenantId::try_new(TENANT_ID).unwrap(),
            RecordId::try_new(fixture.request.request_id().as_str().to_owned()).unwrap(),
        )
        .await
        .expect("reload resolved conflict")
        .expect("resolved conflict remains linked");
    assert_eq!(resolved.first_receipt_id().as_str(), first_receipt_id);
    assert_eq!(
        resolved
            .resolution()
            .expect("resolved conflict has resolution")
            .decision(),
        ProviderResponseConflictDecision::RetainFirstReceipt
    );

    let no_op = restarted
        .run_cycle(TenantId::try_new(TENANT_ID).unwrap(), 90_000_000)
        .await
        .expect("checkpoint replay must be a no-op");
    assert_eq!(no_op.created_events, 0);
    assert_eq!(no_op.retained_first_receipts, 0);
    assert_eq!(source_calls.load(Ordering::SeqCst), 1);
    assert_eq!(executor_calls.load(Ordering::SeqCst), 1);
    assert_eq!(evidence_counts(&admin).await, resolved_baseline);
}

#[derive(Clone)]
struct AllowRetainFirstPolicy;

impl ProviderResponseConflictResolutionPolicyPort for AllowRetainFirstPolicy {
    fn evaluate<'a>(
        &'a self,
        request: ProviderResponseConflictResolutionPolicyRequest,
    ) -> PortFuture<'a, Result<ProviderResponseConflictResolutionPolicyDecision, SdkError>> {
        Box::pin(async move {
            assert_eq!(
                request.decision,
                ProviderResponseConflictDecision::RetainFirstReceipt
            );
            Ok(ProviderResponseConflictResolutionPolicyDecision::Allowed {
                policy_version: "provider-conflict-policy-v1".to_owned(),
            })
        })
    }
}
''',
)

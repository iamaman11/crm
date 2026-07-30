from pathlib import Path


def replace_exact(path: str, old: str, new: str, label: str, expected: int = 1) -> None:
    target = Path(path)
    text = target.read_text(encoding="utf-8")
    count = text.count(old)
    if count != expected:
        raise SystemExit(f"{label}: found {count}, expected {expected}")
    target.write_text(text.replace(old, new), encoding="utf-8")


application = "crates/crm-customer-privacy-application/src/access_export.rs"
replace_exact(
    application,
    '''    pub initiating_capability_id: CapabilityId,
    pub initiating_capability_version: CapabilityVersion,
''',
    '''    /// Registered public capability that initiated the trusted internal orchestration.
    pub initiating_capability_id: CapabilityId,
    /// Registered public capability version preserved for audit provenance.
    pub initiating_capability_version: CapabilityVersion,
''',
    "invocation provenance documentation",
)
replace_exact(
    application,
    '''    pub actor_id: ActorId,
    pub correlation_id: CorrelationId,
    pub trace_id: TraceId,
    pub prepared_at_unix_nanos: i64,
''',
    '''    pub actor_id: ActorId,
    pub correlation_id: CorrelationId,
    pub trace_id: TraceId,
    pub initiating_capability_id: CapabilityId,
    pub initiating_capability_version: CapabilityVersion,
    pub prepared_at_unix_nanos: i64,
''',
    "target provenance fields",
)
replace_exact(
    application,
    '''                actor_id: invocation.actor_id.clone(),
                correlation_id: invocation.correlation_id.clone(),
                trace_id: invocation.trace_id.clone(),
                prepared_at_unix_nanos: prepared.prepared_at_unix_nanos(),
''',
    '''                actor_id: invocation.actor_id.clone(),
                correlation_id: invocation.correlation_id.clone(),
                trace_id: invocation.trace_id.clone(),
                initiating_capability_id: invocation.initiating_capability_id.clone(),
                initiating_capability_version: invocation.initiating_capability_version.clone(),
                prepared_at_unix_nanos: prepared.prepared_at_unix_nanos(),
''',
    "forward target provenance",
)
replace_exact(
    application,
    '''    if invocation.initiating_capability_id.as_str() != ACCESS_EXPORT_REQUEST_CAPABILITY
        || invocation.initiating_capability_version.as_str() != ACCESS_EXPORT_CAPABILITY_VERSION
        || ACCESS_EXPORT_REQUEST_COORDINATE
            != format!(
                "{}@{}",
                invocation.initiating_capability_id, invocation.initiating_capability_version
            )
    {
        return Err(configuration_invalid(
            "access export invocation does not use the frozen internal coordinate",
        ));
    }
''',
    '''    if ACCESS_EXPORT_REQUEST_COORDINATE
        != format!(
            "{ACCESS_EXPORT_REQUEST_CAPABILITY}@{ACCESS_EXPORT_CAPABILITY_VERSION}"
        )
    {
        return Err(configuration_invalid(
            "access export service does not use the frozen internal coordinate",
        ));
    }
    if invocation.initiating_capability_id.as_str() == ACCESS_EXPORT_REQUEST_CAPABILITY {
        return Err(configuration_invalid(
            "the private access export coordinate cannot replace registered audit provenance",
        ));
    }
''',
    "separate internal coordinate and registered provenance",
)

runtime = "crates/crm-application-runtime/src/customer_privacy_access_export.rs"
replace_exact(
    runtime,
    '''                    actor_id: request.actor_id,
                    correlation_id: request.correlation_id,
                    trace_id: request.trace_id,
                    prepared_at_unix_nanos: request.prepared_at_unix_nanos,
''',
    '''                    actor_id: request.actor_id,
                    correlation_id: request.correlation_id,
                    trace_id: request.trace_id,
                    initiating_capability_id: request.initiating_capability_id,
                    initiating_capability_version: request.initiating_capability_version,
                    prepared_at_unix_nanos: request.prepared_at_unix_nanos,
''',
    "runtime provenance forwarding",
)

cdo = "crates/crm-customer-data-operations-execution-composition/src/privacy_export.rs"
replace_exact(
    cdo,
    '''    pub actor_id: ActorId,
    pub correlation_id: CorrelationId,
    pub trace_id: TraceId,
    pub prepared_at_unix_nanos: i64,
''',
    '''    pub actor_id: ActorId,
    pub correlation_id: CorrelationId,
    pub trace_id: TraceId,
    pub initiating_capability_id: CapabilityId,
    pub initiating_capability_version: CapabilityVersion,
    pub prepared_at_unix_nanos: i64,
''',
    "CDO request provenance fields",
)
replace_exact(
    cdo,
    '''        IdempotencyKey::try_new(request.target_idempotency_key.clone())
            .map_err(configuration_error)?;
''',
    '''        IdempotencyKey::try_new(request.target_idempotency_key.clone())
            .map_err(configuration_error)?;
        if request.initiating_capability_id.as_str() == PRIVACY_EXPORT_REQUEST_CAPABILITY {
            return Err(configuration_error(
                "the private privacy-export coordinate cannot replace registered audit provenance",
            ));
        }
''',
    "CDO provenance validation",
)
replace_exact(
    cdo,
    '''            capability_id: CapabilityId::try_new(PRIVACY_EXPORT_REQUEST_CAPABILITY)
                .map_err(configuration_error)?,
            capability_version: CapabilityVersion::try_new(PRIVACY_EXPORT_REQUEST_VERSION)
                .map_err(configuration_error)?,
''',
    '''            capability_id: request.initiating_capability_id.clone(),
            capability_version: request.initiating_capability_version.clone(),
''',
    "CDO audit provenance context",
)

postgres_test = "crates/crm-application-runtime/tests/customer_privacy_access_export_postgres.rs"
replace_exact(
    postgres_test,
    '''    ACCESS_EXPORT_CAPABILITY_VERSION, ACCESS_EXPORT_REQUEST_CAPABILITY, ACTION_PLAN_RECORD_TYPE,
    ACTION_PLAN_STATE_MAXIMUM_BYTES, ACTION_PLAN_STATE_RETENTION_POLICY_ID,
''',
    '''    ACTION_PLAN_RECORD_TYPE, ACTION_PLAN_STATE_MAXIMUM_BYTES,
    ACTION_PLAN_STATE_RETENTION_POLICY_ID,
''',
    "remove private coordinate imports",
)
replace_exact(
    postgres_test,
    '''        actor_id: ActorId::try_new(ACTOR).unwrap(),
        correlation_id: CorrelationId::try_new("access-export-cdo-correlation").unwrap(),
        trace_id: TraceId::try_new("access-export-cdo-trace").unwrap(),
        prepared_at_unix_nanos: prepared.prepared_at_unix_nanos(),
''',
    '''        actor_id: ActorId::try_new(ACTOR).unwrap(),
        correlation_id: CorrelationId::try_new("access-export-cdo-correlation").unwrap(),
        trace_id: TraceId::try_new("access-export-cdo-trace").unwrap(),
        initiating_capability_id: CapabilityId::try_new("customer_privacy.case.approve").unwrap(),
        initiating_capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
        prepared_at_unix_nanos: prepared.prepared_at_unix_nanos(),
''',
    "manual CDO request provenance",
)
replace_exact(
    postgres_test,
    '''        initiating_capability_id: CapabilityId::try_new(ACCESS_EXPORT_REQUEST_CAPABILITY).unwrap(),
        initiating_capability_version: CapabilityVersion::try_new(ACCESS_EXPORT_CAPABILITY_VERSION)
            .unwrap(),
''',
    '''        initiating_capability_id: CapabilityId::try_new("customer_privacy.case.approve").unwrap(),
        initiating_capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
''',
    "access invocation public provenance",
)
replace_exact(
    postgres_test,
    "capability_id = 'customer_privacy.access_export.request'",
    "capability_id = 'customer_privacy.case.approve'",
    "evidence queries use registered provenance",
    expected=2,
)

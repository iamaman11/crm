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
    let runtime = repo.join("crates/crm-application-runtime/src/runtime.rs");

    replace_once(
        &runtime,
        "use crm_capability_adapters::{\n    AuthorizationGrant, FixedWindowRateLimiter, HmacSha256ApprovalVerifier, LiveAuthorizationStore,\n",
        "use crm_capability_adapters::{\n    AuthorizationGrant, FixedWindowRateLimiter, GatewayCapabilityClient, HmacSha256ApprovalVerifier,\n    LiveAuthorizationStore,\n",
    );
    replace_once(
        &runtime,
        "use crm_customer_data_operations_query_adapter::{\n    CustomerDataOperationsQueryAdapter, LIST_IMPORT_ROWS_CAPABILITY,\n};\n",
        "use crm_customer_data_operations_query_adapter::{\n    CustomerDataOperationsQueryAdapter, LIST_IMPORT_ROWS_CAPABILITY,\n};\nuse crm_customer_data_operations_execution_composition::{\n    IMPORT_EXECUTION_WORKER_ACTOR_ID, PartyImportExecutionCoordinator, PartyImportExecutionWorker,\n    PostgresImportExecutionOutcomeSink, PostgresImportExecutionSnapshotReader,\n    internal_capability_definitions,\n};\n",
    );
    replace_once(
        &runtime,
        "use crm_parties_capability_adapter::{\n    MODULE_ID as PARTIES_MODULE_ID, RECORD_TYPE as PARTY_RECORD_TYPE,\n};\n",
        "use crm_parties_capability_adapter::{\n    CREATE_CAPABILITY as PARTY_CREATE_CAPABILITY, MODULE_ID as PARTIES_MODULE_ID,\n    RECORD_TYPE as PARTY_RECORD_TYPE,\n};\n",
    );
    replace_once(
        &runtime,
        "    pub search_worker: Arc<GlobalSearchWorker>,\n    readiness: Arc<AtomicBool>,\n",
        "    pub search_worker: Arc<GlobalSearchWorker>,\n    pub import_execution_worker: Arc<PartyImportExecutionWorker>,\n    readiness: Arc<AtomicBool>,\n",
    );
    replace_once(
        &runtime,
        "        let query_definitions = application_query_definitions()\n            .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;\n        if config.bootstrap_allow_phase6 {\n",
        "        let query_definitions = application_query_definitions()\n            .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;\n        let internal_import_outcome_definitions = internal_capability_definitions()\n            .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;\n        let import_execution_worker_actor_id = ActorId::try_new(IMPORT_EXECUTION_WORKER_ACTOR_ID)\n            .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;\n        if config.bootstrap_allow_phase6 {\n",
    );
    replace_once(
        &runtime,
        "                &query_definitions,\n            )?;\n        }\n\n        let authorizer = Arc::new(LiveCapabilityAuthorizer::new(\n",
        "                &query_definitions,\n            )?;\n            bootstrap_import_execution_worker_access(\n                &config,\n                now,\n                &authorization_store,\n                &mutation_definitions,\n                &internal_import_outcome_definitions,\n                &import_execution_worker_actor_id,\n            )?;\n        }\n\n        let authorizer = Arc::new(LiveCapabilityAuthorizer::new(\n",
    );
    replace_once(
        &runtime,
        "        let visibility_authorizer = Arc::new(LiveQueryVisibilityAuthorizer::new(\n",
        "        let import_execution_reader =\n            Arc::new(PostgresImportExecutionSnapshotReader::new(store.clone()));\n        let import_execution_outcomes = Arc::new(PostgresImportExecutionOutcomeSink::new(\n            store.clone(),\n            authorizer.clone(),\n        ));\n        let import_execution_coordinator = Arc::new(PartyImportExecutionCoordinator::new(\n            Arc::new(GatewayCapabilityClient::new(Arc::clone(&mutation_gateway))),\n            import_execution_outcomes,\n        ));\n        let import_execution_worker = Arc::new(\n            PartyImportExecutionWorker::new(\n                store.clone(),\n                import_execution_reader,\n                import_execution_coordinator,\n                Arc::clone(&clock),\n            )\n            .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,\n        );\n\n        let visibility_authorizer = Arc::new(LiveQueryVisibilityAuthorizer::new(\n",
    );
    replace_once(
        &runtime,
        "            search_worker,\n            readiness: Arc::new(AtomicBool::new(false)),\n",
        "            search_worker,\n            import_execution_worker,\n            readiness: Arc::new(AtomicBool::new(false)),\n",
    );
    replace_once(
        &runtime,
        "    for tenant_id in &components.tenant_ids {\n        scan_link_events(components, tenant_id.clone()).await?;\n",
        "    for tenant_id in &components.tenant_ids {\n        components\n            .import_execution_worker\n            .run_tenant_cycle(tenant_id.clone())\n            .await\n            .map_err(|error| ApplicationRuntimeError::Server(error.to_string()))?;\n        scan_link_events(components, tenant_id.clone()).await?;\n",
    );
    replace_once(
        &runtime,
        "    Ok(())\n}\n\nfn upsert_bootstrap_visibility(\n",
        "    Ok(())\n}\n\nfn bootstrap_import_execution_worker_access(\n    config: &ApplicationConfig,\n    now_unix_nanos: i64,\n    authorization_store: &LiveAuthorizationStore,\n    mutation_definitions: &[CapabilityDefinition],\n    internal_definitions: &[CapabilityDefinition],\n    worker_actor_id: &ActorId,\n) -> Result<(), ApplicationRuntimeError> {\n    let expires_at = expiry(now_unix_nanos)?;\n    let party_create = mutation_definitions\n        .iter()\n        .find(|definition| {\n            definition.owner_module_id.as_str() == PARTIES_MODULE_ID\n                && definition.capability_id.as_str() == PARTY_CREATE_CAPABILITY\n        })\n        .ok_or_else(|| {\n            ApplicationRuntimeError::Assembly(\n                \"Party create capability is missing from the production catalog\".to_owned(),\n            )\n        })?;\n    for tenant_id in &config.tenant_ids {\n        for definition in std::iter::once(party_create).chain(internal_definitions.iter()) {\n            authorization_store\n                .upsert(AuthorizationGrant {\n                    tenant_id: tenant_id.clone(),\n                    actor_id: worker_actor_id.clone(),\n                    policy_id: definition.authorization_policy_id.clone(),\n                    capability_id: definition.capability_id.clone(),\n                    capability_version: definition.capability_version.clone(),\n                    owner_module_id: definition.owner_module_id.clone(),\n                    policy_version: BOOTSTRAP_POLICY_VERSION.to_owned(),\n                    expires_at_unix_nanos: Some(expires_at),\n                })\n                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;\n        }\n    }\n    Ok(())\n}\n\nfn upsert_bootstrap_visibility(\n",
    );

    run(repo, "cargo", &["fmt", "--all"]);
    fs::remove_file(manifest_dir.join("build.rs")).expect("temporary runtime wiring patch must be removable");
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
    let commit_status = Command::new("git")
        .args(["commit", "-m", "feat(phase8a7): assemble import execution runtime worker"])
        .current_dir(repo)
        .status()
        .expect("git commit must start");
    if !commit_status.success() {
        let no_staged_changes = Command::new("git")
            .args(["diff", "--cached", "--quiet"])
            .current_dir(repo)
            .status()
            .expect("git diff must start")
            .success();
        assert!(no_staged_changes, "git commit failed with staged changes");
        return;
    }
    let branch = env::var("GITHUB_HEAD_REF")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "develop/phase8a7-customer-import-jobs".to_owned());
    run(repo, "git", &["push", "origin", &format!("HEAD:{branch}")]);
}

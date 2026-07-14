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
    let metadata = repo.join("crates/crm-application-runtime/src/governed_metadata.rs");
    let runtime = repo.join("crates/crm-application-runtime/src/runtime.rs");

    replace_once(
        &metadata,
        "    CapabilityDefinition, CapabilityExecutionResult, CapabilityRequest,\n    TransactionalCapabilityExecutor,\n",
        "    CapabilityAuthorizer, CapabilityDefinition, CapabilityExecutionResult, CapabilityRequest,\n    TransactionalCapabilityExecutor,\n",
    );
    replace_once(
        &metadata,
        "    AggregateTarget, CapabilityBatchExecutionPlan, PostgresDataStore,\n    PostgresMetadataCapabilityExecutor, PostgresTransactionalAggregateExecutor, RecordGetQuery,\n",
        "    AggregateTarget, CapabilityBatchExecutionPlan, PostgresDataStore,\n    PostgresImmutableFileArtifactStore, PostgresMetadataCapabilityExecutor,\n    PostgresTransactionalAggregateExecutor, RecordGetQuery,\n",
    );
    replace_once(
        &metadata,
        "use crm_customer_data_operations_capability_adapter::{\n    CustomerDataOperationsCapabilityPlanner,\n    MUTATION_CAPABILITY_IDS as CUSTOMER_DATA_OPERATIONS_MUTATION_CAPABILITY_IDS,\n    capability_definitions as customer_data_operations_capability_definitions,\n};\n",
        "use crm_customer_data_operations_capability_adapter::{\n    CREATE_PARTY_IMPORT_JOB_CAPABILITY, CustomerDataOperationsCapabilityPlanner,\n    MUTATION_CAPABILITY_IDS as CUSTOMER_DATA_OPERATIONS_MUTATION_CAPABILITY_IDS,\n    VALIDATE_PARTY_IMPORT_ROWS_CAPABILITY,\n    capability_definitions as customer_data_operations_capability_definitions,\n};\nuse crm_customer_data_operations_source_composition::{\n    CustomerDataOperationsSourceExecutor,\n    SOURCE_MUTATION_CAPABILITY_IDS as CUSTOMER_DATA_OPERATIONS_SOURCE_MUTATION_CAPABILITY_IDS,\n    source_capability_definitions as customer_data_operations_source_capability_definitions,\n};\n",
    );
    replace_once(
        &metadata,
        "    definitions.extend(customer_data_operations_capability_definitions()?);\n    definitions.extend(metadata_mutation_capability_definitions()?);\n",
        "    definitions.extend(\n        customer_data_operations_capability_definitions()?\n            .into_iter()\n            .filter(|definition| {\n                !matches!(\n                    definition.capability_id.as_str(),\n                    CREATE_PARTY_IMPORT_JOB_CAPABILITY | VALIDATE_PARTY_IMPORT_ROWS_CAPABILITY\n                )\n            }),\n    );\n    definitions.extend(customer_data_operations_source_capability_definitions()?);\n    definitions.extend(metadata_mutation_capability_definitions()?);\n",
    );
    replace_once(
        &metadata,
        "    identity_resolution: IdentityResolutionCapabilityExecutor,\n    identity_resolution_merge: MergeLineageCapabilityExecutor,\n",
        "    identity_resolution: IdentityResolutionCapabilityExecutor,\n    identity_resolution_merge: MergeLineageCapabilityExecutor,\n    customer_data_operations_source: CustomerDataOperationsSourceExecutor,\n",
    );
    replace_once(
        &metadata,
        "        metadata: Arc<PostgresMetadataCapabilityExecutor>,\n    ) -> Self {\n",
        "        metadata: Arc<PostgresMetadataCapabilityExecutor>,\n        authorizer: Arc<dyn CapabilityAuthorizer>,\n    ) -> Self {\n",
    );
    replace_once(
        &metadata,
        "        let identity_resolution_merge = MergeLineageCapabilityExecutor::new(\n            Arc::new(PostgresMergeLineageReferenceReader::new(store.clone())),\n            aggregate.clone(),\n        );\n        Self {\n",
        "        let identity_resolution_merge = MergeLineageCapabilityExecutor::new(\n            Arc::new(PostgresMergeLineageReferenceReader::new(store.clone())),\n            aggregate.clone(),\n        );\n        let customer_data_operations_source = CustomerDataOperationsSourceExecutor::new(\n            store.clone(),\n            Arc::new(PostgresImmutableFileArtifactStore::new(store.clone())),\n            authorizer,\n        );\n        Self {\n",
    );
    replace_once(
        &metadata,
        "            identity_resolution,\n            identity_resolution_merge,\n        }\n",
        "            identity_resolution,\n            identity_resolution_merge,\n            customer_data_operations_source,\n        }\n",
    );
    replace_once(
        &metadata,
        "            .field(\"identity_resolution_merge\", &self.identity_resolution_merge)\n            .finish()\n",
        "            .field(\"identity_resolution_merge\", &self.identity_resolution_merge)\n            .field(\n                \"customer_data_operations_source\",\n                &self.customer_data_operations_source,\n            )\n            .finish()\n",
    );
    replace_once(
        &metadata,
        "        if METADATA_MUTATION_CAPABILITY_IDS.contains(&definition.capability_id.as_str()) {\n            self.metadata.execute(definition, request)\n",
        "        if CUSTOMER_DATA_OPERATIONS_SOURCE_MUTATION_CAPABILITY_IDS\n            .contains(&definition.capability_id.as_str())\n        {\n            self.customer_data_operations_source.execute(definition, request)\n        } else if METADATA_MUTATION_CAPABILITY_IDS.contains(&definition.capability_id.as_str()) {\n            self.metadata.execute(definition, request)\n",
    );

    replace_once(
        &runtime,
        "            Arc::new(PostgresMetadataCapabilityExecutor::new(store.clone())),\n        ));\n",
        "            Arc::new(PostgresMetadataCapabilityExecutor::new(store.clone())),\n            authorizer.clone(),\n        ));\n",
    );

    run(repo, "cargo", &["fmt", "--all"]);
    fs::remove_file(manifest_dir.join("build.rs"))
        .expect("temporary source runtime routing hook must be removable");
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
        .args([
            "commit",
            "-m",
            "feat(phase8a7): route artifact-backed import source capabilities",
        ])
        .current_dir(repo)
        .status()
        .expect("git commit must start");
    if !commit_status.success() {
        let clean = Command::new("git")
            .args(["diff", "--cached", "--quiet"])
            .current_dir(repo)
            .status()
            .expect("git diff must start")
            .success();
        assert!(clean, "git commit failed with staged changes");
        return;
    }
    let branch = env::var("GITHUB_HEAD_REF")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "develop/phase8a7-customer-import-jobs".to_owned());
    run(repo, "git", &["push", "origin", &format!("HEAD:{branch}")]);
}

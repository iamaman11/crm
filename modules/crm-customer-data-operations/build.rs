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
    assert!(
        text.contains(old),
        "patch anchor missing in {}",
        path.display()
    );
    fs::write(path, text.replacen(old, new, 1))
        .unwrap_or_else(|error| panic!("cannot write {}: {error}", path.display()));
}

fn remove_if_exists(path: &Path) {
    if path.exists() {
        fs::remove_file(path)
            .unwrap_or_else(|error| panic!("cannot remove {}: {error}", path.display()));
    }
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
    let manifest_dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be configured"),
    );
    let repo = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("module must live under repository/modules");

    let domain = repo.join("modules/crm-customer-data-operations/src/domain.rs");
    replace_once(
        &domain,
        "    pub fn mark_validated(&mut self, command: MarkImportJobValidated) -> Result<(), SdkError> {\n",
        r#"    pub fn record_validation_batch(
        &mut self,
        command: RecordImportValidationBatch,
    ) -> Result<(), SdkError> {
        self.require_version(command.expected_version)?;
        self.require_monotonic_time(command.occurred_at_unix_nanos)?;
        if self.status != ImportJobStatus::Created {
            return Err(conflict(
                "CUSTOMER_DATA_IMPORT_JOB_NOT_CREATED",
                "only a created import job can accept validation progress",
            ));
        }

        let next_valid_rows = self
            .valid_rows
            .checked_add(command.valid_rows)
            .ok_or_else(|| invalid_counter("valid row count overflow"))?;
        let next_invalid_rows = self
            .invalid_rows
            .checked_add(command.invalid_rows)
            .ok_or_else(|| invalid_counter("invalid row count overflow"))?;
        let next_total = next_valid_rows
            .checked_add(next_invalid_rows)
            .ok_or_else(|| invalid_counter("validation row counts overflow"))?;
        if next_total > self.total_rows {
            return Err(invalid_counter(
                "accumulated validation rows cannot exceed the immutable source row count",
            ));
        }

        self.valid_rows = next_valid_rows;
        self.invalid_rows = next_invalid_rows;
        self.advance(command.occurred_at_unix_nanos)
    }

    pub fn finalize_validation(
        &mut self,
        command: FinalizeImportValidation,
    ) -> Result<(), SdkError> {
        self.require_version(command.expected_version)?;
        self.require_monotonic_time(command.occurred_at_unix_nanos)?;
        if self.status != ImportJobStatus::Created {
            return Err(conflict(
                "CUSTOMER_DATA_IMPORT_JOB_NOT_CREATED",
                "only a created import job can finalize validation",
            ));
        }
        let validated_rows = self
            .valid_rows
            .checked_add(self.invalid_rows)
            .ok_or_else(|| invalid_counter("validation row counts overflow"))?;
        if validated_rows != self.total_rows {
            return Err(conflict(
                "CUSTOMER_DATA_IMPORT_VALIDATION_INCOMPLETE",
                "all immutable source rows must have authoritative validation outcomes before finalization",
            ));
        }

        self.status = ImportJobStatus::Validated;
        self.advance(command.occurred_at_unix_nanos)
    }

    pub fn mark_validated(&mut self, command: MarkImportJobValidated) -> Result<(), SdkError> {
"#,
    );
    replace_once(
        &domain,
        "    pub const fn checkpoint_row_position(&self) -> u32 {\n        self.checkpoint_row_position\n    }\n\n",
        r#"    pub const fn total_rows(&self) -> u32 {
        self.total_rows
    }

    pub const fn valid_rows(&self) -> u32 {
        self.valid_rows
    }

    pub const fn invalid_rows(&self) -> u32 {
        self.invalid_rows
    }

    pub const fn checkpoint_row_position(&self) -> u32 {
        self.checkpoint_row_position
    }

"#,
    );

    let planner = repo.join(
        "crates/crm-customer-data-operations-capability-adapter/src/planner.rs",
    );
    replace_once(
        &planner,
        r#"    PARTY_IMPORT_JOB_CREATED_EVENT_SCHEMA, PARTY_IMPORT_JOB_CREATED_EVENT_TYPE,
    PARTY_IMPORT_ROW_VALIDATED_EVENT_SCHEMA, PARTY_IMPORT_ROW_VALIDATED_EVENT_TYPE,
    START_PARTY_IMPORT_EXECUTION_CAPABILITY, START_PARTY_IMPORT_EXECUTION_REQUEST_SCHEMA,
"#,
        r#"    PARTY_IMPORT_JOB_CREATED_EVENT_SCHEMA, PARTY_IMPORT_JOB_CREATED_EVENT_TYPE,
    PARTY_IMPORT_ROW_VALIDATED_EVENT_SCHEMA, PARTY_IMPORT_ROW_VALIDATED_EVENT_TYPE,
    PARTY_IMPORT_VALIDATION_COMPLETED_EVENT_SCHEMA, PARTY_IMPORT_VALIDATION_COMPLETED_EVENT_TYPE,
    PARTY_IMPORT_VALIDATION_PROGRESSED_EVENT_SCHEMA, PARTY_IMPORT_VALIDATION_PROGRESSED_EVENT_TYPE,
    START_PARTY_IMPORT_EXECUTION_CAPABILITY, START_PARTY_IMPORT_EXECUTION_REQUEST_SCHEMA,
"#,
    );
    replace_once(
        &planner,
        r#"    PartialExecutionPolicy, PartyImportKind, PartyImportMapping, PreparedPartyRow, RowDiagnostic,
    RowIdentitySource, SourceDescriptor, SourceSystemId, StartImportExecution, TargetPartyId,
"#,
        r#"    FinalizeImportValidation, PartialExecutionPolicy, PartyImportKind, PartyImportMapping,
    PreparedPartyRow, RecordImportValidationBatch, RowDiagnostic, RowIdentitySource,
    SourceDescriptor, SourceSystemId, StartImportExecution, TargetPartyId,
"#,
    );
    replace_once(
        &planner,
        r#"            CANCEL_PARTY_IMPORT_JOB_CAPABILITY => plan_cancel(definition, request, current),
            FINALIZE_PARTY_IMPORT_VALIDATION_CAPABILITY => Err(finalize_requires_composition()),
"#,
        r#"            CANCEL_PARTY_IMPORT_JOB_CAPABILITY => plan_cancel(definition, request, current),
            FINALIZE_PARTY_IMPORT_VALIDATION_CAPABILITY => {
                plan_finalize_validation(definition, request, current)
            }
"#,
    );

    let query = repo.join("crates/crm-customer-data-operations-query-adapter/src/lib.rs");
    replace_once(
        &query,
        r#"        Some(wire::ImportJobStatus::Completed) => job.status() == ImportJobStatus::Completed,
        Some(wire::ImportJobStatus::Cancelled) => job.status() == ImportJobStatus::Cancelled,
    }
"#,
        r#"        Some(wire::ImportJobStatus::Completed) => job.status() == ImportJobStatus::Completed,
        Some(wire::ImportJobStatus::Cancelled) => job.status() == ImportJobStatus::Cancelled,
        Some(_) => false,
    }
"#,
    );
    replace_once(
        &query,
        r#"        Some(wire::ImportRowStatus::Succeeded) => row.status() == ImportRowStatus::Succeeded,
    }
"#,
        r#"        Some(wire::ImportRowStatus::Succeeded) => row.status() == ImportRowStatus::Succeeded,
        Some(_) => false,
    }
"#,
    );

    let metadata = repo.join("crates/crm-application-runtime/src/governed_metadata.rs");
    replace_once(
        &metadata,
        r#"        } else if IDENTITY_RESOLUTION_MUTATION_CAPABILITY_IDS
            .contains(&definition.capability_id.as_str())
        {
            IdentityResolutionCapabilityPlanner.target(definition, request)
        } else {
            SalesActivitiesCapabilityPlannerRouter.target(definition, request)
        }
"#,
        r#"        } else if IDENTITY_RESOLUTION_MUTATION_CAPABILITY_IDS
            .contains(&definition.capability_id.as_str())
        {
            IdentityResolutionCapabilityPlanner.target(definition, request)
        } else if CUSTOMER_DATA_OPERATIONS_MUTATION_CAPABILITY_IDS
            .contains(&definition.capability_id.as_str())
        {
            CustomerDataOperationsCapabilityPlanner.target(definition, request)
        } else {
            SalesActivitiesCapabilityPlannerRouter.target(definition, request)
        }
"#,
    );
    replace_once(
        &metadata,
        r#"        } else if IDENTITY_RESOLUTION_MUTATION_CAPABILITY_IDS
            .contains(&definition.capability_id.as_str())
        {
            IdentityResolutionCapabilityPlanner.plan(definition, request, current)
        } else {
            SalesActivitiesCapabilityPlannerRouter.plan(definition, request, current)
        }
"#,
        r#"        } else if IDENTITY_RESOLUTION_MUTATION_CAPABILITY_IDS
            .contains(&definition.capability_id.as_str())
        {
            IdentityResolutionCapabilityPlanner.plan(definition, request, current)
        } else if CUSTOMER_DATA_OPERATIONS_MUTATION_CAPABILITY_IDS
            .contains(&definition.capability_id.as_str())
        {
            CustomerDataOperationsCapabilityPlanner.plan(definition, request, current)
        } else {
            SalesActivitiesCapabilityPlannerRouter.plan(definition, request, current)
        }
"#,
    );
    replace_once(
        &metadata,
        r#"        } else if IDENTITY_RESOLUTION_MERGE_QUERY_CAPABILITY_IDS
            .contains(&definition.capability_id.as_str())
        {
            self.identity_resolution_merge.validate(definition, request)
        } else {
            self.production.validate(definition, request)
        }
"#,
        r#"        } else if IDENTITY_RESOLUTION_MERGE_QUERY_CAPABILITY_IDS
            .contains(&definition.capability_id.as_str())
        {
            self.identity_resolution_merge.validate(definition, request)
        } else if CUSTOMER_DATA_OPERATIONS_QUERY_CAPABILITY_IDS
            .contains(&definition.capability_id.as_str())
        {
            self.customer_data_operations.validate(definition, request)
        } else {
            self.production.validate(definition, request)
        }
"#,
    );
    replace_once(
        &metadata,
        r#"        } else if IDENTITY_RESOLUTION_MERGE_QUERY_CAPABILITY_IDS
            .contains(&definition.capability_id.as_str())
        {
            self.identity_resolution_merge.execute(definition, request)
        } else {
            self.production.execute(definition, request)
        }
"#,
        r#"        } else if IDENTITY_RESOLUTION_MERGE_QUERY_CAPABILITY_IDS
            .contains(&definition.capability_id.as_str())
        {
            self.identity_resolution_merge.execute(definition, request)
        } else if CUSTOMER_DATA_OPERATIONS_QUERY_CAPABILITY_IDS
            .contains(&definition.capability_id.as_str())
        {
            self.customer_data_operations.execute(definition, request)
        } else {
            self.production.execute(definition, request)
        }
"#,
    );

    let runtime = repo.join("crates/crm-application-runtime/src/runtime.rs");
    let query_adapter_anchor = r#"        let identity_resolution_merge_query_adapter = IdentityResolutionMergeQueryAdapter::new(
            store.clone(),
            CursorCodec::new(cursor_key)
                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,
            visibility_authorizer.clone(),
        )
        .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
"#;
    let query_adapter_replacement = format!(
        "{query_adapter_anchor}{}",
        r#"        let customer_data_operations_query_adapter = CustomerDataOperationsQueryAdapter::new(
            store.clone(),
            CursorCodec::new(cursor_key)
                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,
            visibility_authorizer.clone(),
        )
        .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
"#
    );
    replace_once(
        &runtime,
        query_adapter_anchor,
        &query_adapter_replacement,
    );
    let bootstrap_anchor = r#"                PARTY_RELATIONSHIPS_MODULE_ID => upsert_bootstrap_visibility(
                    visibility_store,
                    config,
                    tenant_id,
                    definition,
                    BootstrapVisibilityResource {
                        owner_module_id: PARTY_RELATIONSHIPS_MODULE_ID,
                        resource_type: PARTY_RELATIONSHIP_RECORD_TYPE,
                    },
                    party_relationship_fields(),
                    expires_at,
                )?,
"#;
    let bootstrap_replacement = format!(
        "{bootstrap_anchor}{}",
        r#"                CUSTOMER_DATA_OPERATIONS_MODULE_ID => {
                    upsert_bootstrap_visibility(
                        visibility_store,
                        config,
                        tenant_id,
                        definition,
                        BootstrapVisibilityResource {
                            owner_module_id: CUSTOMER_DATA_OPERATIONS_MODULE_ID,
                            resource_type: CUSTOMER_DATA_IMPORT_JOB_RECORD_TYPE,
                        },
                        customer_data_import_job_fields(),
                        expires_at,
                    )?;
                    if definition.capability_id.as_str() == LIST_IMPORT_ROWS_CAPABILITY {
                        upsert_bootstrap_visibility(
                            visibility_store,
                            config,
                            tenant_id,
                            definition,
                            BootstrapVisibilityResource {
                                owner_module_id: CUSTOMER_DATA_OPERATIONS_MODULE_ID,
                                resource_type: CUSTOMER_DATA_IMPORT_ROW_RECORD_TYPE,
                            },
                            customer_data_import_row_fields(),
                            expires_at,
                        )?;
                    }
                }
"#
    );
    replace_once(&runtime, bootstrap_anchor, &bootstrap_replacement);

    let module_lib = repo.join("modules/crm-customer-data-operations/src/lib.rs");
    replace_once(
        &module_lib,
        "pub mod domain;\npub mod persistence;\n",
        "pub mod domain;\npub mod execution;\npub mod persistence;\n",
    );
    replace_once(
        &module_lib,
        "pub use domain::*;\npub use persistence::*;\n",
        "pub use domain::*;\npub use execution::*;\npub use persistence::*;\n",
    );

    let contract_surface = repo.join(
        "crates/crm-customer-data-operations-capability-adapter/tests/contract_surface.rs",
    );
    replace_once(
        &contract_surface,
        "wire.mapping.unwrap().source_external_id_column.as_deref(),",
        "wire.mapping.unwrap().source_external_id_column.as_str(),",
    );

    for workflow in [
        ".github/workflows/phase8a7-current-fix.yml",
        ".github/workflows/phase8a7-normalize-current.yml",
        ".github/workflows/phase8a7-apply-validation-application.yml",
    ] {
        remove_if_exists(&repo.join(workflow));
    }

    run(repo, "git", &["fetch", "origin", "main", "--depth=1"]);
    run(
        repo,
        "git",
        &[
            "checkout",
            "origin/main",
            "--",
            ".github/workflows/rust-generated-sync.yml",
        ],
    );
    run(repo, "git", &["add", "-A", ".github/workflows"]);

    remove_if_exists(&manifest_dir.join("build.rs"));
}

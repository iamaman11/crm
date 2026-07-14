from pathlib import Path

REPO = Path(__file__).resolve().parents[2]


def patch(path: str, old: str, new: str, marker: str) -> None:
    target = REPO / path
    text = target.read_text()
    if marker in text:
        return
    if old not in text:
        raise RuntimeError(f"patch anchor missing in {path}: {marker}")
    target.write_text(text.replace(old, new, 1))


def main() -> None:
    patch(
        "modules/crm-customer-data-operations/src/domain.rs",
        "    pub fn mark_validated(&mut self, command: MarkImportJobValidated) -> Result<(), SdkError> {\n",
        '''    pub fn record_validation_batch(
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
''',
        "pub fn record_validation_batch",
    )
    patch(
        "modules/crm-customer-data-operations/src/domain.rs",
        "    pub const fn checkpoint_row_position(&self) -> u32 {\n        self.checkpoint_row_position\n    }\n\n",
        '''    pub const fn total_rows(&self) -> u32 {
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

''',
        "pub const fn valid_rows",
    )

    patch(
        "crates/crm-customer-data-operations-capability-adapter/src/planner.rs",
        '''    PARTY_IMPORT_JOB_CREATED_EVENT_SCHEMA, PARTY_IMPORT_JOB_CREATED_EVENT_TYPE,
    PARTY_IMPORT_ROW_VALIDATED_EVENT_SCHEMA, PARTY_IMPORT_ROW_VALIDATED_EVENT_TYPE,
    START_PARTY_IMPORT_EXECUTION_CAPABILITY, START_PARTY_IMPORT_EXECUTION_REQUEST_SCHEMA,
''',
        '''    PARTY_IMPORT_JOB_CREATED_EVENT_SCHEMA, PARTY_IMPORT_JOB_CREATED_EVENT_TYPE,
    PARTY_IMPORT_ROW_VALIDATED_EVENT_SCHEMA, PARTY_IMPORT_ROW_VALIDATED_EVENT_TYPE,
    PARTY_IMPORT_VALIDATION_COMPLETED_EVENT_SCHEMA, PARTY_IMPORT_VALIDATION_COMPLETED_EVENT_TYPE,
    PARTY_IMPORT_VALIDATION_PROGRESSED_EVENT_SCHEMA, PARTY_IMPORT_VALIDATION_PROGRESSED_EVENT_TYPE,
    START_PARTY_IMPORT_EXECUTION_CAPABILITY, START_PARTY_IMPORT_EXECUTION_REQUEST_SCHEMA,
''',
        "PARTY_IMPORT_VALIDATION_PROGRESSED_EVENT_SCHEMA, PARTY_IMPORT_VALIDATION_PROGRESSED_EVENT_TYPE",
    )
    patch(
        "crates/crm-customer-data-operations-capability-adapter/src/planner.rs",
        '''    PartialExecutionPolicy, PartyImportKind, PartyImportMapping, PreparedPartyRow, RowDiagnostic,
    RowIdentitySource, SourceDescriptor, SourceSystemId, StartImportExecution, TargetPartyId,
''',
        '''    FinalizeImportValidation, PartialExecutionPolicy, PartyImportKind, PartyImportMapping,
    PreparedPartyRow, RecordImportValidationBatch, RowDiagnostic, RowIdentitySource,
    SourceDescriptor, SourceSystemId, StartImportExecution, TargetPartyId,
''',
        "FinalizeImportValidation, PartialExecutionPolicy",
    )
    patch(
        "crates/crm-customer-data-operations-capability-adapter/src/planner.rs",
        '''            CANCEL_PARTY_IMPORT_JOB_CAPABILITY => plan_cancel(definition, request, current),
            FINALIZE_PARTY_IMPORT_VALIDATION_CAPABILITY => Err(finalize_requires_composition()),
''',
        '''            CANCEL_PARTY_IMPORT_JOB_CAPABILITY => plan_cancel(definition, request, current),
            FINALIZE_PARTY_IMPORT_VALIDATION_CAPABILITY => {
                plan_finalize_validation(definition, request, current)
            }
''',
        "FINALIZE_PARTY_IMPORT_VALIDATION_CAPABILITY => {\n                plan_finalize_validation",
    )

    patch(
        "crates/crm-customer-data-operations-query-adapter/src/lib.rs",
        '''        Some(wire::ImportJobStatus::Completed) => job.status() == ImportJobStatus::Completed,
        Some(wire::ImportJobStatus::Cancelled) => job.status() == ImportJobStatus::Cancelled,
    }
''',
        '''        Some(wire::ImportJobStatus::Completed) => job.status() == ImportJobStatus::Completed,
        Some(wire::ImportJobStatus::Cancelled) => job.status() == ImportJobStatus::Cancelled,
        Some(_) => false,
    }
''',
        "Some(_) => false,\n    }\n}\n\nfn row_matches_status",
    )
    target = REPO / "crates/crm-customer-data-operations-query-adapter/src/lib.rs"
    text = target.read_text()
    row_old = '''        Some(wire::ImportRowStatus::Succeeded) => row.status() == ImportRowStatus::Succeeded,
    }
'''
    row_new = '''        Some(wire::ImportRowStatus::Succeeded) => row.status() == ImportRowStatus::Succeeded,
        Some(_) => false,
    }
'''
    if text.count("Some(_) => false,") < 2:
        if row_old not in text:
            raise RuntimeError("row enum wildcard anchor missing")
        target.write_text(text.replace(row_old, row_new, 1))

    patch(
        "crates/crm-application-runtime/src/governed_metadata.rs",
        '''        } else if IDENTITY_RESOLUTION_MUTATION_CAPABILITY_IDS
            .contains(&definition.capability_id.as_str())
        {
            IdentityResolutionCapabilityPlanner.target(definition, request)
        } else {
            SalesActivitiesCapabilityPlannerRouter.target(definition, request)
        }
''',
        '''        } else if IDENTITY_RESOLUTION_MUTATION_CAPABILITY_IDS
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
''',
        "CustomerDataOperationsCapabilityPlanner.target",
    )
    patch(
        "crates/crm-application-runtime/src/governed_metadata.rs",
        '''        } else if IDENTITY_RESOLUTION_MUTATION_CAPABILITY_IDS
            .contains(&definition.capability_id.as_str())
        {
            IdentityResolutionCapabilityPlanner.plan(definition, request, current)
        } else {
            SalesActivitiesCapabilityPlannerRouter.plan(definition, request, current)
        }
''',
        '''        } else if IDENTITY_RESOLUTION_MUTATION_CAPABILITY_IDS
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
''',
        "CustomerDataOperationsCapabilityPlanner.plan",
    )
    patch(
        "crates/crm-application-runtime/src/governed_metadata.rs",
        '''        } else if IDENTITY_RESOLUTION_MERGE_QUERY_CAPABILITY_IDS
            .contains(&definition.capability_id.as_str())
        {
            self.identity_resolution_merge.validate(definition, request)
        } else {
            self.production.validate(definition, request)
        }
''',
        '''        } else if IDENTITY_RESOLUTION_MERGE_QUERY_CAPABILITY_IDS
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
''',
        "self.customer_data_operations.validate",
    )
    patch(
        "crates/crm-application-runtime/src/governed_metadata.rs",
        '''        } else if IDENTITY_RESOLUTION_MERGE_QUERY_CAPABILITY_IDS
            .contains(&definition.capability_id.as_str())
        {
            self.identity_resolution_merge.execute(definition, request)
        } else {
            self.production.execute(definition, request)
        }
''',
        '''        } else if IDENTITY_RESOLUTION_MERGE_QUERY_CAPABILITY_IDS
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
''',
        "self.customer_data_operations.execute",
    )

    runtime = REPO / "crates/crm-application-runtime/src/runtime.rs"
    text = runtime.read_text()
    if "let customer_data_operations_query_adapter =" not in text:
        anchor = '''        let identity_resolution_merge_query_adapter = IdentityResolutionMergeQueryAdapter::new(
            store.clone(),
            CursorCodec::new(cursor_key)
                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,
            visibility_authorizer.clone(),
        )
        .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
'''
        addition = anchor + '''        let customer_data_operations_query_adapter = CustomerDataOperationsQueryAdapter::new(
            store.clone(),
            CursorCodec::new(cursor_key)
                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,
            visibility_authorizer.clone(),
        )
        .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
'''
        if anchor not in text:
            raise RuntimeError("runtime query adapter construction anchor missing")
        text = text.replace(anchor, addition, 1)
    if "CUSTOMER_DATA_OPERATIONS_MODULE_ID =>" not in text:
        anchor = '''                PARTY_RELATIONSHIPS_MODULE_ID => upsert_bootstrap_visibility(
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
'''
        addition = anchor + '''                CUSTOMER_DATA_OPERATIONS_MODULE_ID => {
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
'''
        if anchor not in text:
            raise RuntimeError("runtime bootstrap visibility anchor missing")
        text = text.replace(anchor, addition, 1)
    runtime.write_text(text)

    patch(
        "modules/crm-customer-data-operations/src/lib.rs",
        "pub mod domain;\npub mod persistence;\n",
        "pub mod domain;\npub mod execution;\npub mod persistence;\n",
        "pub mod execution;",
    )
    patch(
        "modules/crm-customer-data-operations/src/lib.rs",
        "pub use domain::*;\npub use persistence::*;\n",
        "pub use domain::*;\npub use execution::*;\npub use persistence::*;\n",
        "pub use execution::*;",
    )

    patch(
        "crates/crm-customer-data-operations-capability-adapter/tests/contract_surface.rs",
        "wire.mapping.unwrap().source_external_id_column.as_deref(),",
        "wire.mapping.unwrap().source_external_id_column.as_str(),",
        "source_external_id_column.as_str(),",
    )

    Path(__file__).unlink()


if __name__ == "__main__":
    main()

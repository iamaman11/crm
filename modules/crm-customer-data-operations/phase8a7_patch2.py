from pathlib import Path

REPO = Path(__file__).resolve().parents[2]


def edit(relative: str, transform) -> None:
    path = REPO / relative
    text = path.read_text()
    updated = transform(text)
    if updated != text:
        path.write_text(updated)


def replace_if_missing(text: str, marker: str, old: str, new: str) -> str:
    if marker in text:
        return text
    if old in text:
        return text.replace(old, new, 1)
    return text


def main() -> None:
    def domain(text: str) -> str:
        text = replace_if_missing(
            text,
            "pub fn record_validation_batch",
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
        let next_valid_rows = self.valid_rows.checked_add(command.valid_rows)
            .ok_or_else(|| invalid_counter("valid row count overflow"))?;
        let next_invalid_rows = self.invalid_rows.checked_add(command.invalid_rows)
            .ok_or_else(|| invalid_counter("invalid row count overflow"))?;
        let next_total = next_valid_rows.checked_add(next_invalid_rows)
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
        let validated_rows = self.valid_rows.checked_add(self.invalid_rows)
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
        )
        return replace_if_missing(
            text,
            "pub const fn valid_rows",
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
        )

    edit("modules/crm-customer-data-operations/src/domain.rs", domain)

    def planner(text: str) -> str:
        text = replace_if_missing(
            text,
            "PARTY_IMPORT_VALIDATION_PROGRESSED_EVENT_SCHEMA, PARTY_IMPORT_VALIDATION_PROGRESSED_EVENT_TYPE",
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
        )
        text = replace_if_missing(
            text,
            "FinalizeImportValidation, PartialExecutionPolicy",
            '''    PartialExecutionPolicy, PartyImportKind, PartyImportMapping, PreparedPartyRow, RowDiagnostic,
    RowIdentitySource, SourceDescriptor, SourceSystemId, StartImportExecution, TargetPartyId,
''',
            '''    FinalizeImportValidation, PartialExecutionPolicy, PartyImportKind, PartyImportMapping,
    PreparedPartyRow, RecordImportValidationBatch, RowDiagnostic, RowIdentitySource,
    SourceDescriptor, SourceSystemId, StartImportExecution, TargetPartyId,
''',
        )
        return replace_if_missing(
            text,
            "FINALIZE_PARTY_IMPORT_VALIDATION_CAPABILITY => {\n                plan_finalize_validation",
            '''            CANCEL_PARTY_IMPORT_JOB_CAPABILITY => plan_cancel(definition, request, current),
            FINALIZE_PARTY_IMPORT_VALIDATION_CAPABILITY => Err(finalize_requires_composition()),
''',
            '''            CANCEL_PARTY_IMPORT_JOB_CAPABILITY => plan_cancel(definition, request, current),
            FINALIZE_PARTY_IMPORT_VALIDATION_CAPABILITY => {
                plan_finalize_validation(definition, request, current)
            }
''',
        )

    edit("crates/crm-customer-data-operations-capability-adapter/src/planner.rs", planner)

    def query(text: str) -> str:
        if text.count("Some(_) => false,") < 1:
            text = text.replace(
                '''        Some(wire::ImportJobStatus::Cancelled) => job.status() == ImportJobStatus::Cancelled,
    }
''',
                '''        Some(wire::ImportJobStatus::Cancelled) => job.status() == ImportJobStatus::Cancelled,
        Some(_) => false,
    }
''',
                1,
            )
        if text.count("Some(_) => false,") < 2:
            text = text.replace(
                '''        Some(wire::ImportRowStatus::Succeeded) => row.status() == ImportRowStatus::Succeeded,
    }
''',
                '''        Some(wire::ImportRowStatus::Succeeded) => row.status() == ImportRowStatus::Succeeded,
        Some(_) => false,
    }
''',
                1,
            )
        return text

    edit("crates/crm-customer-data-operations-query-adapter/src/lib.rs", query)

    def metadata(text: str) -> str:
        text = replace_if_missing(
            text,
            "CustomerDataOperationsCapabilityPlanner.target",
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
        )
        text = replace_if_missing(
            text,
            "CustomerDataOperationsCapabilityPlanner.plan",
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
        )
        text = replace_if_missing(
            text,
            "self.customer_data_operations.validate",
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
        )
        return replace_if_missing(
            text,
            "self.customer_data_operations.execute",
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
        )

    edit("crates/crm-application-runtime/src/governed_metadata.rs", metadata)

    def runtime(text: str) -> str:
        if "let customer_data_operations_query_adapter =" not in text:
            anchor = '''        let identity_resolution_merge_query_adapter = IdentityResolutionMergeQueryAdapter::new(
            store.clone(),
            CursorCodec::new(cursor_key)
                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,
            visibility_authorizer.clone(),
        )
        .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
'''
            if anchor in text:
                text = text.replace(
                    anchor,
                    anchor + '''        let customer_data_operations_query_adapter = CustomerDataOperationsQueryAdapter::new(
            store.clone(),
            CursorCodec::new(cursor_key)
                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,
            visibility_authorizer.clone(),
        )
        .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
''',
                    1,
                )
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
            if anchor in text:
                text = text.replace(
                    anchor,
                    anchor + '''                CUSTOMER_DATA_OPERATIONS_MODULE_ID => {
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
''',
                    1,
                )
        return text

    edit("crates/crm-application-runtime/src/runtime.rs", runtime)

    edit(
        "modules/crm-customer-data-operations/src/lib.rs",
        lambda text: text
        if "pub mod execution;" in text
        else text.replace("pub mod domain;\n", "pub mod domain;\npub mod execution;\n", 1),
    )
    edit(
        "modules/crm-customer-data-operations/src/lib.rs",
        lambda text: text
        if "pub use execution::*;" in text
        else text.replace("pub use domain::*;\n", "pub use domain::*;\npub use execution::*;\n", 1),
    )
    edit(
        "crates/crm-customer-data-operations-capability-adapter/tests/contract_surface.rs",
        lambda text: text.replace(
            "wire.mapping.unwrap().source_external_id_column.as_deref(),",
            "wire.mapping.unwrap().source_external_id_column.as_str(),",
            1,
        ),
    )

    for relative in [
        "modules/crm-customer-data-operations/phase8a7_patch.py",
        "modules/crm-customer-data-operations/phase8a7_patch2.py",
    ]:
        path = REPO / relative
        if path.exists():
            path.unlink()


if __name__ == "__main__":
    main()

# CRM #352 — поведенческие свидетельства по всем 93 legacy crates

Pinned remote [`dev@a627e6a9f1966aaac5fcc0179c943811f879eb38`](https://github.com/iamaman11/crm/commit/a627e6a9f1966aaac5fcc0179c943811f879eb38). Это статический каталог *каждого* из 532 `src/**/*.rs` и `tests/**/*.rs` файлов 93 legacy crates. Он фиксирует nested функции и публичность, тестовые названия, видимые error/guard strings и ссылки на строку. Индексатор regex-based и не заменяет Rust compiler, AST/call graph/исполняемую проверку бизнес-правил. Все приведённые исходники являются SOURCE-ONLY, если не доказан отдельно соответствующий действующий D1 owner. **Не трактовать сходство имени как паритет**. По продуктовым решениям см. [93×matrix](legacy-crate-semantic-matrix.md).

**Дополнительно:** [`build.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/build.rs) и [`examples/customer_privacy_application_runtime_lifecycle.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/examples/customer_privacy_application_runtime_lifecycle.rs) прочитаны, но не входят в 532 src/test файла. Все связи Cargo приведены в матрице.

## 01. `crm-activities-capability-adapter` — NOW / A? + B-NOW-if-gap + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-activities-capability-adapter/src/lib.rs) · 10 lines

- **Re-exports:** `pub use planner::*;`

### [`src/planner.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-activities-capability-adapter/src/planner.rs) · 939 lines

- **Public declarations (1):** `capability_definition@L47`
- **Internal/trait/helpers (37):** `target@L82`, `plan@L118`, `plan_create@L135`, `plan_update@L201`, `plan_complete@L264`, `plan_reminder@L334`, `execution_plan@L409`, `persisted_contract@L438`, `persisted_payload@L449`, `task_from_snapshot@L453`, `task_to_wire@L466`, `owner_from_wire@L493`, `owner_to_wire@L508`, `priority_from_wire@L518`, `priority_to_wire@L531`, `status_to_wire@L540`, `optional_time@L547`, `string_patch@L556`, `owner_patch@L577`, `priority_patch@L598`, `time_patch@L619`, `changed_fields@L640`, `ensure_target@L660`, `required@L667`, `ensure_definition@L671`, `unsupported_capability@L690`, `invalid_plan@L699`, `create_plan_is_deterministic_and_uses_internal_persisted_schema@L719`, `create_replacement_coordinate_reuses_the_v1_wire_contract@L737`, `non_create_replacement_version_is_rejected@L765`, `cross_tenant_related_resource_is_rejected@L794`, `completing_an_already_completed_task_is_an_audited_noop@L804`, `create_request@L821`, `complete_request@L845`, `completed_snapshot@L858`, `owner_wire@L886`, `request@L894`
- **Public types:** `ActivitiesTaskCapabilityPlanner@L45`
- **Detected tests (5):** `create_plan_is_deterministic_and_uses_internal_persisted_schema@L719`, `create_replacement_coordinate_reuses_the_v1_wire_contract@L737`, `non_create_replacement_version_is_rejected@L765`, `cross_tenant_related_resource_is_rejected@L794`, `completing_an_already_completed_task_is_an_audited_noop@L804`
- **Error/guard literals:** `ACTIVITIES_PERSISTED_TASK_IDENTITY_INVALID`, `ACTIVITIES_CAPABILITY_UNSUPPORTED`, `ACTIVITIES_MUTATION_PLAN_INVALID`
- **Rust crate references:** `crm_activities`, `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_contracts`, `crm_core_data`, `crm_module_sdk`, `crm_proto_contracts`

### [`tests/postgres_noop.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-activities-capability-adapter/tests/postgres_noop.rs) · 253 lines

- **Internal/trait/helpers (3):** `completed_task_noop_commits_audit_and_idempotency_without_outbox@L24`, `count@L182`, `request@L211`
- **Detected tests (1):** `completed_task_noop_commits_audit_and_idempotency_without_outbox@L24`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`
- **Rust crate references:** `crm_activities_capability_adapter`, `crm_capability_runtime`, `crm_core_data`, `crm_module_sdk`, `crm_proto_contracts`

## 02. `crm-application-composition` — RETIRE-REVIEW / D? + B? invariants

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-composition/src/lib.rs) · 1378 lines

- **Public declarations (35):** `new@L79`, `with_mutation@L87`, `with_query@L92`, `new@L117`, `add_mutations@L121`, `add_queries@L143`, `add_empty_module@L165`, `merge@L177`, `build@L190`, `new@L274`, `add_module@L278`, `build@L314`, `module_ids@L390`, `mutation_definitions@L394`, `query_definitions@L398`, `mutation_registry@L402`, `query_registry@L406`, `mutation_validator@L410`, `mutation_executor@L414`, `query_validator@L418`, `query_executor@L422`, `new@L649`, `new@L696`, `new@L744`, `order@L748`, `new@L772`, `new@L858`, `add@L866`, `add_in_phase@L875`, `build@L898`, `len@L928`, `is_empty@L932`, `coordinates@L936`, `scheduled_coordinates@L942`, `run_tenant_cycle@L950`
- **Internal/trait/helpers (53):** `coordinate@L28`, `fmt@L43`, `fmt@L61`, `fmt@L108`, `fmt@L216`, `fmt@L263`, `validate_owner@L350`, `fmt@L379`, `from_mutations@L433`, `from_queries@L444`, `resolve@L457`, `fmt@L480`, `validate@L489`, `fmt@L511`, `execute@L520`, `fmt@L542`, `validate@L551`, `fmt@L573`, `execute@L582`, `validate@L602`, `validate@L615`, `is_active@L625`, `fmt@L639`, `validate@L658`, `fmt@L686`, `validate@L705`, `run_tenant_cycle@L724`, `fmt@L761`, `run_tenant_cycle@L786`, `fmt@L815`, `fmt@L848`, `valid_worker_id@L905`, `fmt@L919`, `route_unavailable@L980`, `module_not_active@L984`, `composition_invalid@L994`, `validate@L1021`, `execute@L1034`, `is_active@L1054`, `run_tenant_cycle@L1070`, `exact_routes_do_not_need_central_capability_switches@L1083`, `inactive_module_fails_before_inner_semantic_validation@L1117`, `background_workers_run_in_stable_coordinate_order@L1133`, `background_worker_phases_precede_module_sort_order@L1166`, `inactive_background_worker_is_skipped_before_inner_execution@L1201`, `contribution_set_merges_route_fragments_without_duplicate_module_registration@L1219`, `independently_built_contribution_sets_merge_by_owner@L1246`, `duplicate_and_owner_mismatched_routes_fail_assembly@L1278`, `module_id@L1307`, `definition@L1311`, `contract@L1327`, `payload@L1339`, `request@L1353`
- **Public types:** `MutationRoute@L36`, `QueryRoute@L54`, `ModuleRuntimeContribution@L72`, `ModuleContributionSet@L103`, `CompositionError@L200`, `ApplicationCompositionBuilder@L256`, `ApplicationComposition@L366`, `NoopMutationSemanticValidator@L599`, `NoopQuerySemanticValidator@L612`, `ModuleActivationPort@L624`, `ActivationGatedMutationValidator@L633`, `ActivationGatedQueryValidator@L680`, `TenantBackgroundWorker@L723`, `BackgroundWorkerPhase@L734`, `ActivationGatedBackgroundWorker@L754`, `BackgroundCompositionError@L808`, `BackgroundWorkerRegistryBuilder@L841`, `BackgroundWorkerRegistry@L914`
- **Detected tests (8):** `exact_routes_do_not_need_central_capability_switches@L1083`, `inactive_module_fails_before_inner_semantic_validation@L1117`, `background_workers_run_in_stable_coordinate_order@L1133`, `background_worker_phases_precede_module_sort_order@L1166`, `inactive_background_worker_is_skipped_before_inner_execution@L1201`, `contribution_set_merges_route_fragments_without_duplicate_module_registration@L1219`, `independently_built_contribution_sets_merge_by_owner@L1246`, `duplicate_and_owner_mismatched_routes_fail_assembly@L1278`
- **Error/guard literals:** `APPLICATION_BACKGROUND_WORKER_FAILED`, `MODULE_NOT_ACTIVE`, `APPLICATION_COMPOSITION_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_module_sdk`, `crm_query_runtime`

## 03. `crm-application-runtime` — RETIRE-REVIEW / D? + B? invariants

### [`src/background.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/src/background.rs) · 695 lines

- **Public declarations (1):** `build_production_background_workers@L76`
- **Internal/trait/helpers (28):** `add_customer_enrichment_workers@L188`, `add_worker@L221`, `new@L247`, `fmt@L253`, `run_tenant_cycle@L261`, `new@L282`, `fmt@L288`, `run_tenant_cycle@L296`, `new@L318`, `fmt@L324`, `run_tenant_cycle@L333`, `new@L388`, `fmt@L397`, `run_tenant_cycle@L406`, `new@L432`, `fmt@L438`, `run_tenant_cycle@L446`, `new@L472`, `fmt@L478`, `run_tenant_cycle@L486`, `worker_error@L501`, `background_composition_error@L511`, `configuration_error@L521`, `active@L547`, `set@L553`, `is_active@L559`, `run_tenant_cycle@L578`, `enrichment_workers_run_in_phase_order_and_stop_after_disable_or_uninstall@L591`
- **Public types:** `ProductionBackgroundWorkerDependencies@L60`
- **Detected tests (1):** `enrichment_workers_run_in_phase_order_and_stop_after_disable_or_uninstall@L591`
- **Error/guard literals:** `IMPORT_EXECUTION`, `EXPORT_SELECTION`, `SALES_ACTIVITIES_LINK`, `SALES_ACTIVITIES_LINK_DEAD_LETTERED`, `PHASE6_PROJECTION`, `CUSTOMER_360_PROJECTION`, `GLOBAL_SEARCH`, `APPLICATION_BACKGROUND_COMPOSITION_INVALID`, `APPLICATION_BACKGROUND_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_application_composition`, `crm_core_data`, `crm_core_events`, `crm_customer_360_composition`, `crm_customer_data_operations_capability_adapter`, `crm_customer_data_operations_execution_composition`, `crm_customer_enrichment_application_composition`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_materialization_composition`, `crm_customer_enrichment_provider_process_composition`, `crm_customer_privacy_production`, `crm_first_party_modules`, `crm_global_search_composition`, `crm_module_sdk`, `crm_sales_activities_capability_composition`, `crm_sales_activities_link`, `crm_search_query_adapter`

### [`src/bootstrap_visibility.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/src/bootstrap_visibility.rs) · 8 lines


### [`src/bootstrap_visibility/fields.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/src/bootstrap_visibility/fields.rs) · 146 lines

- **Public declarations (16):** `fields@L3`, `sales_fields@L7`, `customer_data_import_job_fields@L24`, `customer_data_import_row_fields@L28`, `customer_360_party_fields@L40`, `customer_360_account_fields@L44`, `customer_360_contact_point_fields@L48`, `customer_360_party_relationship_fields@L60`, `party_fields@L64`, `account_fields@L68`, `contact_point_fields@L72`, `consent_fields@L85`, `identity_resolution_fields@L102`, `identity_resolution_merge_fields@L111`, `party_relationship_fields@L121`, `task_fields@L131`

### [`src/bootstrap_visibility/registry.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/src/bootstrap_visibility/registry.rs) · 350 lines

- **Public declarations (2):** `resources_for@L68`, `build_bootstrap_visibility_registry@L80`
- **Internal/trait/helpers (18):** `register@L135`, `resource@L152`, `sales_visibility@L164`, `activities_visibility@L168`, `parties_visibility@L176`, `accounts_visibility@L181`, `contact_points_visibility@L189`, `consents_visibility@L194`, `identity_resolution_visibility@L202`, `party_relationships_visibility@L217`, `customer_data_operations_visibility@L225`, `customer_enrichment_visibility@L243`, `customer_privacy_visibility@L252`, `data_quality_visibility@L288`, `no_visibility@L296`, `customer_360_visibility@L300`, `search_visibility@L328`, `unsupported_owner@L341`
- **Public types:** `VisibilityProvider@L53`, `BootstrapVisibilityResource@L56`, `BootstrapVisibilityRegistry@L63`
- **Error/guard literals:** `APPLICATION_BOOTSTRAP_VISIBILITY_DUPLICATE`, `APPLICATION_BOOTSTRAP_VISIBILITY_OWNER_UNSUPPORTED`
- **Rust crate references:** `crm_capability_runtime`, `crm_consents_capability_adapter`, `crm_contact_points_capability_composition`, `crm_customer_accounts_capability_adapter`, `crm_customer_data_operations_capability_adapter`, `crm_customer_data_operations_query_adapter`, `crm_customer_enrichment`, `crm_customer_enrichment_visibility`, `crm_customer_privacy_production`, `crm_first_party_modules`, `crm_identity_resolution_capability_adapter`, `crm_metadata_api_adapter`, `crm_module_sdk`, `crm_party_reference_composition`, `crm_party_relationships_capability_adapter`, `crm_sales_activities_query_adapter`, `crm_search_query_adapter`

### [`src/bootstrap_visibility/tests.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/src/bootstrap_visibility/tests.rs) · 146 lines

- **Internal/trait/helpers (5):** `definition@L20`, `contract@L36`, `registry_resolves_module_owned_resources_without_owner_switches@L49`, `registry_resolves_all_four_customer_privacy_queries@L90`, `registry_rejects_undeclared_query_owner@L136`
- **Detected tests (3):** `registry_resolves_module_owned_resources_without_owner_switches@L49`, `registry_resolves_all_four_customer_privacy_queries@L90`, `registry_rejects_undeclared_query_owner@L136`
- **Error/guard literals:** `APPLICATION_BOOTSTRAP_VISIBILITY_OWNER_UNSUPPORTED`
- **Rust crate references:** `crm_capability_runtime`, `crm_customer_data_operations_capability_adapter`, `crm_customer_data_operations_query_adapter`, `crm_customer_enrichment`, `crm_customer_enrichment_visibility`, `crm_customer_privacy_production`, `crm_module_sdk`

### [`src/config.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/src/config.rs) · 559 lines

- **Public declarations (2):** `from_env@L66`, `validate@L98`
- **Internal/trait/helpers (19):** `fmt@L146`, `required@L184`, `secret@L191`, `parse_or_default@L199`, `parse_bool@L212`, `parse_tenants@L223`, `parse_retention_policies@L239`, `parse_provider_adapters@L278`, `validate_provider_adapters@L326`, `valid_provider_seconds@L394`, `canonical_transport_key@L398`, `canonical_handle_alias@L409`, `canonical_environment_name@L416`, `provider_adapter_config_invalid@L428`, `rejects_listener_collision_and_short_secrets@L437`, `retention_policy_configuration_is_explicit_and_bounded@L448`, `provider_adapter_configuration_is_exact_explicit_and_secret_indirect@L459`, `provider_adapter_configuration_rejects_ambiguity_and_unknown_tenants@L503`, `valid_config@L537`
- **Public types:** `CustomerEnrichmentProviderAdapterState@L20`, `CustomerEnrichmentProviderCredentialBinding@L26`, `CustomerEnrichmentProviderAdapterConfig@L33`, `ApplicationConfig@L45`, `ApplicationConfigError@L140`
- **Detected tests (4):** `rejects_listener_collision_and_short_secrets@L437`, `retention_policy_configuration_is_explicit_and_bounded@L448`, `provider_adapter_configuration_is_exact_explicit_and_secret_indirect@L459`, `provider_adapter_configuration_rejects_ambiguity_and_unknown_tenants@L503`
- **Error/guard literals:** `CRM_CUSTOMER_ENRICHMENT_PROVIDER_ADAPTERS`, `CRM_API_TENANTS`, `CRM_DATABASE_URL`, `CRM_HTTP_BIND`, `CRM_GRPC_BIND`, `CRM_API_BEARER_TOKEN`, `CRM_API_ACTOR_ID`, `CRM_CURSOR_SIGNING_KEY`, `CRM_APPROVAL_SIGNING_KEY`, `CRM_DEFAULT_TIMEOUT_MILLIS`, `CRM_MAXIMUM_TIMEOUT_MILLIS`, `CRM_QUERY_DEFAULT_PAGE_SIZE`, `CRM_QUERY_MAXIMUM_PAGE_SIZE`, `CRM_QUERY_SCAN_MULTIPLIER`, `CRM_DATABASE_MAX_CONNECTIONS`, `CRM_BOOTSTRAP_ALLOW_PHASE6`, `CRM_EXPORT_RETENTION_POLICIES`, `REGISTRY_PRIMARY_TOKEN`
- **Rust crate references:** `crm_customer_enrichment`, `crm_module_sdk`

### [`src/customer_enrichment_application_worker.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/src/customer_enrichment_application_worker.rs) · 768 lines

- **Public declarations (1):** `build_customer_enrichment_application_worker@L78`
- **Internal/trait/helpers (31):** `fmt@L64`, `new@L135`, `authorize@L139`, `fmt@L218`, `new@L233`, `fmt@L245`, `evaluate@L255`, `new@L289`, `fmt@L303`, `apply@L314`, `outcome_authorization_identity@L334`, `new@L367`, `fmt@L379`, `get@L389`, `new@L450`, `validate_party@L462`, `validate_consent@L500`, `fmt@L526`, `evaluate@L537`, `authorize_query@L564`, `consent_get_query_request@L586`, `validate_consent_authorization@L633`, `policy_denied@L676`, `policy_decision_id@L684`, `canonical_token@L710`, `configuration_error@L717`, `configuration_invalid@L721`, `party_snapshot_unavailable@L731`, `party_snapshot_invalid@L740`, `policy_denied_error@L749`, `policy_invalid@L759`
- **Public types:** `CustomerEnrichmentApplicationWorkerDependencies@L52`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_APPLICATION_OUTCOME_PERMISSION_DENIED`, `CUSTOMER_ENRICHMENT_APPLICATION_PARTY_PERMISSION_DENIED`, `CUSTOMER_ENRICHMENT_APPLICATION_CONSENT_PERMISSION_DENIED`, `CUSTOMER_ENRICHMENT_APPLICATION_CONFIGURATION_INVALID`, `CUSTOMER_ENRICHMENT_APPLICATION_PARTY_UNAVAILABLE`, `CUSTOMER_ENRICHMENT_APPLICATION_PARTY_STATE_INVALID`, `CUSTOMER_ENRICHMENT_APPLICATION_POLICY_DENIED`, `CUSTOMER_ENRICHMENT_APPLICATION_POLICY_INVALID`
- **Rust crate references:** `crm_capability_ingress`, `crm_capability_runtime`, `crm_consents_capability_adapter`, `crm_consents_query_adapter`, `crm_core_data`, `crm_customer_enrichment`, `crm_customer_enrichment_application_adapter`, `crm_customer_enrichment_application_composition`, `crm_customer_enrichment_capability_adapter`, `crm_module_sdk`, `crm_parties_query_adapter`, `crm_proto_contracts`, `crm_query_runtime`

### [`src/customer_enrichment_materialization_process.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/src/customer_enrichment_materialization_process.rs) · 115 lines

- **Public declarations (1):** `build_customer_enrichment_materialization_process@L29`
- **Internal/trait/helpers (5):** `fmt@L20`, `new@L63`, `fmt@L72`, `execute@L82`, `materialization_configuration_invalid@L106`
- **Public types:** `CustomerEnrichmentMaterializationProcessDependencies@L14`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_MATERIALIZATION_PERMISSION_DENIED`, `CUSTOMER_ENRICHMENT_MATERIALIZATION_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment_materialization_adapter`, `crm_customer_enrichment_materialization_composition`, `crm_module_sdk`

### [`src/customer_enrichment_provider_process.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/src/customer_enrichment_provider_process.rs) · 81 lines

- **Public declarations (1):** `build_customer_enrichment_provider_process@L33`
- **Internal/trait/helpers (5):** `fmt@L22`, `configuration_error@L56`, `canonical_worker_actor_is_valid@L71`, `dependencies_are_thread_safe@L76`, `assert_send_sync@L77`
- **Public types:** `CustomerEnrichmentProviderProcessDependencies@L13`
- **Detected tests (3):** `canonical_worker_actor_is_valid@L71`, `dependencies_are_thread_safe@L76`, `assert_send_sync@L77`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_PROVIDER_PROCESS_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_core_data`, `crm_customer_enrichment_provider_process_composition`, `crm_module_sdk`, `crm_parties_query_adapter`, `crm_query_runtime`

### [`src/customer_enrichment_provider_registry.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/src/customer_enrichment_provider_registry.rs) · 593 lines

- **Public declarations (3):** `try_new@L75`, `build_process_customer_enrichment_provider_transport_catalog@L140`, `build_customer_enrichment_provider_registry@L233`
- **Internal/trait/helpers (27):** `resolve_exact@L36`, `resolve@L46`, `fmt@L58`, `fmt@L101`, `resolve_exact@L125`, `registry_http_coordinate@L189`, `required_process_value@L197`, `optional_process_value@L204`, `resolve@L221`, `seconds_to_nanos@L293`, `validate_transport_key@L301`, `provider_transport_unavailable@L318`, `provider_process_configuration_missing@L336`, `provider_process_configuration_invalid@L346`, `provider_secret_configuration_missing@L356`, `provider_secret_configuration_invalid@L366`, `provider_configuration_invalid@L376`, `resolve@L403`, `dispatch@L415`, `coordinate@L441`, `enabled_config@L445`, `empty_configuration_stays_fail_closed@L463`, `disabled_coordinate_needs_no_transport_or_secret@L477`, `enabled_coordinate_requires_exact_transport_and_secret_environment@L504`, `unknown_transport_and_missing_secret_fail_startup_without_value_leakage@L533`, `transport_catalog_does_not_fallback_across_versions@L572`, `_assert_dispatch_request_is_send@L591`
- **Public types:** `ProviderTransportCatalogPort@L35`, `ProviderSecretValueSourcePort@L45`, `ProviderTransportRegistration@L51`, `StaticProviderTransportCatalog@L70`, `ProcessProviderSecretValueSource@L218`
- **Detected tests (5):** `empty_configuration_stays_fail_closed@L463`, `disabled_coordinate_needs_no_transport_or_secret@L477`, `enabled_coordinate_requires_exact_transport_and_secret_environment@L504`, `unknown_transport_and_missing_secret_fail_startup_without_value_leakage@L533`, `transport_catalog_does_not_fallback_across_versions@L572`
- **Error/guard literals:** `CRM_CUSTOMER_ENRICHMENT_REGISTRY_HTTP_ENDPOINT`, `CRM_CUSTOMER_ENRICHMENT_REGISTRY_HTTP_ALLOWED_ENDPOINTS`, `CRM_CUSTOMER_ENRICHMENT_REGISTRY_HTTP_TIMEOUT_MILLIS`, `CRM_CUSTOMER_ENRICHMENT_REGISTRY_HTTP_MAXIMUM_REQUEST_BYTES`, `CRM_CUSTOMER_ENRICHMENT_REGISTRY_HTTP_MAXIMUM_RESPONSE_BYTES`, `CUSTOMER_ENRICHMENT_PROVIDER_TRANSPORT_UNAVAILABLE`, `CUSTOMER_ENRICHMENT_PROVIDER_HOST_CONFIGURATION_MISSING`, `CUSTOMER_ENRICHMENT_PROVIDER_HOST_CONFIGURATION_INVALID`, `CUSTOMER_ENRICHMENT_PROVIDER_SECRET_CONFIGURATION_MISSING`, `CUSTOMER_ENRICHMENT_PROVIDER_SECRET_CONFIGURATION_INVALID`, `CUSTOMER_ENRICHMENT_PROVIDER_CONFIGURATION_INVALID`, `REGISTRY_PRIMARY_TOKEN`, `CUSTOMER_ENRICHMENT_PROVIDER_ADAPTER_DISABLED`
- **Rust crate references:** `crm_customer_enrichment`, `crm_customer_enrichment_provider_registry`, `crm_customer_enrichment_registry_http_transport`, `crm_module_sdk`

### [`src/customer_enrichment_provider_source.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/src/customer_enrichment_provider_source.rs) · 314 lines

- **Public declarations (1):** `new@L33`
- **Internal/trait/helpers (14):** `fmt@L47`, `load@L58`, `request_status_requires_provider@L213`, `module_id@L223`, `record_type@L227`, `source_identifier_invalid@L231`, `source_configuration_invalid@L235`, `source_snapshot_invalid@L245`, `request_unavailable@L255`, `provider_profile_unavailable@L264`, `party_unavailable@L273`, `only_pre_response_states_require_provider_execution@L287`, `source_is_thread_safe@L309`, `assert_send_sync@L310`
- **Public types:** `GovernedCustomerEnrichmentProviderSource@L26`
- **Detected tests (3):** `only_pre_response_states_require_provider_execution@L287`, `source_is_thread_safe@L309`, `assert_send_sync@L310`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_PROVIDER_PARTY_PERMISSION_DENIED`, `CUSTOMER_ENRICHMENT_PROVIDER_PARTY_VERSION_CHANGED`, `CUSTOMER_ENRICHMENT_PROVIDER_PARTY_FIELD_NOT_VISIBLE`, `CUSTOMER_ENRICHMENT_PROVIDER_SOURCE_CONFIGURATION_INVALID`, `CUSTOMER_ENRICHMENT_PROVIDER_SOURCE_SNAPSHOT_INVALID`, `CUSTOMER_ENRICHMENT_PROVIDER_REQUEST_UNAVAILABLE`, `CUSTOMER_ENRICHMENT_PROVIDER_PROFILE_UNAVAILABLE`, `CUSTOMER_ENRICHMENT_PROVIDER_PARTY_UNAVAILABLE`
- **Rust crate references:** `crm_core_data`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_provider_process_composition`, `crm_module_sdk`, `crm_parties_query_adapter`, `crm_proto_contracts`, `crm_query_runtime`

### [`src/customer_enrichment_provider_worker.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/src/customer_enrichment_provider_worker.rs) · 112 lines

- **Public declarations (1):** `build_customer_enrichment_provider_worker@L32`
- **Internal/trait/helpers (4):** `fmt@L22`, `new@L64`, `fmt@L78`, `execute@L89`
- **Public types:** `CustomerEnrichmentProviderWorkerDependencies@L15`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_DISPATCH_PERMISSION_DENIED`, `CUSTOMER_ENRICHMENT_RESPONSE_PERMISSION_DENIED`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_worker_composition`, `crm_module_sdk`

### [`src/customer_enrichment_reject_promotion.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/src/customer_enrichment_reject_promotion.rs) · 492 lines

- **Public declarations (2):** `application_mutation_definitions@L38`, `build_production_composition@L48`
- **Internal/trait/helpers (18):** `validate@L119`, `validate_review_binding@L191`, `new@L234`, `fmt@L240`, `execute@L252`, `new@L279`, `fmt@L291`, `evaluate@L301`, `policy_denied@L370`, `policy_decision_id@L382`, `canonical_evidence@L409`, `validate_token@L416`, `validate_reference@L427`, `cursor@L441`, `review_input_invalid@L453`, `review_policy_invalid@L463`, `composition_error@L473`, `configuration_error@L483`
- **Re-exports:** `pub use suggestion_reads::application_query_definitions;`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_SUGGESTION_REVIEW_TARGET_STALE`, `APPLICATION_CURSOR_CONFIGURATION_INVALID`, `CUSTOMER_ENRICHMENT_SUGGESTION_REVIEW_INPUT_INVALID`, `CUSTOMER_ENRICHMENT_SUGGESTION_REVIEW_POLICY_INVALID`, `APPLICATION_COMPOSITION_INVALID`, `APPLICATION_COMPOSITION_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_application_composition`, `crm_capability_runtime`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_review_adapter`, `crm_customer_enrichment_review_composition`, `crm_module_sdk`, `crm_parties_query_adapter`, `crm_proto_contracts`, `crm_query_runtime`

### [`src/customer_enrichment_suggestion_list_promotion.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/src/customer_enrichment_suggestion_list_promotion.rs) · 112 lines

- **Public declarations (2):** `application_query_definitions@L15`, `build_production_composition@L25`
- **Internal/trait/helpers (3):** `cursor@L81`, `composition_error@L93`, `configuration_error@L103`
- **Error/guard literals:** `APPLICATION_CURSOR_CONFIGURATION_INVALID`, `APPLICATION_COMPOSITION_INVALID`, `APPLICATION_COMPOSITION_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_application_composition`, `crm_capability_runtime`, `crm_customer_enrichment_suggestion_query_adapter`, `crm_module_sdk`, `crm_query_runtime`

### [`src/customer_privacy_access_export.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/src/customer_privacy_access_export.rs) · 72 lines

- **Public declarations (2):** `new@L17`, `build_customer_privacy_access_export@L63`
- **Internal/trait/helpers (1):** `request@L23`
- **Public types:** `CustomerDataPrivacyExportTarget@L12`
- **Rust crate references:** `crm_customer_data_operations_execution_composition`, `crm_customer_privacy_production`, `crm_module_sdk`

### [`src/customer_privacy_case_create_promotion.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/src/customer_privacy_case_create_promotion.rs) · 165 lines

- **Public declarations (3):** `application_mutation_definitions@L31`, `application_query_definitions@L39`, `build_production_composition@L51`
- **Internal/trait/helpers (2):** `composition_error@L146`, `configuration_error@L156`
- **Re-exports:** `pub use base_runtime::PRODUCTION_REVIEW_POLICY_VERSION;`
- **Error/guard literals:** `APPLICATION_COMPOSITION_INVALID`, `APPLICATION_COMPOSITION_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_application_composition`, `crm_capability_runtime`, `crm_contact_points_capability_composition`, `crm_core_data`, `crm_customer_privacy_production`, `crm_module_sdk`, `crm_party_reference_composition`

### [`src/export_artifact_download_http.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/src/export_artifact_download_http.rs) · 166 lines

- **Public declarations (1):** `export_artifact_download_router@L15`
- **Internal/trait/helpers (7):** `download_export_artifact@L26`, `success_response@L77`, `error_response@L116`, `optional_header@L130`, `invalid_request@L137`, `hex@L145`, `invalid_timeout_is_rejected_before_disclosure@L159`
- **Detected tests (1):** `invalid_timeout_is_rejected_before_disclosure@L159`
- **Rust crate references:** `crm_module_sdk`

### [`src/export_artifact_download.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/src/export_artifact_download.rs) · 476 lines

- **Public declarations (3):** `new@L71`, `definition@L93`, `download@L97`
- **Internal/trait/helpers (18):** `fmt@L57`, `download_authorized@L142`, `disclosure_execution_context@L211`, `validate_finalized_artifact@L237`, `disclosure_audit_intent@L259`, `authentication_error@L321`, `context_error@L330`, `validate_retention_policies@L352`, `artifact_expires_at_unix_nanos@L368`, `artifact_expired@L385`, `retention_policy_unavailable@L394`, `retention_configuration_error@L403`, `download_timeout@L412`, `integrity_error@L421`, `identifier_error@L431`, `hex@L441`, `retention_expiry_is_exact_and_unknown_policies_fail_closed@L455`, `disclosure_definition_is_job_bound_high_risk_read@L466`
- **Public types:** `PartyExportArtifactDownloadRequest@L26`, `PartyExportArtifactDownloadResult@L37`, `PartyExportArtifactDownloadService@L45`
- **Detected tests (2):** `retention_expiry_is_exact_and_unknown_policies_fail_closed@L455`, `disclosure_definition_is_job_bound_high_risk_read@L466`
- **Error/guard literals:** `CUSTOMER_DATA_EXPORT_ARTIFACT_DOWNLOAD_PERMISSION_DENIED`, `CUSTOMER_DATA_EXPORT_ARTIFACT_AUDIT_SERIALIZATION_FAILED`, `CUSTOMER_DATA_EXPORT_ARTIFACT_EXPIRED`, `CUSTOMER_DATA_EXPORT_RETENTION_POLICY_UNAVAILABLE`, `CUSTOMER_DATA_EXPORT_RETENTION_CONFIGURATION_INVALID`, `CUSTOMER_DATA_EXPORT_ARTIFACT_DOWNLOAD_TIMEOUT`, `CUSTOMER_DATA_EXPORT_ARTIFACT_INTEGRITY_INVALID`, `CUSTOMER_DATA_EXPORT_ARTIFACT_CONTEXT_INVALID`
- **Rust crate references:** `crm_capability_ingress`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_data_operations_capability_adapter`, `crm_customer_data_operations_query_adapter`, `crm_module_sdk`, `crm_query_runtime`

### [`src/export_execution_source.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/src/export_execution_source.rs) · 139 lines

- **Public declarations (2):** `new@L30`, `export_execution_source_kind@L38`
- **Internal/trait/helpers (5):** `fmt@L20`, `get@L48`, `assert_send_sync@L120`, `execution_kind_mapping_is_closed_and_exact@L123`, `governed_execution_source_is_thread_safe@L135`
- **Public types:** `GovernedPartyExportExecutionSource@L14`
- **Detected tests (2):** `execution_kind_mapping_is_closed_and_exact@L123`, `governed_execution_source_is_thread_safe@L135`
- **Error/guard literals:** `CUSTOMER_DATA_EXPORT_EXECUTION_SOURCE_PERMISSION_DENIED`
- **Rust crate references:** `crm_customer_data_operations_execution_composition`, `crm_module_sdk`, `crm_parties_query_adapter`, `crm_query_runtime`

### [`src/export_selection_bootstrap.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/src/export_selection_bootstrap.rs) · 233 lines

- **Public declarations (1):** `bootstrap_export_selection_worker_access@L31`
- **Internal/trait/helpers (3):** `find_query@L158`, `grant_capabilities@L177`, `grant_visibility@L202`
- **Public types:** `ExportSelectionWorkerAccess@L24`
- **Rust crate references:** `crm_capability_adapters`, `crm_capability_runtime`, `crm_customer_data_operations_capability_adapter`, `crm_customer_data_operations_execution_composition`, `crm_customer_data_operations_query_adapter`, `crm_module_sdk`, `crm_parties_query_adapter`, `crm_party_reference_composition`

### [`src/export_selection_source.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/src/export_selection_source.rs) · 225 lines

- **Public declarations (1):** `new@L40`
- **Internal/trait/helpers (7):** `fmt@L29`, `list_page@L52`, `get@L130`, `capability_authorizer@L202`, `assert_send_sync@L211`, `source_kind_contract_is_closed_and_exact@L214`, `governed_two_phase_export_source_is_thread_safe@L221`
- **Public types:** `GovernedPartyExportSelectionSource@L22`
- **Detected tests (2):** `source_kind_contract_is_closed_and_exact@L214`, `governed_two_phase_export_source_is_thread_safe@L221`
- **Error/guard literals:** `CUSTOMER_DATA_EXPORT_SELECTION_SOURCE_PERMISSION_DENIED`, `CUSTOMER_DATA_EXPORT_EXECUTION_SOURCE_PERMISSION_DENIED`
- **Rust crate references:** `crm_capability_adapters`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_data_operations_execution_composition`, `crm_module_sdk`, `crm_parties_query_adapter`, `crm_query_runtime`

### [`src/gateway_grpc.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/src/gateway_grpc.rs) · 293 lines

- **Public declarations (1):** `new@L32`
- **Internal/trait/helpers (13):** `mutate@L41`, `query@L73`, `verify_pilot_cookie@L99`, `decode_route@L129`, `decode_payload@L148`, `decode_approval@L175`, `encode_mutation_result@L203`, `encode_query_result@L221`, `encode_payload@L229`, `parse_data_class@L246`, `data_class_name@L261`, `parse_encoding@L275`, `encoding_name@L285`
- **Public types:** `ApplicationGatewayService@L27`
- **Re-exports:** `pub use super::application_gateway_service_server::ApplicationGatewayServiceServer as ApplicationGatewayServer;`
- **Rust crate references:** `crm_capability_ingress`, `crm_capability_runtime`, `crm_module_sdk`, `crm_query_runtime`

### [`src/generated_contract_telemetry.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/src/generated_contract_telemetry.rs) · 4 lines


### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/src/lib.rs) · 72 lines

- **Public declarations (1):** `declared_business_module_ids@L67`
- **Re-exports:** `pub use {`
- **Rust crate references:** `crm_data_quality_source_composition`

### [`src/native_composition.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/src/native_composition.rs) · 234 lines

- **Public declarations (6):** `new@L53`, `application_mutation_definitions@L80`, `application_capability_catalog@L98`, `application_query_definitions@L102`, `build_production_composition@L119`, `declared_business_module_ids@L214`
- **Internal/trait/helpers (4):** `is_active@L59`, `cursor@L182`, `composition_error@L194`, `configuration_error@L204`
- **Public types:** `PostgresModuleActivation@L48`, `ProductionCompositionDependencies@L68`
- **Error/guard literals:** `APPLICATION_CURSOR_CONFIGURATION_INVALID`, `APPLICATION_COMPOSITION_INVALID`, `APPLICATION_COMPOSITION_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_application_composition`, `crm_capability_adapters`, `crm_capability_runtime`, `crm_core_data`, `crm_first_party_modules`, `crm_global_search_composition`, `crm_metadata_api_adapter`, `crm_metadata_query_adapter`, `crm_module_sdk`, `crm_query_runtime`, `crm_sales_activities_link`, `crm_search_query_adapter`, `crm_search_runtime`

### [`src/platform.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/src/platform.rs) · 84 lines

- **Internal/trait/helpers (5):** `now_unix_nanos@L10`, `default@L29`, `fill_bytes@L37`, `identity_unavailable@L60`, `process_identity_source_fills_distinct_non_secret_id_material@L74`
- **Public types:** `SystemClock@L7`, `ProcessIdentitySource@L24`
- **Detected tests (1):** `process_identity_source_fills_distinct_non_secret_id_material@L74`
- **Error/guard literals:** `PROCESS_IDENTITY_UNAVAILABLE`
- **Rust crate references:** `crm_module_sdk`

### [`src/process.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/src/process.rs) · 20 lines

- **Public declarations (1):** `run_from_env@L9`

### [`src/production_identity.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/src/production_identity.rs) · 522 lines

- **Public declarations (8):** `from_environment@L135`, `provision@L198`, `public_origin@L263`, `sign_in@L267`, `authorize@L307`, `sign_out@L346`, `set_cookie@L365`, `clear_cookie@L368`
- **Internal/trait/helpers (12):** `now_nanos@L74`, `sha@L81`, `constant_equal@L88`, `secret@L99`, `cookie_token@L111`, `duplicate_cookie_is_rejected@L377`, `secret_is_random_and_cookie_flags_are_strict@L401`, `fixture@L410`, `two_operators_get_distinct_tenant_bound_sessions_and_logout_revokes@L444`, `cookie_session_uses_existing_tenant_bound_authenticator_and_revocation@L469`, `expired_sessions_fail_closed@L499`, `compares_digest_in_constant_time_shape@L517`
- **Public types:** `PilotOperator@L34`, `PilotIdentity@L50`, `PilotSessionSnapshot@L59`, `PilotAuthenticated@L69`
- **Detected tests (6):** `duplicate_cookie_is_rejected@L377`, `secret_is_random_and_cookie_flags_are_strict@L401`, `two_operators_get_distinct_tenant_bound_sessions_and_logout_revokes@L444`, `cookie_session_uses_existing_tenant_bound_authenticator_and_revocation@L469`, `expired_sessions_fail_closed@L499`, `compares_digest_in_constant_time_shape@L517`
- **Error/guard literals:** `CRM_PILOT_OPERATORS`, `CRM_PUBLIC_ORIGIN`
- **Rust crate references:** `crm_capability_adapters`, `crm_capability_ingress`, `crm_capability_runtime`, `crm_module_sdk`

### [`src/runtime.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/src/runtime.rs) · 1325 lines

- **Public declarations (3):** `is_ready@L117`, `assemble@L129`, `run_until_signal@L545`
- **Internal/trait/helpers (33):** `fmt@L109`, `fmt@L593`, `into_parts@L641`, `decode_http_approval@L651`, `run_http_server@L664`, `run_grpc_server@L696`, `health@L721`, `metrics@L725`, `ready@L729`, `login_page@L749`, `verified_origin@L755`, `login@L762`, `auth_session@L786`, `logout@L803`, `resolve_cookie_session@L831`, `mutation@L858`, `query@L893`, `capability_route@L923`, `bad_route@L938`, `bad_approval@L946`, `governed_json_response@L954`, `governed_error_response@L963`, `background_worker_loop@L969`, `run_background_cycle@L1001`, `bootstrap_application_access@L1015`, `bootstrap_import_execution_worker_access@L1060`, `bootstrap_customer_enrichment_provider_process_access@L1107`, `bootstrap_customer_enrichment_materialization_process_access@L1163`, `bootstrap_customer_enrichment_application_worker_access@L1195`, `upsert_bootstrap_visibility@L1275`, `expiry@L1302`, `join_task@L1308`, `expiry_is_strictly_after_now@L1321`
- **Public types:** `ApplicationComponents@L91`, `ApplicationRuntime@L123`, `ApplicationRuntimeError@L584`
- **Detected tests (1):** `expiry_is_strictly_after_now@L1321`
- **Rust crate references:** `crm_application_composition`, `crm_capability_adapters`, `crm_capability_ingress`, `crm_capability_runtime`, `crm_consents_query_adapter`, `crm_core_data`, `crm_customer_360_composition`, `crm_customer_data_operations_capability_adapter`, `crm_customer_data_operations_execution_composition`, `crm_customer_data_operations_query_adapter`, `crm_customer_enrichment_application_adapter`, `crm_customer_enrichment_application_composition`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_materialization_adapter`, `crm_customer_enrichment_materialization_composition`, `crm_customer_enrichment_provider_process_composition`, `crm_global_search_composition`, `crm_module_sdk`, `crm_parties_query_adapter`, `crm_party_reference_composition`, `crm_query_runtime`, `crm_sales_activities_capability_composition`

### [`tests/customer_enrichment_provider_source_contract.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/tests/customer_enrichment_provider_source_contract.rs) · 14 lines

- **Internal/trait/helpers (2):** `governed_provider_source_is_a_thread_safe_internal_process_port@L5`, `assert_contract@L6`
- **Detected tests (2):** `governed_provider_source_is_a_thread_safe_internal_process_port@L5`, `assert_contract@L6`
- **Rust crate references:** `crm_application_runtime`, `crm_customer_enrichment_provider_process_composition`

### [`tests/customer_enrichment_provider_transport_isolation_contract.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/tests/customer_enrichment_provider_transport_isolation_contract.rs) · 134 lines

- **Internal/trait/helpers (5):** `dispatch@L29`, `resolve@L50`, `exact_host_transport_mismatches_fail_before_secret_or_provider_io@L57`, `coordinate@L111`, `enabled_config@L115`
- **Detected tests (1):** `exact_host_transport_mismatches_fail_before_secret_or_provider_io@L57`
- **Error/guard literals:** `REGISTRY_PRIMARY_TOKEN`, `CUSTOMER_ENRICHMENT_PROVIDER_TRANSPORT_UNAVAILABLE`
- **Rust crate references:** `crm_application_runtime`, `crm_customer_enrichment`, `crm_customer_enrichment_provider_registry`, `crm_module_sdk`

### [`tests/customer_enrichment_registration_contract.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/tests/customer_enrichment_registration_contract.rs) · 78 lines

- **Internal/trait/helpers (3):** `definition_publications_request_lifecycle_and_reviews_are_the_composed_enrichment_mutations@L15`, `production_queries_have_exact_module_owned_visibility_parity@L48`, `customer_enrichment_is_a_declared_business_module@L75`
- **Detected tests (3):** `definition_publications_request_lifecycle_and_reviews_are_the_composed_enrichment_mutations@L15`, `production_queries_have_exact_module_owned_visibility_parity@L48`, `customer_enrichment_is_a_declared_business_module@L75`
- **Rust crate references:** `crm_application_runtime`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_review_adapter`, `crm_customer_enrichment_visibility`

### [`tests/customer_privacy_access_export_postgres.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/tests/customer_privacy_access_export_postgres.rs) · 843 lines

- **Internal/trait/helpers (19):** `repository_step_10_access_export_recovers_finalized_artifact_before_case_link@L43`, `build_case_plan_and_decision@L300`, `owner_invocation@L392`, `access_invocation@L413`, `action_plan_payload@L431`, `seed_tenant_and_actor@L446`, `seed_record@L483`, `seed_transaction@L537`, `reference_version@L565`, `reference_count@L576`, `privacy_export_job_count@L586`, `customer_privacy_access_export_evidence_counts@L596`, `cleanup@L623`, `data_class_name@L655`, `artifact_count@L675`, `create@L681`, `append_chunk@L730`, `finalize@L779`, `read_finalized@L813`
- **Detected tests (1):** `repository_step_10_access_export_recovers_finalized_artifact_before_case_link@L43`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`, `CUSTOMER_PRIVACY_CASE_NOT_FOUND`, `CUSTOMER_PRIVACY_ACCESS_EXPORT_CONFLICT`, `ACCESS_DISCOVERY`, `FILE_ARTIFACT_CONFLICT`, `FILE_ARTIFACT_NOT_FOUND`, `FILE_ARTIFACT_CHUNK_CONFLICT`, `FILE_ARTIFACT_FINALIZE_CONFLICT`, `FILE_ARTIFACT_NOT_FINALIZED`
- **Rust crate references:** `crm_core_data`, `crm_customer_data_operations_execution_composition`, `crm_customer_privacy_production`, `crm_module_sdk`

### [`tests/customer_privacy_hold_retention_postgres.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/tests/customer_privacy_hold_retention_postgres.rs) · 538 lines

- **Internal/trait/helpers (12):** `phase_260_enforces_hold_retention_precedence_replay_and_fail_closed_state@L36`, `build_case_and_plan@L219`, `resource@L323`, `hold@L340`, `action_plan_payload@L362`, `invocation@L377`, `item@L400`, `seed_record@L411`, `insert_fixture_transaction@L460`, `decision_count@L489`, `cleanup@L499`, `data_class_name@L531`
- **Detected tests (1):** `phase_260_enforces_hold_retention_precedence_replay_and_fail_closed_state@L36`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`, `LITIGATION_HOLD`, `CUSTOMER_PRIVACY_RETENTION_EVIDENCE_INVALID`, `CUSTOMER_PRIVACY_PERSISTENCE_ADAPTER_INVALID`, `ERASURE_DISCOVERY`
- **Rust crate references:** `crm_core_data`, `crm_customer_privacy_production`, `crm_module_sdk`

### [`tests/customer_privacy_owner_execution_postgres.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/tests/customer_privacy_owner_execution_postgres.rs) · 826 lines

- **Internal/trait/helpers (18):** `phase_280_owner_execution_is_replay_safe_resumable_and_tenant_bound@L34`, `build_case_plan_and_decision@L412`, `resource@L500`, `action_plan_payload@L512`, `invocation@L527`, `read_context@L552`, `ready@L567`, `position@L583`, `seed_record@L591`, `insert_fixture_transaction@L638`, `assert_checkpoint@L663`, `assert_immutable_evidence@L691`, `checkpoint_count@L717`, `attempt_count@L727`, `outcome_count@L737`, `execution_table_counts@L747`, `cleanup@L762`, `data_class_name@L819`
- **Detected tests (1):** `phase_280_owner_execution_is_replay_safe_resumable_and_tenant_bound@L34`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`, `CUSTOMER_PRIVACY_OWNER_EXECUTION_EVIDENCE_INVALID`, `CUSTOMER_PRIVACY_CASE_NOT_FOUND`, `OWNER_TEMPORARILY_UNAVAILABLE`, `CONFLICTING_REPLAY`, `CUSTOMER_PRIVACY_OWNER_EXECUTION_CONFLICT`, `ERASURE_DISCOVERY`
- **Rust crate references:** `crm_core_data`, `crm_customer_privacy_production`, `crm_module_sdk`

### [`tests/customer_privacy_owner_execution_ready_postgres.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/tests/customer_privacy_owner_execution_ready_postgres.rs) · 414 lines

- **Internal/trait/helpers (8):** `postgres_ready_work_is_bounded_resumable_lineage_exact_and_tenant_bound@L35`, `load_ready@L180`, `build_case_plan_and_decision@L192`, `action_plan_payload@L284`, `seed_record@L308`, `insert_fixture_transaction@L354`, `cleanup@L379`, `data_class_name@L407`
- **Detected tests (1):** `postgres_ready_work_is_bounded_resumable_lineage_exact_and_tenant_bound@L35`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`, `CUSTOMER_PRIVACY_OWNER_EXECUTION_READY_WORK_INVALID`, `ERASURE_DISCOVERY`
- **Rust crate references:** `crm_core_data`, `crm_customer_privacy_production`, `crm_module_sdk`

### [`tests/customer_privacy_production_inventory.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/tests/customer_privacy_production_inventory.rs) · 151 lines

- **Internal/trait/helpers (4):** `customer_privacy_runtime_inventory_is_exactly_nine_mutations_and_seven_queries@L39`, `all_public_privacy_routes_are_runtime_and_worker_inventory_is_unchanged@L92`, `classifications@L141`, `root@L148`
- **Detected tests (2):** `customer_privacy_runtime_inventory_is_exactly_nine_mutations_and_seven_queries@L39`, `all_public_privacy_routes_are_runtime_and_worker_inventory_is_unchanged@L92`
- **Error/guard literals:** `CARGO_MANIFEST_DIR`
- **Rust crate references:** `crm_application_runtime`

### [`tests/customer_privacy_restriction_policy_postgres.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/tests/customer_privacy_restriction_policy_postgres.rs) · 453 lines

- **Internal/trait/helpers (11):** `final_restriction_decision_is_live_tenant_bound_strict_and_lock_safe@L36`, `decide@L183`, `begin_bound@L207`, `bind_context@L218`, `disable_fixture_triggers@L244`, `insert_restriction@L251`, `insert_business_transaction@L287`, `insert_payload@L318`, `cleanup@L362`, `fixture_request@L403`, `request@L411`
- **Detected tests (1):** `final_restriction_decision_is_live_tenant_bound_strict_and_lock_safe@L36`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`, `CUSTOMER_PRIVACY_RESTRICTION_ACTIVE`, `CUSTOMER_PRIVACY_RESTRICTION_STATE_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_privacy_production`, `crm_module_sdk`

### [`tests/data_quality_registration_contract.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/tests/data_quality_registration_contract.rs) · 45 lines

- **Internal/trait/helpers (1):** `application_runtime_registers_exact_data_quality_definition_publications@L11`
- **Detected tests (1):** `application_runtime_registers_exact_data_quality_definition_publications@L11`
- **Rust crate references:** `crm_application_runtime`, `crm_data_quality_source_composition`, `crm_module_sdk`

### [`tests/export_artifact_download_contract.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/tests/export_artifact_download_contract.rs) · 48 lines

- **Internal/trait/helpers (3):** `assert_send_sync@L14`, `disclosure_service_and_request_are_thread_safe@L17`, `disclosure_capability_remains_dedicated_high_risk_read_only_surface@L24`
- **Detected tests (2):** `disclosure_service_and_request_are_thread_safe@L17`, `disclosure_capability_remains_dedicated_high_risk_read_only_surface@L24`
- **Rust crate references:** `crm_application_runtime`, `crm_capability_runtime`, `crm_customer_data_operations_query_adapter`, `crm_module_sdk`

### [`tests/export_selection_runtime_contract.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/tests/export_selection_runtime_contract.rs) · 12 lines

- **Internal/trait/helpers (2):** `assert_send_sync@L4`, `production_export_selection_runtime_types_are_send_sync@L7`
- **Detected tests (1):** `production_export_selection_runtime_types_are_send_sync@L7`
- **Rust crate references:** `crm_application_runtime`, `crm_customer_data_operations_execution_composition`

### [`tests/export_selection_source_contract.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/tests/export_selection_source_contract.rs) · 29 lines

- **Internal/trait/helpers (3):** `assert_source_contract@L10`, `governed_runtime_source_implements_private_export_selection_port@L13`, `continuation_and_kind_contracts_remain_bounded_and_explicit@L18`
- **Detected tests (2):** `governed_runtime_source_implements_private_export_selection_port@L13`, `continuation_and_kind_contracts_remain_bounded_and_explicit@L18`
- **Rust crate references:** `crm_application_runtime`, `crm_customer_data_operations_execution_composition`, `crm_module_sdk`

### [`tests/export_selection_worker_identity_contract.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/tests/export_selection_worker_identity_contract.rs) · 13 lines

- **Internal/trait/helpers (1):** `export_selection_worker_uses_a_dedicated_actor_identity@L6`
- **Detected tests (1):** `export_selection_worker_uses_a_dedicated_actor_identity@L6`
- **Rust crate references:** `crm_customer_data_operations_execution_composition`

### [`tests/import_execution_runtime_surface.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/tests/import_execution_runtime_surface.rs) · 26 lines

- **Internal/trait/helpers (2):** `import_execution_internal_outcomes_are_not_public_application_mutations@L7`, `import_execution_worker_uses_a_dedicated_non_user_actor_identity@L20`
- **Detected tests (2):** `import_execution_internal_outcomes_are_not_public_application_mutations@L7`, `import_execution_worker_uses_a_dedicated_non_user_actor_identity@L20`
- **Rust crate references:** `crm_application_runtime`, `crm_customer_data_operations_execution_composition`

### [`tests/import_source_runtime_surface.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/tests/import_source_runtime_surface.rs) · 24 lines

- **Internal/trait/helpers (1):** `production_catalog_exposes_only_artifact_backed_import_source_mutations@L9`
- **Detected tests (1):** `production_catalog_exposes_only_artifact_backed_import_source_mutations@L9`
- **Rust crate references:** `crm_application_runtime`, `crm_customer_data_operations_source_composition`

### [`tests/party_tombstone_rebuild_convergence_postgres.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/tests/party_tombstone_rebuild_convergence_postgres.rs) · 823 lines

- **Internal/trait/helpers (19):** `production_owner_execution_rebuilds_stale_party_derived_state@L50`, `is_active@L238`, `authorize_visibility@L251`, `seed_party@L265`, `party_record_type@L339`, `build_case_plan_and_decision@L344`, `action_plan_payload@L441`, `seed_tenant_and_actor@L456`, `seed_owner_action_capability@L486`, `seed_record@L509`, `insert_fixture_transaction@L553`, `seed_stale_customer_360@L577`, `assert_legacy_customer_360_stale@L647`, `assert_authoritative_party_minimized@L673`, `assert_customer_360_tombstone@L693`, `assert_search_tombstone@L745`, `authoritative_counts@L784`, `data_class_name@L808`, `unique_id@L817`
- **Detected tests (1):** `production_owner_execution_rebuilds_stale_party_derived_state@L50`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`, `ERASURE_DISCOVERY`
- **Rust crate references:** `crm_application_composition`, `crm_capability_ingress`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_360_composition`, `crm_customer_privacy_production`, `crm_global_search_composition`, `crm_module_sdk`, `crm_party_reference_composition`, `crm_proto_contracts`, `crm_query_runtime`

### [`tests/postgres_customer_enrichment_application_worker.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/tests/postgres_customer_enrichment_application_worker.rs) · 537 lines

- **Internal/trait/helpers (13):** `production_application_worker_uses_governed_party_gateway_and_recovers_after_grant@L65`, `activate_modules@L265`, `accepted_review@L276`, `seed_review@L291`, `review_seed_context@L342`, `worker_authorization_grant@L372`, `worker_party_visibility@L388`, `application_attempt_version@L407`, `party_state@L418`, `authorize_visibility@L479`, `test_configuration_error@L495`, `check@L509`, `verify@L528`
- **Detected tests (1):** `production_application_worker_uses_governed_party_gateway_and_recovers_after_grant@L65`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`, `CUSTOMER_ENRICHMENT_APPLICATION_PARTY_PERMISSION_DENIED`, `CUSTOMER_ENRICHMENT_APPLICATION_OUTCOME_PERMISSION_DENIED`, `CUSTOMER_ENRICHMENT_APPLICATION_WORKER_TEST_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_application_runtime`, `crm_capability_adapters`, `crm_capability_ingress`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment`, `crm_customer_enrichment_application_adapter`, `crm_customer_enrichment_application_composition`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_review_adapter`, `crm_module_sdk`, `crm_parties_query_adapter`, `crm_party_reference_composition`, `crm_proto_contracts`, `crm_query_runtime`

### [`tests/postgres_customer_enrichment_consent_policy.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/tests/postgres_customer_enrichment_consent_policy.rs) · 351 lines

- **Internal/trait/helpers (6):** `production_request_creation_denies_invalid_consent_before_persistence@L47`, `request_command@L257`, `consent_fields@L288`, `request_record_count@L308`, `check@L323`, `verify@L342`
- **Detected tests (1):** `production_request_creation_denies_invalid_consent_before_persistence@L47`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`, `CUSTOMER_ENRICHMENT_REQUEST_CONSENT_DENIED`, `CUSTOMER_ENRICHMENT_CONSENT_PERMISSION_DENIED`
- **Rust crate references:** `crm_application_runtime`, `crm_capability_adapters`, `crm_capability_ingress`, `crm_capability_runtime`, `crm_consents_capability_adapter`, `crm_consents_query_adapter`, `crm_core_data`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_capability_composition`, `crm_module_sdk`, `crm_parties_query_adapter`, `crm_proto_contracts`

### [`tests/postgres_customer_enrichment_provider_credential_isolation_process.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/tests/postgres_customer_enrichment_provider_credential_isolation_process.rs) · 467 lines

- **Internal/trait/helpers (8):** `resolve@L42`, `load@L64`, `forbidden_provider@L125`, `spawn_forbidden_provider@L137`, `wrong_tenant_credential_binding_prevents_provider_io_and_checkpoint@L154`, `process_worker_grant@L333`, `credential_isolation_snapshot@L346`, `credential_isolation_evidence_counts@L425`
- **Detected tests (1):** `wrong_tenant_credential_binding_prevents_provider_io_and_checkpoint@L154`
- **Error/guard literals:** `TEST_PROVIDER_SECRET_MISSING`, `TEST_PROVIDER_SOURCE_LINEAGE_MISMATCH`, `TEST_PROVIDER_REQUEST_READ_FAILED`, `TEST_PROVIDER_REQUEST_MISSING`, `DATABASE_URL`, `ADMIN_DATABASE_URL`, `TEST_WRONG_TENANT_REGISTRY_TOKEN`, `CUSTOMER_ENRICHMENT_PROVIDER_SECRET_UNAVAILABLE`, `PROJECTION_CHECKPOINT_FAILED`
- **Rust crate references:** `crm_application_runtime`, `crm_capability_adapters`, `crm_capability_runtime`, `crm_core_events`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_provider_process_composition`, `crm_customer_enrichment_registry_http_transport`, `crm_module_sdk`

### [`tests/postgres_customer_enrichment_provider_http_process.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/tests/postgres_customer_enrichment_provider_http_process.rs) · 450 lines

- **Internal/trait/helpers (7):** `resolve@L42`, `load@L61`, `exact_host_http_transport_records_one_canonical_process_outcome@L90`, `execute@L307`, `process_worker_grant@L322`, `provider_process_snapshot@L335`, `process_evidence_counts@L410`
- **Detected tests (1):** `exact_host_http_transport_records_one_canonical_process_outcome@L90`
- **Error/guard literals:** `TEST_PROVIDER_SECRET_MISSING`, `TEST_PROVIDER_SOURCE_LINEAGE_MISMATCH`, `DATABASE_URL`, `ADMIN_DATABASE_URL`, `TEST_REGISTRY_PRIMARY_TOKEN`, `TEST_PROVIDER_EXECUTOR_MUST_NOT_RUN`
- **Rust crate references:** `crm_application_runtime`, `crm_capability_adapters`, `crm_capability_runtime`, `crm_core_events`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_provider_process_composition`, `crm_customer_enrichment_registry_http_transport`, `crm_module_sdk`

### [`tests/postgres_customer_enrichment_provider_http_timeout_process.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/tests/postgres_customer_enrichment_provider_http_timeout_process.rs) · 563 lines

- **Internal/trait/helpers (8):** `resolve@L43`, `load@L65`, `timeout_then_success_provider@L127`, `spawn_timeout_then_success_provider@L179`, `timeout_recovery_reuses_exact_provider_key_and_checkpoint_lineage@L202`, `process_worker_grant@L434`, `timeout_process_snapshot@L447`, `timeout_evidence_counts@L523`
- **Detected tests (1):** `timeout_recovery_reuses_exact_provider_key_and_checkpoint_lineage@L202`
- **Error/guard literals:** `TEST_PROVIDER_SECRET_MISSING`, `TEST_PROVIDER_SOURCE_LINEAGE_MISMATCH`, `TEST_PROVIDER_REQUEST_READ_FAILED`, `TEST_PROVIDER_REQUEST_MISSING`, `DATABASE_URL`, `ADMIN_DATABASE_URL`, `TEST_REGISTRY_PRIMARY_TOKEN`, `CUSTOMER_ENRICHMENT_PROVIDER_RETRYABLE_FAILURE`
- **Rust crate references:** `crm_application_runtime`, `crm_capability_adapters`, `crm_capability_runtime`, `crm_core_events`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_provider_process_composition`, `crm_customer_enrichment_registry_http_transport`, `crm_module_sdk`

### [`tests/postgres_customer_enrichment_provider_materialization_http_process.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/tests/postgres_customer_enrichment_provider_materialization_http_process.rs) · 867 lines

- **Internal/trait/helpers (14):** `resolve@L65`, `load@L84`, `governed_registry_provider@L119`, `spawn_governed_registry_provider@L163`, `exact_host_http_response_materializes_one_governed_suggestion@L188`, `execute@L502`, `worker_grant@L517`, `process_fixture@L535`, `seed_provider_dependencies@L606`, `seed_definition_record@L668`, `upload_candidate_evidence@L719`, `provider_candidate@L776`, `process_evidence_counts@L814`, `suggestion_count@L859`
- **Detected tests (1):** `exact_host_http_response_materializes_one_governed_suggestion@L188`
- **Error/guard literals:** `TEST_PROVIDER_SECRET_MISSING`, `TEST_PROVIDER_SOURCE_LINEAGE_MISMATCH`, `DATABASE_URL`, `ADMIN_DATABASE_URL`, `TEST_REGISTRY_PRIMARY_TOKEN`, `CUSTOMER_ENRICHMENT_SUGGESTION_EVIDENCE_UNAVAILABLE`, `TEST_PROVIDER_EXECUTOR_MUST_NOT_RUN`
- **Rust crate references:** `crm_application_runtime`, `crm_capability_adapters`, `crm_capability_runtime`, `crm_core_data`, `crm_core_events`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_materialization_adapter`, `crm_customer_enrichment_materialization_composition`, `crm_customer_enrichment_provider_process_composition`, `crm_customer_enrichment_registry_http_transport`, `crm_module_sdk`, `crm_proto_contracts`

### [`tests/postgres_customer_enrichment_provider_worker.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/tests/postgres_customer_enrichment_provider_worker.rs) · 197 lines

- **Internal/trait/helpers (3):** `provider_worker_requires_dispatch_and_response_grants_and_recovers@L16`, `worker_grant@L167`, `request_status@L180`
- **Detected tests (1):** `provider_worker_requires_dispatch_and_response_grants_and_recovers@L16`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`, `CUSTOMER_ENRICHMENT_DISPATCH_PERMISSION_DENIED`, `CUSTOMER_ENRICHMENT_RESPONSE_PERMISSION_DENIED`
- **Rust crate references:** `crm_application_runtime`, `crm_capability_adapters`, `crm_capability_runtime`, `crm_customer_enrichment_capability_adapter`, `crm_module_sdk`

### [`tests/postgres_customer_enrichment_suggestion_accept.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/tests/postgres_customer_enrichment_suggestion_accept.rs) · 439 lines

- **Internal/trait/helpers (4):** `production_suggestion_acceptance_requires_approval_and_is_replay_safe@L43`, `accept_request@L389`, `check@L411`, `verify@L430`
- **Detected tests (1):** `production_suggestion_acceptance_requires_approval_and_is_replay_safe@L43`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`, `CUSTOMER_ENRICHMENT_APPROVAL_REQUIRED`, `CUSTOMER_ENRICHMENT_SUGGESTION_REVIEW_POLICY_DENIED`, `CUSTOMER_ENRICHMENT_REVIEW_CONFLICT`, `TENANT_FORBIDDEN`, `MODULE_NOT_ACTIVE`, `CAPABILITY_PERMISSION_DENIED`
- **Rust crate references:** `crm_application_runtime`, `crm_capability_adapters`, `crm_capability_ingress`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_review_adapter`, `crm_customer_enrichment_suggestion_query_adapter`, `crm_module_sdk`, `crm_parties_query_adapter`, `crm_proto_contracts`, `crm_query_runtime`

### [`tests/postgres_customer_enrichment_suggestion_get.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/tests/postgres_customer_enrichment_suggestion_get.rs) · 351 lines

- **Internal/trait/helpers (1):** `production_suggestion_queries_are_activation_gated_permission_aware_and_side_effect_free@L33`
- **Detected tests (1):** `production_suggestion_queries_are_activation_gated_permission_aware_and_side_effect_free@L33`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`, `CUSTOMER_ENRICHMENT_SUGGESTION_LIST_CURSOR_INVALID`, `QUERY_PERMISSION_DENIED`, `TENANT_FORBIDDEN`, `MODULE_NOT_ACTIVE`, `CUSTOMER_ENRICHMENT_SUGGESTION_NOT_FOUND`
- **Rust crate references:** `crm_application_runtime`, `crm_capability_adapters`, `crm_capability_ingress`, `crm_core_data`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_suggestion_query_adapter`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`

### [`tests/postgres_customer_enrichment_suggestion_reject.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/tests/postgres_customer_enrichment_suggestion_reject.rs) · 410 lines

- **Internal/trait/helpers (4):** `production_suggestion_rejection_is_policy_bound_atomic_and_replay_safe@L43`, `reject_request@L366`, `check@L382`, `verify@L401`
- **Detected tests (1):** `production_suggestion_rejection_is_policy_bound_atomic_and_replay_safe@L43`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`, `CUSTOMER_ENRICHMENT_SUGGESTION_REVIEW_POLICY_DENIED`, `CUSTOMER_ENRICHMENT_REVIEW_CONFLICT`, `TENANT_FORBIDDEN`, `MODULE_NOT_ACTIVE`, `CAPABILITY_PERMISSION_DENIED`
- **Rust crate references:** `crm_application_runtime`, `crm_capability_adapters`, `crm_capability_ingress`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_review_adapter`, `crm_customer_enrichment_suggestion_query_adapter`, `crm_module_sdk`, `crm_parties_query_adapter`, `crm_proto_contracts`, `crm_query_runtime`

### [`tests/production_route_parity.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/tests/production_route_parity.rs) · 245 lines

- **Internal/trait/helpers (10):** `production_routes_exactly_cover_governed_bindings_and_exact_classifications@L52`, `binding_registry@L145`, `route_classifications@L152`, `root@L159`, `classified_routes@L163`, `unique_modules@L182`, `unique_runtime_routes@L198`, `unique_routes@L207`, `assert_route_kinds_are_disjoint@L221`, `route_coordinate@L238`
- **Detected tests (1):** `production_routes_exactly_cover_governed_bindings_and_exact_classifications@L52`
- **Error/guard literals:** `CARGO_MANIFEST_DIR`
- **Rust crate references:** `crm_application_runtime`, `crm_capability_runtime`

### [`tests/support/customer_enrichment_consent_policy.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/tests/support/customer_enrichment_consent_policy.rs) · 420 lines

- **Public declarations (5):** `suffix@L106`, `authorization_id@L119`, `seed_party@L124`, `seed_definitions@L166`, `seed_consent@L211`
- **Internal/trait/helpers (7):** `is_active@L58`, `authorize_visibility@L71`, `withdraw_consent@L273`, `seed_record@L304`, `seed_context@L345`, `capability_request@L371`, `configuration_error@L411`
- **Public types:** `DefinitionFixture@L88`, `ConsentFixtureKind@L94`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_CONSENT_FIXTURE_INVALID`
- **Rust crate references:** `crm_application_composition`, `crm_capability_ingress`, `crm_capability_runtime`, `crm_consents`, `crm_consents_capability_adapter`, `crm_core_data`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_module_sdk`, `crm_party_reference_composition`, `crm_proto_contracts`, `crm_query_runtime`

### [`tests/support/customer_enrichment_suggestion_get.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/tests/support/customer_enrichment_suggestion_get.rs) · 15 lines

- **Re-exports:** `pub use domain::*;`, `pub use list::*;`, `pub use transport::*;`

### [`tests/support/customer_enrichment_suggestion_get/domain.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/tests/support/customer_enrichment_suggestion_get/domain.rs) · 427 lines

- **Public declarations (5):** `activate_customer_enrichment@L78`, `suggestion@L88`, `seed_suggestion@L177`, `set_installation_status@L368`, `evidence_counts@L411`
- **Internal/trait/helpers (7):** `is_active@L40`, `authorize_visibility@L53`, `seed_party@L225`, `party_request@L290`, `seed_configuration_error@L333`, `seed_context@L343`, `scalar@L421`
- **Public types:** `EvidenceCounts@L70`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_REVIEW_SEED_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_application_composition`, `crm_capability_ingress`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_review_adapter`, `crm_module_sdk`, `crm_party_reference_composition`, `crm_proto_contracts`, `crm_query_runtime`

### [`tests/support/customer_enrichment_suggestion_get/list.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/tests/support/customer_enrichment_suggestion_get/list.rs) · 28 lines

- **Public declarations (1):** `list_request@L9`
- **Rust crate references:** `crm_customer_enrichment`, `crm_customer_enrichment_review_adapter`, `crm_proto_contracts`

### [`tests/support/customer_enrichment_suggestion_get/transport.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-application-runtime/tests/support/customer_enrichment_suggestion_get/transport.rs) · 237 lines

- **Public declarations (14):** `access_token@L27`, `authorization_grant@L31`, `visibility_grant@L44`, `execute_get@L63`, `execute_query@L82`, `execute_mutation@L102`, `success_payload@L156`, `success_mutation_payload@L163`, `assert_error_code@L173`, `assert_mutation_error_code@L180`, `suggestion_fields@L187`, `review_fields@L212`, `tenant@L230`, `actor@L234`
- **Internal/trait/helpers (2):** `authenticated_headers@L128`, `payload@L138`
- **Rust crate references:** `crm_capability_adapters`, `crm_capability_ingress`, `crm_capability_runtime`, `crm_customer_enrichment`, `crm_module_sdk`, `crm_proto_contracts`

## 04. `crm-capability-adapters` — RETIRE-REVIEW / D? + B? invariants

### [`src/approval.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-capability-adapters/src/approval.rs) · 340 lines

- **Public declarations (4):** `from_evidence@L26`, `issue@L101`, `revoke@L121`, `new@L144`
- **Internal/trait/helpers (12):** `validate@L44`, `fmt@L70`, `verify@L150`, `sha256@L183`, `constant_time_equal@L187`, `invalid_approval@L196`, `dependency_error@L206`, `verifies_exact_stored_proof_and_binding@L226`, `revocation_and_wrong_proof_are_rejected@L241`, `approval@L268`, `definition@L281`, `request@L305`
- **Public types:** `ApprovalRecord@L14`, `ApprovalStoreError@L63`, `ApprovalStore@L96`, `StoredApprovalVerifier@L139`
- **Detected tests (2):** `verifies_exact_stored_proof_and_binding@L226`, `revocation_and_wrong_proof_are_rejected@L241`
- **Error/guard literals:** `CAPABILITY_APPROVAL_INVALID`, `APPROVAL_STORE_UNAVAILABLE`
- **Rust crate references:** `crm_capability_runtime`, `crm_module_sdk`

### [`src/authorization.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-capability-adapters/src/authorization.rs) · 570 lines

- **Public declarations (4):** `upsert@L97`, `revoke@L111`, `revision@L132`, `new@L193`
- **Internal/trait/helpers (19):** `key@L40`, `validate@L51`, `fmt@L74`, `deployment_denies_capability@L140`, `deployment_denies_capability_from_value@L152`, `fmt@L183`, `authorize_binding@L197`, `authorize@L265`, `authorize@L281`, `deployment_ceiling_parses_exact_capability_ids@L308`, `revocation_is_visible_to_the_next_live_decision@L334`, `query_revocation_uses_the_same_live_authorization_store@L360`, `exact_version_grants_coexist_for_the_same_policy@L395`, `exact_capability_binding_is_enforced@L424`, `query_definition@L437`, `query_request@L469`, `grant@L498`, `definition@L511`, `request@L535`
- **Public types:** `AuthorizationGrant@L28`, `AuthorizationStoreError@L67`, `LiveAuthorizationStore@L92`, `LiveCapabilityAuthorizer@L177`
- **Detected tests (5):** `deployment_ceiling_parses_exact_capability_ids@L308`, `revocation_is_visible_to_the_next_live_decision@L334`, `query_revocation_uses_the_same_live_authorization_store@L360`, `exact_version_grants_coexist_for_the_same_policy@L395`, `exact_capability_binding_is_enforced@L424`
- **Error/guard literals:** `CRM_AUTHORIZATION_DENIED_CAPABILITIES`, `AUTHORIZATION_STORE_UNAVAILABLE`
- **Rust crate references:** `crm_capability_runtime`, `crm_module_sdk`, `crm_query_runtime`

### [`src/client.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-capability-adapters/src/client.rs) · 396 lines

- **Public declarations (2):** `new@L26`, `semantic_input_hash@L97`
- **Internal/trait/helpers (12):** `fmt@L32`, `invoke@L41`, `hash_field@L112`, `data_class_tag@L117`, `encoding_tag@L131`, `execute@L166`, `rebinds_module_and_capability_while_preserving_delivery_lineage@L178`, `maps_live_target_authorization_denial_to_sdk_error@L251`, `gateway@L286`, `target_definition@L338`, `target_input@L362`, `caller_context@L376`
- **Public types:** `GatewayCapabilityClient@L21`
- **Detected tests (2):** `rebinds_module_and_capability_while_preserving_delivery_lineage@L178`, `maps_live_target_authorization_denial_to_sdk_error@L251`
- **Error/guard literals:** `CAPABILITY_PERMISSION_DENIED`
- **Rust crate references:** `crm_capability_runtime`, `crm_module_sdk`

### [`src/contract_usage_telemetry.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-capability-adapters/src/contract_usage_telemetry.rs) · 766 lines

- **Public declarations (1):** `meter_contract_registries@L432`
- **Internal/trait/helpers (28):** `as_str@L17`, `from_definition@L32`, `from_catalog@L39`, `display@L46`, `from_catalog@L60`, `from_delivery@L69`, `display@L78`, `from@L98`, `from@L129`, `new@L179`, `record_resolved@L269`, `record_event_delivery@L282`, `render_prometheus@L289`, `render_capability_series@L310`, `render_event_series@L332`, `increment@L355`, `add_published_definitions@L361`, `configuration_error@L382`, `fmt@L400`, `resolve@L410`, `resolve@L509`, `exact_capability_resolution_and_event_delivery_increment@L525`, `wrong_event_provider_consumer_or_version_does_not_increment@L570`, `unknown_resolution_does_not_increment@L594`, `catalogs_fail_closed_against_live_capabilities_and_duplicates@L625`, `prometheus_output_is_deterministic_zero_seeded_and_pii_free@L667`, `definition@L709`, `delivery@L734`
- **Detected tests (5):** `exact_capability_resolution_and_event_delivery_increment@L525`, `wrong_event_provider_consumer_or_version_does_not_increment@L570`, `unknown_resolution_does_not_increment@L594`, `catalogs_fail_closed_against_live_capabilities_and_duplicates@L625`, `prometheus_output_is_deterministic_zero_seeded_and_pii_free@L667`
- **Error/guard literals:** `CONTRACT_USAGE_TELEMETRY_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_module_sdk`

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-capability-adapters/src/lib.rs) · 24 lines

- **Re-exports:** `pub use approval::*;`, `pub use authorization::*;`, `pub use client::*;`, `pub use contract_usage_telemetry::meter_contract_registries;`, `pub use query_visibility::*;`, `pub use rate_limit::*;`, `pub use registry::*;`, `pub use signed_approval::*;`

### [`src/query_visibility.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-capability-adapters/src/query_visibility.rs) · 465 lines

- **Public declarations (3):** `upsert@L201`, `revoke@L224`, `new@L263`
- **Internal/trait/helpers (17):** `from_environment@L39`, `parse@L43`, `apply@L77`, `invalid_field_ceiling@L94`, `key@L115`, `validate@L127`, `fmt@L158`, `default@L184`, `with_field_ceiling@L237`, `fmt@L253`, `authorize_visibility@L269`, `exact_resource_grant_overrides_type_wide_fields_and_revoke_is_live@L342`, `deployment_ceiling_can_hide_fields_without_hiding_the_resource@L374`, `hidden_field_configuration_is_explicit_and_strict@L401`, `grant@L414`, `resource@L429`, `request@L436`
- **Public types:** `QueryVisibilityGrant@L101`, `QueryVisibilityStoreError@L151`, `LiveQueryVisibilityStore@L177`, `LiveQueryVisibilityAuthorizer@L247`
- **Detected tests (3):** `exact_resource_grant_overrides_type_wide_fields_and_revoke_is_live@L342`, `deployment_ceiling_can_hide_fields_without_hiding_the_resource@L374`, `hidden_field_configuration_is_explicit_and_strict@L401`
- **Error/guard literals:** `CRM_QUERY_HIDDEN_FIELDS`, `QUERY_VISIBILITY_UNAVAILABLE`
- **Rust crate references:** `crm_module_sdk`, `crm_query_runtime`

### [`src/rate_limit.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-capability-adapters/src/rate_limit.rs) · 334 lines

- **Public declarations (2):** `upsert@L57`, `new@L102`
- **Internal/trait/helpers (9):** `validate@L20`, `fmt@L35`, `fmt@L92`, `check@L112`, `unavailable_error@L200`, `enforces_limit_and_resets_at_next_window@L221`, `counters_are_tenant_scoped@L246`, `definition@L275`, `request@L299`
- **Public types:** `FixedWindowPolicy@L13`, `RateLimitStoreError@L29`, `RateLimitPolicyStore@L52`, `FixedWindowRateLimiter@L85`
- **Detected tests (2):** `enforces_limit_and_resets_at_next_window@L221`, `counters_are_tenant_scoped@L246`
- **Error/guard literals:** `RATE_LIMIT_POLICY_BINDING_MISSING`, `RATE_LIMIT_POLICY_NOT_FOUND`, `RATE_LIMIT_CLOCK_INVALID`, `RATE_LIMIT_STORE_UNAVAILABLE`
- **Rust crate references:** `crm_capability_runtime`, `crm_module_sdk`

### [`src/registry.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-capability-adapters/src/registry.rs) · 218 lines

- **Public declarations (5):** `new@L53`, `definition@L78`, `len@L87`, `is_empty@L91`, `catalog_dependency_error@L148`
- **Internal/trait/helpers (6):** `fmt@L23`, `resolve@L97`, `validate_definition@L106`, `resolves_only_exact_version@L165`, `rejects_duplicate_coordinate@L189`, `definition@L194`
- **Public types:** `CapabilityCatalogError@L9`, `CapabilityCatalog@L48`
- **Detected tests (3):** `resolves_only_exact_version@L165`, `rejects_duplicate_coordinate@L189`, `definition@L194`
- **Error/guard literals:** `CAPABILITY_CATALOG_UNAVAILABLE`
- **Rust crate references:** `crm_capability_runtime`, `crm_module_sdk`

### [`src/signed_approval.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-capability-adapters/src/signed_approval.rs) · 294 lines

- **Public declarations (2):** `try_new@L34`, `sign@L48`
- **Internal/trait/helpers (13):** `fmt@L25`, `verify@L54`, `canonical_approval_bytes@L82`, `append_field@L95`, `hmac_sha256@L100`, `constant_time_equal@L127`, `configuration_error@L139`, `invalid_approval@L149`, `verifies_exact_signed_approval_and_rejects_tampering@L171`, `rejects_short_signing_key@L207`, `approval@L216`, `definition@L232`, `request@L257`
- **Public types:** `HmacSha256ApprovalVerifier@L20`
- **Detected tests (2):** `verifies_exact_signed_approval_and_rejects_tampering@L171`, `rejects_short_signing_key@L207`
- **Error/guard literals:** `APPROVAL_SIGNING_CONFIGURATION_INVALID`, `CAPABILITY_APPROVAL_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_module_sdk`

## 05. `crm-capability-ingress` — RETIRE-REVIEW / D? + B? invariants

### [`src/authentication.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-capability-ingress/src/authentication.rs) · 318 lines

- **Public declarations (6):** `permits_tenant@L18`, `issue@L82`, `revoke@L108`, `code@L137`, `retryable@L147`, `new@L190`
- **Internal/trait/helpers (11):** `validate@L32`, `fmt@L52`, `fmt@L153`, `authenticate@L167`, `fmt@L180`, `authenticate@L196`, `parse_bearer@L227`, `validate_token@L239`, `token_digest@L246`, `authenticates_without_storing_plaintext_and_honors_revocation@L258`, `rejects_expired_token@L293`
- **Public types:** `AuthenticatedPrincipal@L11`, `AccessTokenGrant@L24`, `AuthenticationStoreError@L44`, `AccessTokenStore@L77`, `AuthenticationError@L128`, `RequestAuthenticator@L166`, `BearerTokenAuthenticator@L174`
- **Detected tests (2):** `authenticates_without_storing_plaintext_and_honors_revocation@L258`, `rejects_expired_token@L293`
- **Error/guard literals:** `AUTHENTICATION_REQUIRED`, `AUTHENTICATION_INVALID`, `AUTHENTICATION_EXPIRED`, `AUTHENTICATION_REVOKED`, `AUTHENTICATION_UNAVAILABLE`
- **Rust crate references:** `crm_module_sdk`

### [`src/context.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-capability-ingress/src/context.rs) · 495 lines

- **Public declarations (6):** `validate@L51`, `code@L90`, `retryable@L106`, `new@L152`, `resolve@L164`, `semantic_input_hash@L292`
- **Internal/trait/helpers (15):** `fmt@L115`, `fmt@L141`, `resolve_timeout@L250`, `generate_identifier@L275`, `hash_field@L307`, `data_class_tag@L312`, `encoding_tag@L326`, `parse_id@L335`, `optional_or_default@L343`, `optional_or_generated@L352`, `resolves_tenant_actor_trace_and_timeout_into_complete_context@L375`, `rejects_cross_tenant_request_and_excessive_timeout@L423`, `semantic_hash_changes_when_contract_metadata_changes@L462`, `route@L472`, `payload@L481`
- **Public types:** `CapabilityRoute@L17`, `IngressMetadata@L25`, `CapabilityCallEnvelope@L37`, `TimeoutPolicy@L45`, `TimeoutBudget@L63`, `ResolvedCapabilityCall@L69`, `ContextResolutionError@L76`, `ExecutionContextResolver@L134`
- **Detected tests (3):** `resolves_tenant_actor_trace_and_timeout_into_complete_context@L375`, `rejects_cross_tenant_request_and_excessive_timeout@L423`, `semantic_hash_changes_when_contract_metadata_changes@L462`
- **Error/guard literals:** `TENANT_REQUIRED`, `TENANT_INVALID`, `TENANT_FORBIDDEN`, `IDEMPOTENCY_KEY_REQUIRED`, `EXECUTION_CONTEXT_INVALID`, `TIMEOUT_INVALID`, `TIMEOUT_EXCEEDS_LIMIT`, `EXECUTION_CONTEXT_UNAVAILABLE`, `EXECUTION_CONTEXT_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_module_sdk`

### [`src/core.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-capability-ingress/src/core.rs) · 429 lines

- **Public declarations (2):** `new@L155`, `execute@L167`
- **Internal/trait/helpers (14):** `fmt@L31`, `from@L53`, `from_sdk@L100`, `context_safe_message@L111`, `fmt@L144`, `resolve@L221`, `validate@L232`, `check@L243`, `verify@L260`, `authorize@L272`, `execute@L290`, `authentication_and_tenant_resolution_precede_gateway_execution@L307`, `definition@L377`, `envelope@L401`
- **Public types:** `ExecutionReceipt@L15`, `IngressError@L23`, `SafeTransportError@L44`, `CapabilityIngress@L137`
- **Detected tests (1):** `authentication_and_tenant_resolution_precede_gateway_execution@L307`
- **Error/guard literals:** `CAPABILITY_RATE_LIMITED`, `CAPABILITY_DEADLINE_EXCEEDED`
- **Rust crate references:** `crm_capability_runtime`, `crm_module_sdk`

### [`src/grpc.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-capability-ingress/src/grpc.rs) · 209 lines

- **Public declarations (2):** `new@L28`, `handle@L32`
- **Internal/trait/helpers (9):** `metadata_from_grpc@L73`, `metadata_value@L86`, `optional_u64_metadata@L99`, `invalid_metadata_error@L112`, `status_from_safe_error@L122`, `code_for_category@L144`, `insert_ascii_metadata@L157`, `maps_safe_categories_to_grpc_codes@L169`, `typed_error_code_is_returned_as_metadata@L189`
- **Public types:** `GrpcCapabilityMessage@L16`, `GrpcCapabilityMiddleware@L23`
- **Detected tests (2):** `maps_safe_categories_to_grpc_codes@L169`, `typed_error_code_is_returned_as_metadata@L189`
- **Error/guard literals:** `TRANSPORT_METADATA_INVALID`, `CAPABILITY_PERMISSION_DENIED`
- **Rust crate references:** `crm_capability_runtime`, `crm_module_sdk`

### [`src/http.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-capability-ingress/src/http.rs) · 216 lines

- **Public declarations (2):** `new@L46`, `handle@L50`
- **Internal/trait/helpers (9):** `metadata_from_headers@L90`, `header_value@L103`, `optional_u64_header@L116`, `invalid_header_error@L126`, `error_response@L136`, `status_for_category@L159`, `insert_response_header@L172`, `maps_safe_categories_to_http_statuses@L187`, `invalid_timeout_header_is_safe_bad_request@L207`
- **Public types:** `HttpCapabilityRequest@L20`, `HttpCapabilityBody@L28`, `HttpCapabilityResponse@L34`, `HttpCapabilityMiddleware@L41`
- **Detected tests (2):** `maps_safe_categories_to_http_statuses@L187`, `invalid_timeout_header_is_safe_bad_request@L207`
- **Error/guard literals:** `TRANSPORT_METADATA_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_module_sdk`

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-capability-ingress/src/lib.rs) · 29 lines

- **Re-exports:** `pub use authentication::*;`, `pub use context::*;`, `pub use core::*;`, `pub use grpc::*;`, `pub use http::*;`, `pub use query_context::*;`, `pub use query_core::*;`, `pub use query_grpc::*;`, `pub use query_http::*;`

### [`src/query_context.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-capability-ingress/src/query_context.rs) · 267 lines

- **Public declarations (2):** `new@L56`, `resolve@L68`
- **Internal/trait/helpers (7):** `fmt@L45`, `resolve_timeout@L131`, `generate_identifier@L156`, `parse_id@L173`, `optional_or_default@L181`, `optional_or_generated@L190`, `resolves_query_without_mutation_only_identity_fields@L214`
- **Public types:** `QueryIngressMetadata@L15`, `QueryCallEnvelope@L24`, `ResolvedQueryCall@L31`, `QueryContextResolver@L38`
- **Detected tests (1):** `resolves_query_without_mutation_only_identity_fields@L214`
- **Rust crate references:** `crm_module_sdk`, `crm_query_runtime`

### [`src/query_core.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-capability-ingress/src/query_core.rs) · 174 lines

- **Public declarations (2):** `new@L132`, `execute@L144`
- **Internal/trait/helpers (4):** `fmt@L31`, `from@L44`, `query_context_safe_message@L90`, `fmt@L121`
- **Public types:** `QueryExecutionReceipt@L15`, `QueryIngressError@L23`, `QueryIngress@L114`
- **Error/guard literals:** `QUERY_DEADLINE_EXCEEDED`
- **Rust crate references:** `crm_module_sdk`, `crm_query_runtime`

### [`src/query_grpc.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-capability-ingress/src/query_grpc.rs) · 198 lines

- **Public declarations (2):** `new@L23`, `handle@L27`
- **Internal/trait/helpers (9):** `metadata_from_grpc@L67`, `metadata_value@L77`, `optional_u64_metadata@L90`, `invalid_metadata_error@L103`, `status_from_safe_error@L113`, `code_for_category@L135`, `insert_ascii_metadata@L148`, `query_metadata_does_not_depend_on_mutation_only_metadata@L161`, `query_safe_error_code_is_returned_as_metadata@L178`
- **Public types:** `GrpcQueryMessage@L12`, `GrpcQueryMiddleware@L18`
- **Detected tests (2):** `query_metadata_does_not_depend_on_mutation_only_metadata@L161`, `query_safe_error_code_is_returned_as_metadata@L178`
- **Error/guard literals:** `TRANSPORT_METADATA_INVALID`, `QUERY_PERMISSION_DENIED`
- **Rust crate references:** `crm_module_sdk`, `crm_query_runtime`

### [`src/query_http.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-capability-ingress/src/query_http.rs) · 193 lines

- **Public declarations (2):** `new@L37`, `handle@L41`
- **Internal/trait/helpers (8):** `metadata_from_headers@L80`, `header_value@L90`, `optional_u64_header@L103`, `invalid_header_error@L113`, `error_response@L123`, `status_for_category@L146`, `insert_response_header@L159`, `query_metadata_does_not_depend_on_mutation_only_headers@L175`
- **Public types:** `HttpQueryRequest@L12`, `HttpQueryBody@L19`, `HttpQueryResponse@L25`, `HttpQueryMiddleware@L32`
- **Detected tests (1):** `query_metadata_does_not_depend_on_mutation_only_headers@L175`
- **Error/guard literals:** `TRANSPORT_METADATA_INVALID`
- **Rust crate references:** `crm_module_sdk`, `crm_query_runtime`

### [`tests/postgres_gateway_e2e.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-capability-ingress/tests/postgres_gateway_e2e.rs) · 686 lines

- **Internal/trait/helpers (22):** `assert_incremented_once@L55`, `validate@L68`, `authorize@L94`, `execute_batch@L118`, `plan@L138`, `public_gateway_proves_no_bypass_replay_live_authorization_and_atomic_rollback@L198`, `compose@L384`, `token_store@L451`, `authorization_grant@L467`, `capability_definition@L480`, `input_payload@L504`, `http_request@L518`, `insert_header@L564`, `success_result@L571`, `transport_error@L578`, `error_code@L585`, `clear_calls@L589`, `assert_call_order@L593`, `bind_context@L600`, `evidence_counts@L623`, `count_prefix@L661`, `audit_head@L669`
- **Detected tests (1):** `public_gateway_proves_no_bypass_replay_live_authorization_and_atomic_rollback@L198`
- **Error/guard literals:** `CAPABILITY_INPUT_HASH_INVALID`, `DATABASE_URL`, `AUTHENTICATION_INVALID`, `TENANT_FORBIDDEN`, `CAPABILITY_IDEMPOTENCY_KEY_REUSED`, `CAPABILITY_PERMISSION_DENIED`, `CAPABILITY_STORAGE_UNAVAILABLE`
- **Rust crate references:** `crm_capability_adapters`, `crm_capability_ingress`, `crm_capability_runtime`, `crm_core_data`, `crm_module_sdk`

### [`tests/postgres_production_evidence_rollback.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-capability-ingress/tests/postgres_production_evidence_rollback.rs) · 378 lines

- **Internal/trait/helpers (14):** `target@L55`, `plan@L63`, `validate@L83`, `omitted_production_evidence_rolls_back_every_transactional_side_effect@L112`, `compose@L164`, `token_store@L215`, `definition@L231`, `payload@L239`, `sales_create@L253`, `http_request@L276`, `insert_header@L321`, `evidence_counts@L328`, `scalar_count@L358`, `record_count@L368`
- **Detected tests (1):** `omitted_production_evidence_rolls_back_every_transactional_side_effect@L112`
- **Error/guard literals:** `CAPABILITY_INPUT_HASH_INVALID`, `DATABASE_URL`, `ADMIN_DATABASE_URL`, `CAPABILITY_EXECUTION_PLAN_INVALID`
- **Rust crate references:** `crm_capability_adapters`, `crm_capability_ingress`, `crm_capability_runtime`, `crm_core_data`, `crm_module_sdk`, `crm_proto_contracts`, `crm_sales_activities_capability_composition`

### [`tests/postgres_production_mutations.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-capability-ingress/tests/postgres_production_mutations.rs) · 955 lines

- **Internal/trait/helpers (30):** `validate@L66`, `authorize@L92`, `execute@L115`, `production_sales_and_activities_mutations_use_the_authenticated_public_postgres_path@L138`, `compose@L517`, `token_store@L581`, `definition@L597`, `payload@L605`, `route@L621`, `http_request@L630`, `grpc_request@L672`, `insert_header@L717`, `insert_metadata@L724`, `http_success@L731`, `http_error_code@L738`, `grpc_error_code@L745`, `clear_calls@L754`, `assert_calls@L758`, `actor_owner@L765`, `sales_create@L771`, `sales_update@L796`, `sales_advance@L814`, `task_create@L832`, `task_update@L852`, `schedule_reminder@L872`, `complete_task@L882`, `evidence_counts@L889`, `scalar_count@L919`, `assert_mutation_delta@L929`, `assert_noop_delta@L948`
- **Detected tests (1):** `production_sales_and_activities_mutations_use_the_authenticated_public_postgres_path@L138`
- **Error/guard literals:** `CAPABILITY_INPUT_HASH_INVALID`, `DATABASE_URL`, `ADMIN_DATABASE_URL`, `AUTHENTICATION_INVALID`, `TENANT_FORBIDDEN`, `CAPABILITY_IDEMPOTENCY_KEY_REUSED`, `CAPABILITY_PERMISSION_DENIED`
- **Rust crate references:** `crm_capability_adapters`, `crm_capability_ingress`, `crm_capability_runtime`, `crm_core_data`, `crm_module_sdk`, `crm_proto_contracts`, `crm_sales_activities_capability_composition`

### [`tests/postgres_production_queries.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-capability-ingress/tests/postgres_production_queries.rs) · 561 lines

- **Internal/trait/helpers (13):** `validate@L56`, `authenticated_http_and_grpc_queries_need_no_mutation_identity_and_are_side_effect_free@L75`, `authorization_grant@L413`, `mutation_definition@L426`, `query_definition@L434`, `mutation_request@L442`, `query_payload@L480`, `route@L496`, `actor_owner@L505`, `sales_fields@L511`, `task_fields@L528`, `evidence_counts@L545`, `scalar_count@L555`
- **Detected tests (1):** `authenticated_http_and_grpc_queries_need_no_mutation_identity_and_are_side_effect_free@L75`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`, `QUERY_PERMISSION_DENIED`
- **Rust crate references:** `crm_capability_adapters`, `crm_capability_ingress`, `crm_capability_runtime`, `crm_core_data`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`, `crm_sales_activities_capability_composition`, `crm_sales_activities_query_adapter`

## 06. `crm-capability-plan-support` — RETIRE-REVIEW / D? + B? invariants

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-capability-plan-support/src/lib.rs) · 532 lines

- **Public declarations (24):** `protobuf_contract@L33`, `protobuf_payload@L49`, `persisted_json_payload@L81`, `persisted_json_payload_with_data_class@L88`, `persisted_json_bytes@L113`, `persisted_json_bytes_with_data_class@L120`, `decode_request@L141`, `decode_request_with_data_class@L149`, `capability_idempotency@L182`, `record_ref@L207`, `event_evidence@L225`, `event_evidence_with_data_class@L242`, `audit_intent@L266`, `wire_resource_to_domain@L326`, `domain_resource_to_wire@L356`, `wire_money_to_domain@L365`, `domain_money_to_wire@L388`, `wire_date_to_domain@L395`, `domain_date_to_wire@L407`, `wire_time_to_nanos@L415`, `nanos_to_wire_time@L425`, `input_identifier@L429`, `configured_identifier@L436`, `stored_data_error@L448`
- **Internal/trait/helpers (6):** `stable_evidence_id@L457`, `sha256_hex@L485`, `append_hash_field@L489`, `hex@L494`, `message_descriptor_hash_is_message_specific_and_nonzero@L509`, `cross_tenant_reference_is_rejected@L517`
- **Public types:** `PersistedPayloadContract@L24`, `EventSpec@L218`
- **Re-exports:** `pub use crm_proto_contracts::message_descriptor_hash;`
- **Detected tests (2):** `message_descriptor_hash_is_message_specific_and_nonzero@L509`, `cross_tenant_reference_is_rejected@L517`
- **Error/guard literals:** `PROTOBUF_PAYLOAD_TOO_LARGE`, `PERSISTED_AGGREGATE_TOO_LARGE`, `PERSISTED_AGGREGATE_CONTRACT_INVALID`, `CAPABILITY_INPUT_CONTRACT_MISMATCH`, `CAPABILITY_INPUT_PROTOBUF_INVALID`, `CAPABILITY_IDEMPOTENCY_EXPIRY_INVALID`, `AUDIT_ENVELOPE_SERIALIZATION_FAILED`, `CAPABILITY_ADAPTER_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_contracts`, `crm_core_data`, `crm_module_sdk`, `crm_proto_contracts`

## 07. `crm-capability-runtime` — RETIRE-REVIEW / D? + B? invariants

### [`src/gateway.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-capability-runtime/src/gateway.rs) · 343 lines

- **Public declarations (3):** `new@L39`, `execute@L59`, `gateway_error_to_sdk@L287`
- **Internal/trait/helpers (6):** `fmt@L23`, `validate_definition_binding@L141`, `validate_input_contract@L177`, `validate_output_contract@L187`, `validate_approval_binding@L205`, `fmt@L256`
- **Public types:** `CapabilityGateway@L12`, `GatewayError@L226`
- **Error/guard literals:** `CAPABILITY_NOT_FOUND`, `CAPABILITY_RATE_LIMITED`, `CAPABILITY_APPROVAL_REQUIRED`, `CAPABILITY_PERMISSION_DENIED`, `CAPABILITY_INVALID`
- **Rust crate references:** `crm_module_sdk`

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-capability-runtime/src/lib.rs) · 18 lines

- **Re-exports:** `pub use gateway::*;`, `pub use ports::*;`, `pub use types::*;`

### [`src/ports.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-capability-runtime/src/ports.rs) · 55 lines

- **Internal/trait/helpers (6):** `resolve@L8`, `validate@L16`, `check@L24`, `verify@L32`, `authorize@L41`, `execute@L49`
- **Public types:** `CapabilityRegistryPort@L7`, `CapabilitySemanticValidator@L15`, `CapabilityRateLimiter@L23`, `CapabilityApprovalVerifier@L31`, `CapabilityAuthorizer@L40`, `TransactionalCapabilityExecutor@L48`
- **Rust crate references:** `crm_module_sdk`

### [`src/testing.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-capability-runtime/src/testing.rs) · 153 lines

- **Public declarations (1):** `call_log@L14`
- **Internal/trait/helpers (8):** `record@L18`, `resolve@L30`, `validate@L51`, `check@L70`, `verify@L91`, `authorize@L111`, `execute@L132`, `now_unix_nanos@L149`
- **Public types:** `CallLog@L12`, `StaticCapabilityRegistry@L23`, `FixedSemanticValidator@L45`, `FixedRateLimiter@L63`, `FixedApprovalVerifier@L85`, `FixedAuthorizer@L104`, `RecordingExecutor@L126`, `FixedClock@L144`
- **Rust crate references:** `crm_module_sdk`

### [`src/types.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-capability-runtime/src/types.rs) · 103 lines

- **Public declarations (1):** `matches@L29`
- **Public types:** `CapabilityRisk@L9`, `PayloadContract@L18`, `CapabilityDefinition@L43`, `ApprovalEvidence@L59`, `CapabilityRequest@L72`, `CapabilityExecutionResult@L81`, `RateLimitDecision@L89`, `AuthorizationDecision@L97`
- **Rust crate references:** `crm_module_sdk`

### [`tests/gateway.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-capability-runtime/tests/gateway.rs) · 248 lines

- **Internal/trait/helpers (14):** `block_on@L19`, `payload_contract@L31`, `definition@L43`, `payload@L59`, `request@L73`, `outcome@L107`, `gateway@L115`, `calls@L169`, `live_authorization_is_the_final_stage_before_execution@L174`, `unknown_capability_never_reaches_validation_or_execution@L194`, `input_contract_mismatch_stops_before_semantic_validation@L203`, `rate_limit_stops_before_approval_authorization_and_execution@L215`, `approval_binding_is_checked_before_proof_verification@L225`, `authorization_denial_never_calls_the_transactional_executor@L237`
- **Detected tests (6):** `live_authorization_is_the_final_stage_before_execution@L174`, `unknown_capability_never_reaches_validation_or_execution@L194`, `input_contract_mismatch_stops_before_semantic_validation@L203`, `rate_limit_stops_before_approval_authorization_and_execution@L215`, `approval_binding_is_checked_before_proof_verification@L225`, `authorization_denial_never_calls_the_transactional_executor@L237`
- **Rust crate references:** `crm_capability_runtime`, `crm_module_sdk`

## 08. `crm-consents-capability-adapter` — ACTIVE-GUARD / A? + B-ON-DEMAND + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-consents-capability-adapter/src/lib.rs) · 135 lines

- **Public declarations (2):** `capability_definitions@L37`, `capability_definition@L44`
- **Internal/trait/helpers (4):** `configured@L87`, `configuration_error@L97`, `publishes_exact_create_and_withdraw_coordinates_as_personal_mutations@L106`, `rejects_unknown_consent_mutation_coordinate@L130`
- **Re-exports:** `pub use planner::*;`
- **Detected tests (2):** `publishes_exact_create_and_withdraw_coordinates_as_personal_mutations@L106`, `rejects_unknown_consent_mutation_coordinate@L130`
- **Error/guard literals:** `CONSENTS_CAPABILITY_UNSUPPORTED`, `CONSENTS_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_module_sdk`

### [`src/owner_planner.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-consents-capability-adapter/src/owner_planner.rs) · 531 lines

- **Public declarations (5):** `referenced_scope_from_create@L277`, `consent_authorization_to_wire@L293`, `persisted_contract@L354`, `persisted_payload@L365`, `consent_authorization_from_snapshot@L375`
- **Internal/trait/helpers (17):** `target@L36`, `plan@L88`, `plan_create@L103`, `plan_withdraw@L182`, `mutation_plan@L249`, `authorization_id_from_ref@L394`, `party_reference_from_ref@L403`, `optional_contact_point_reference_from_ref@L411`, `required_time@L419`, `optional_time@L425`, `channel_from_wire@L429`, `effect_from_wire@L444`, `ensure_definition@L455`, `invalid_plan@L470`, `unsupported_capability@L479`, `public_wire_projection_preserves_immutable_scope_and_lifecycle@L493`, `persisted_contract_is_exact_and_nonzero@L524`
- **Public types:** `ConsentCapabilityPlanner@L26`, `CreateConsentReferenceScope@L29`
- **Detected tests (2):** `public_wire_projection_preserves_immutable_scope_and_lifecycle@L493`, `persisted_contract_is_exact_and_nonzero@L524`
- **Error/guard literals:** `CONSENTS_PERSISTED_AUTHORIZATION_IDENTITY_INVALID`, `CONSENTS_MUTATION_PLAN_INVALID`, `CONSENTS_CAPABILITY_UNSUPPORTED`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_consents`, `crm_core_data`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/planner.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-consents-capability-adapter/src/planner.rs) · 184 lines

- **Internal/trait/helpers (10):** `target@L47`, `plan@L55`, `ensure_supported_contact_point_scope@L88`, `party_link_payload@L102`, `configured_relationship_type@L117`, `configured_record_type@L121`, `config_error@L125`, `party_link_contract_is_personal_json_and_stably_hashed@L141`, `authoritative_party_access_path_coordinates_are_stable@L152`, `scoped_push_fails_closed_until_a_push_endpoint_kind_exists@L166`
- **Public types:** `ConsentCapabilityPlanner@L44`
- **Re-exports:** `pub use owner_planner::{`
- **Detected tests (3):** `party_link_contract_is_personal_json_and_stably_hashed@L141`, `authoritative_party_access_path_coordinates_are_stable@L152`, `scoped_push_fails_closed_until_a_push_endpoint_kind_exists@L166`
- **Error/guard literals:** `CONSENTS_REFERENCE_UNAVAILABLE`, `CONSENTS_RELATIONSHIP_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_consents`, `crm_core_data`, `crm_module_sdk`

## 09. `crm-consents-capability-composition` — ACTIVE-GUARD / A? + B-ON-DEMAND + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-consents-capability-composition/src/lib.rs) · 561 lines

- **Public declarations (8):** `new@L71`, `new@L146`, `new@L190`, `validate_reference_scope@L208`, `channel_is_compatible@L234`, `mutation_capability_definitions@L256`, `query_capability_definitions@L262`, `build_contribution@L268`
- **Internal/trait/helpers (26):** `party_exists@L52`, `contact_point@L58`, `party_exists@L77`, `contact_point@L99`, `fmt@L137`, `validate@L152`, `fmt@L181`, `execute@L196`, `consents_cursor@L321`, `production_composition_error@L333`, `reference_unavailable@L343`, `unsupported_capability@L352`, `config_error@L361`, `available@L385`, `party_exists@L395`, `contact_point@L409`, `scope@L424`, `tenant@L438`, `facts@L442`, `fake_dependency_error@L449`, `party_wide_scope_requires_only_same_tenant_party_existence@L460`, `missing_party_and_missing_contact_point_share_the_same_safe_failure@L473`, `ownership_mismatch_and_channel_mismatch_fail_closed@L501`, `deterministic_channel_mapping_accepts_sms_on_phone_and_messaging_on_messaging@L519`, `dependency_failures_are_propagated_not_masked_as_missing_references@L532`, `push_has_no_false_contact_point_kind_mapping_until_a_push_endpoint_kind_exists@L549`
- **Public types:** `ConsentContactPointFacts@L46`, `ConsentReferenceReader@L51`, `PostgresConsentReferenceReader@L66`, `ConsentCapabilitySemanticValidator@L132`, `ConsentCapabilityExecutor@L176`, `ConsentsProductionDependencies@L247`
- **Detected tests (6):** `party_wide_scope_requires_only_same_tenant_party_existence@L460`, `missing_party_and_missing_contact_point_share_the_same_safe_failure@L473`, `ownership_mismatch_and_channel_mismatch_fail_closed@L501`, `deterministic_channel_mapping_accepts_sms_on_phone_and_messaging_on_messaging@L519`, `dependency_failures_are_propagated_not_masked_as_missing_references@L532`, `push_has_no_false_contact_point_kind_mapping_until_a_push_endpoint_kind_exists@L549`
- **Error/guard literals:** `CONSENTS_CURSOR_CONFIGURATION_INVALID`, `CONSENTS_PRODUCTION_COMPOSITION_INVALID`, `CONSENTS_REFERENCE_UNAVAILABLE`, `CONSENTS_COMPOSITION_CAPABILITY_UNSUPPORTED`, `CONSENTS_REFERENCE_CONFIGURATION_INVALID`, `FAKE_REFERENCE_DEPENDENCY_FAILURE`
- **Rust crate references:** `crm_application_composition`, `crm_capability_runtime`, `crm_consents`, `crm_consents_capability_adapter`, `crm_consents_query_adapter`, `crm_contact_points`, `crm_contact_points_capability_adapter`, `crm_core_data`, `crm_module_sdk`, `crm_parties_capability_adapter`, `crm_query_runtime`

## 10. `crm-consents-privacy-scope-adapter` — END / B-END + D? adapter

### [`src/contract.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-consents-privacy-scope-adapter/src/contract.rs) · 94 lines

- **Public declarations (8):** `consents_privacy_scope_definition@L24`, `validate_definition@L44`, `module_id@L55`, `capability_id@L59`, `capability_version@L63`, `schema_id@L67`, `schema_version@L71`, `output_descriptor_hash@L75`
- **Internal/trait/helpers (1):** `payload_contract@L79`
- **Error/guard literals:** `CONSENTS_PRIVACY_SCOPE_DEFINITION_MISMATCH`
- **Rust crate references:** `crm_capability_runtime`, `crm_consents`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/digest.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-consents-privacy-scope-adapter/src/digest.rs) · 44 lines

- **Public declarations (2):** `encode_hex@L3`, `decode_hex@L13`
- **Internal/trait/helpers (2):** `nibble@L30`, `cursor_invalid@L38`
- **Error/guard literals:** `CONSENTS_PRIVACY_SCOPE_CURSOR_INVALID`
- **Rust crate references:** `crm_module_sdk`

### [`src/errors.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-consents-privacy-scope-adapter/src/errors.rs) · 64 lines

- **Public declarations (7):** `configured@L3`, `lineage_invalid@L15`, `stored_state_invalid@L29`, `row_decode_error@L39`, `database_unavailable@L43`, `invalid_contract@L53`, `invalid_contract_with_reference@L57`
- **Error/guard literals:** `CONSENTS_PRIVACY_SCOPE_CONFIGURATION_INVALID`, `CONSENTS_PRIVACY_SCOPE_LINEAGE_INVALID`, `CONSENTS_PRIVACY_SCOPE_STORED_STATE_INVALID`, `CONSENTS_PRIVACY_SCOPE_UNAVAILABLE`
- **Rust crate references:** `crm_module_sdk`

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-consents-privacy-scope-adapter/src/lib.rs) · 32 lines

- **Re-exports:** `pub use contract::{`, `pub use owner_action::{`, `pub use postgres::ConsentsPrivacyScopeQueryAdapter;`

### [`src/owner_action.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-consents-privacy-scope-adapter/src/owner_action.rs) · 90 lines

- **Public declarations (2):** `consents_privacy_action_definition@L17`, `consents_privacy_action_planner@L21`
- **Internal/trait/helpers (8):** `owner_module_id@L26`, `capability_id@L30`, `supports_resource_type@L34`, `anonymize@L38`, `deletion_tombstone@L46`, `reject_immutable_action@L55`, `publishes_the_frozen_owner_action_coordinate@L72`, `supports_only_the_authoritative_consent_record_type@L84`
- **Public types:** `ConsentsPrivacyActionPlanner@L12`, `ConsentsPrivacyActionPolicy@L15`
- **Detected tests (2):** `publishes_the_frozen_owner_action_coordinate@L72`, `supports_only_the_authoritative_consent_record_type@L84`
- **Rust crate references:** `crm_capability_runtime`, `crm_consents_capability_adapter`, `crm_customer_privacy`, `crm_customer_privacy_owner_scope_support`, `crm_module_sdk`, `crm_consents`

### [`src/postgres.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-consents-privacy-scope-adapter/src/postgres.rs) · 274 lines

- **Public declarations (1):** `new@L44`
- **Internal/trait/helpers (7):** `fmt@L35`, `execute_query@L48`, `execute@L83`, `read_consent_page@L98`, `map_canonical_claim_error@L167`, `strict_consent_resource@L193`, `configured@L271`
- **Public types:** `ConsentsPrivacyScopeQueryAdapter@L30`
- **Rust crate references:** `crm_capability_runtime`, `crm_consents`, `crm_consents_capability_adapter`, `crm_core_data`, `crm_customer_privacy_owner_scope_support`, `crm_module_sdk`, `crm_query_runtime`

### [`src/request.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-consents-privacy-scope-adapter/src/request.rs) · 250 lines

- **Public declarations (3):** `validate_request_contract@L27`, `validate_wire_request@L32`, `encode_cursor@L87`
- **Internal/trait/helpers (5):** `decode_cursor@L110`, `cursor_digest@L144`, `map_request_contract_error@L171`, `map_common_lineage_error@L190`, `cursor_invalid@L244`
- **Public types:** `ValidatedRequest@L18`
- **Error/guard literals:** `CONSENTS_PRIVACY_SCOPE_REQUEST_INVALID`, `CONSENTS_PRIVACY_SCOPE_LINEAGE_INVALID`, `CONSENTS_PRIVACY_SCOPE_REQUEST_BINDING_MISMATCH`, `CONSENTS_PRIVACY_SCOPE_INPUT_CONTRACT_MISMATCH`, `CONSENTS_PRIVACY_SCOPE_INPUT_HASH_MISMATCH`, `CONSENTS_PRIVACY_SCOPE_TENANT_MISMATCH`, `CONSENTS_PRIVACY_SCOPE_CASE_ID_INVALID`, `CONSENTS_PRIVACY_SCOPE_PARTY_INVALID`, `CONSENTS_PRIVACY_SCOPE_GENERATION_INVALID`, `CONSENTS_PRIVACY_SCOPE_REGISTRY_INVALID`, `CONSENTS_PRIVACY_SCOPE_REGISTRY_UNAVAILABLE`, `CONSENTS_PRIVACY_SCOPE_REGISTRY_MISMATCH`, `CONSENTS_PRIVACY_SCOPE_PURPOSE_INVALID`, `CONSENTS_PRIVACY_SCOPE_REQUEST_TIME_INVALID`, `CONSENTS_PRIVACY_SCOPE_PAGE_SIZE_INVALID`, `CONSENTS_PRIVACY_SCOPE_CURSOR_INVALID`
- **Rust crate references:** `crm_customer_privacy_owner_scope_support`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`

### [`src/response.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-consents-privacy-scope-adapter/src/response.rs) · 142 lines

- **Public declarations (2):** `build_response@L23`, `typed_output@L97`
- **Internal/trait/helpers (1):** `page_digest@L113`
- **Public types:** `VerifiedConsentResource@L18`
- **Rust crate references:** `crm_consents`, `crm_consents_capability_adapter`, `crm_customer_privacy_owner_scope_support`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/tests.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-consents-privacy-scope-adapter/src/tests.rs) · 325 lines

- **Internal/trait/helpers (11):** `context@L27`, `request@L41`, `query_request@L63`, `validated@L85`, `assert_invalid_argument@L93`, `publishes_exact_contract_only_coordinate@L100`, `request_contract_preserves_consents_specific_integrity_errors@L111`, `common_lineage_failures_keep_exact_consents_error_contracts@L143`, `cursor_round_trip_is_bound_to_lineage_and_page_size@L267`, `response_is_reference_only_deterministic_and_preserves_consent_evidence@L295`, `malformed_wire_request_is_rejected_before_owner_reads@L321`
- **Detected tests (6):** `publishes_exact_contract_only_coordinate@L100`, `request_contract_preserves_consents_specific_integrity_errors@L111`, `common_lineage_failures_keep_exact_consents_error_contracts@L143`, `cursor_round_trip_is_bound_to_lineage_and_page_size@L267`, `response_is_reference_only_deterministic_and_preserves_consent_evidence@L295`, `malformed_wire_request_is_rejected_before_owner_reads@L321`
- **Error/guard literals:** `PRIVACY_ERASURE_SCOPE`, `CONSENTS_PRIVACY_SCOPE_REQUEST_BINDING_MISMATCH`, `CONSENTS_PRIVACY_SCOPE_INPUT_HASH_MISMATCH`, `CONSENTS_PRIVACY_SCOPE_INPUT_CONTRACT_MISMATCH`, `CONSENTS_PRIVACY_SCOPE_TENANT_MISMATCH`, `CONSENTS_PRIVACY_SCOPE_CASE_ID_INVALID`, `CONSENTS_PRIVACY_SCOPE_PARTY_INVALID`, `CONSENTS_PRIVACY_SCOPE_GENERATION_INVALID`, `CONSENTS_PRIVACY_SCOPE_REGISTRY_INVALID`, `CONSENTS_PRIVACY_SCOPE_REGISTRY_MISMATCH`, `CONSENTS_PRIVACY_SCOPE_PURPOSE_INVALID`, `CONSENTS_PRIVACY_SCOPE_REQUEST_TIME_INVALID`, `CONSENTS_PRIVACY_SCOPE_PAGE_SIZE_INVALID`, `CONSENTS_PRIVACY_SCOPE_CURSOR_INVALID`, `CONSENTS_PRIVACY_SCOPE_REQUEST_INVALID`
- **Rust crate references:** `crm_customer_privacy`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`, `crm_consents`

### [`tests/postgres_scope.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-consents-privacy-scope-adapter/tests/postgres_scope.rs) · 259 lines

- **Internal/trait/helpers (1):** `consents_scope_is_paginated_tenant_bound_strict_and_side_effect_free@L26`
- **Detected tests (1):** `consents_scope_is_paginated_tenant_bound_strict_and_side_effect_free@L26`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`, `CONSENTS_PRIVACY_SCOPE_LINEAGE_INVALID`, `CONSENTS_PRIVACY_SCOPE_CURSOR_INVALID`, `CONSENTS_PRIVACY_SCOPE_STORED_STATE_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_consents_capability_adapter`, `crm_consents_privacy_scope_adapter`, `crm_core_data`, `crm_parties_capability_adapter`, `crm_proto_contracts`, `crm_query_runtime`, `crm_consents`

### [`tests/postgres_scope/support.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-consents-privacy-scope-adapter/tests/postgres_scope/support.rs) · 438 lines

- **Public declarations (6):** `insert_canonical_redirect@L42`, `corrupt_consent_metadata@L135`, `create_party@L183`, `create_consent@L211`, `scope_request@L328`, `write_surface_counts@L398`
- **Internal/trait/helpers (3):** `party_request@L253`, `consent_request@L272`, `capability_request@L292`
- **Public types:** `WriteSurfaceCounts@L387`
- **Error/guard literals:** `PRIVACY_ERASURE_SCOPE`
- **Rust crate references:** `crm_capability_runtime`, `crm_consents_capability_adapter`, `crm_consents_privacy_scope_adapter`, `crm_customer_privacy`, `crm_identity_resolution_capability_adapter`, `crm_module_sdk`, `crm_parties_capability_adapter`, `crm_proto_contracts`, `crm_query_runtime`, `crm_consents`, `crm_parties`

## 11. `crm-consents-query-adapter` — ACTIVE-GUARD / A? + B-ON-DEMAND + D? adapter

### [`src/base_adapter.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-consents-query-adapter/src/base_adapter.rs) · 1083 lines

- **Public declarations (3):** `new@L68`, `query_capability_definitions@L88`, `query_capability_definition@L95`
- **Internal/trait/helpers (44):** `fmt@L56`, `validate@L129`, `execute@L168`, `execute_get@L186`, `execute_list@L231`, `execute_authorize@L263`, `collect_authorizations@L313`, `has_more_visible_authorization@L375`, `load_authorization_candidates@L413`, `try_from@L460`, `matches@L483`, `hash@L526`, `validate_list@L564`, `evaluate_command@L585`, `static_scope_matches@L607`, `authorization_payload@L630`, `consent_authorization_to_wire_with_visibility@L663`, `required_channel@L709`, `optional_channel@L718`, `optional_effect@L734`, `optional_status@L746`, `channel_wire_value@L762`, `effect_wire_value@L773`, `status_wire_value@L780`, `cursor_binding@L787`, `decode_after@L806`, `encode_next@L834`, `decode_input@L855`, `validate_record_id@L886`, `consent_module_id@L891`, `consent_record_type@L895`, `enforce_visibility_scan_limit@L899`, `enforce_authorization_scan_limit@L907`, `is_authorization_data_unavailable@L920`, `resource_not_found@L927`, `unsupported_query@L936`, `cursor_error@L945`, `visibility_scan_limit_error@L955`, `config_error@L964`, `authorization@L983`, `publishes_get_list_and_authorize_as_personal_read_only_queries@L1003`, `list_filters_canonicalize_semantic_purpose_for_stable_cursor_binding@L1021`, `field_visibility_independently_redacts_legal_basis_and_evidence@L1043`, `data_unavailable_is_reserved_for_candidate_read_unavailability_not_corruption@L1066`
- **Public types:** `ConsentQueryAdapter@L48`
- **Detected tests (4):** `publishes_get_list_and_authorize_as_personal_read_only_queries@L1003`, `list_filters_canonicalize_semantic_purpose_for_stable_cursor_binding@L1021`, `field_visibility_independently_redacts_legal_basis_and_evidence@L1043`, `data_unavailable_is_reserved_for_candidate_read_unavailability_not_corruption@L1066`
- **Error/guard literals:** `CONSENTS_QUERY_CURSOR_INVALID`, `CONSENTS_QUERY_INPUT_CONTRACT_MISMATCH`, `CONSENTS_QUERY_INPUT_PROTOBUF_INVALID`, `CONSENTS_AUTHORIZATION_SCAN_LIMIT_EXCEEDED`, `DATA_QUERY_UNAVAILABLE`, `QUERY_RESOURCE_NOT_FOUND`, `CONSENTS_QUERY_CAPABILITY_UNSUPPORTED`, `CONSENTS_QUERY_VISIBILITY_SCAN_LIMIT_EXCEEDED`, `CONSENTS_QUERY_CONFIGURATION_INVALID`, `DATA_QUERY_STORED_VALUE_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_consents`, `crm_consents_capability_adapter`, `crm_core_data`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-consents-query-adapter/src/lib.rs) · 344 lines

- **Public declarations (1):** `new@L59`
- **Internal/trait/helpers (15):** `fmt@L49`, `execute_authorize@L72`, `load_authorization_candidates@L121`, `validate@L166`, `execute@L176`, `decode_authorize_input@L191`, `evaluate_command@L221`, `required_channel@L243`, `authorization_payload@L258`, `channel_wire_value@L291`, `configured_module_id@L302`, `configured_record_type@L306`, `configured_relationship_type@L310`, `config_error@L314`, `authoritative_access_path_coordinates_are_stable@L329`
- **Public types:** `ConsentQueryAdapter@L43`
- **Re-exports:** `pub use base_adapter::{`
- **Detected tests (1):** `authoritative_access_path_coordinates_are_stable@L329`
- **Error/guard literals:** `DATA_QUERY_UNAVAILABLE`, `CONSENTS_QUERY_INPUT_CONTRACT_MISMATCH`, `CONSENTS_QUERY_INPUT_PROTOBUF_INVALID`, `CONSENTS_QUERY_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_consents`, `crm_consents_capability_adapter`, `crm_core_data`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`

## 12. `crm-contact-points-capability-adapter` — NOW / A? + B-NOW-if-gap + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-contact-points-capability-adapter/src/lib.rs) · 144 lines

- **Public declarations (2):** `capability_definitions@L42`, `capability_definition@L49`
- **Internal/trait/helpers (4):** `configured@L85`, `configuration_error@L95`, `publishes_exact_create_update_and_verify_coordinates_as_personal_mutations@L104`, `rejects_unknown_contact_point_mutation_coordinate@L139`
- **Re-exports:** `pub use planner::*;`
- **Detected tests (2):** `publishes_exact_create_update_and_verify_coordinates_as_personal_mutations@L104`, `rejects_unknown_contact_point_mutation_coordinate@L139`
- **Error/guard literals:** `CONTACT_POINTS_CAPABILITY_UNSUPPORTED`, `CONTACT_POINTS_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_module_sdk`

### [`src/planner.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-contact-points-capability-adapter/src/planner.rs) · 570 lines

- **Public declarations (5):** `contact_point_to_wire@L349`, `persisted_contract@L403`, `persisted_payload@L414`, `contact_point_from_snapshot@L424`, `referenced_party_id_from_create@L440`
- **Internal/trait/helpers (15):** `target@L30`, `plan@L98`, `plan_create@L114`, `plan_update@L186`, `plan_verify@L255`, `mutation_plan@L320`, `contact_point_id_from_ref@L452`, `party_reference_from_ref@L461`, `contact_point_kind_from_wire@L470`, `contact_point_status_from_wire@L484`, `optional_time@L495`, `ensure_definition@L499`, `invalid_plan@L512`, `unsupported_capability@L521`, `wire_mapping_preserves_endpoint_verification_and_version_metadata@L535`
- **Public types:** `ContactPointCapabilityPlanner@L27`
- **Detected tests (1):** `wire_mapping_preserves_endpoint_verification_and_version_metadata@L535`
- **Error/guard literals:** `CONTACT_POINTS_PERSISTED_CONTACT_POINT_IDENTITY_INVALID`, `CONTACT_POINTS_CAPABILITY_PLAN_INVALID`, `CONTACT_POINTS_CAPABILITY_UNSUPPORTED`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_contact_points`, `crm_core_data`, `crm_module_sdk`, `crm_proto_contracts`

## 13. `crm-contact-points-capability-composition` — NOW / A? + B-NOW-if-gap + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-contact-points-capability-composition/src/lib.rs) · 204 lines

- **Public declarations (5):** `new@L47`, `contact_points_runtime_boundary@L91`, `mutation_capability_definitions@L101`, `query_capability_definitions@L107`, `build_contribution@L113`
- **Internal/trait/helpers (7):** `fmt@L38`, `validate@L53`, `contact_point_planner@L161`, `contact_points_cursor@L165`, `production_composition_error@L177`, `reference_unavailable@L187`, `unsupported_capability@L196`
- **Public types:** `ContactPointPartyReferenceSemanticValidator@L33`, `ContactPointsProductionDependencies@L81`
- **Error/guard literals:** `CONTACT_POINTS_CURSOR_CONFIGURATION_INVALID`, `CONTACT_POINTS_PRODUCTION_COMPOSITION_INVALID`, `CONTACT_POINTS_PARTY_REFERENCE_UNAVAILABLE`, `CONTACT_POINTS_COMPOSITION_CAPABILITY_UNSUPPORTED`
- **Rust crate references:** `crm_application_composition`, `crm_capability_runtime`, `crm_contact_points_capability_adapter`, `crm_contact_points_query_adapter`, `crm_core_data`, `crm_module_sdk`, `crm_party_reference_composition`, `crm_query_runtime`

### [`src/privacy.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-contact-points-capability-composition/src/privacy.rs) · 117 lines

- **Public declarations (1):** `new@L28`
- **Internal/trait/helpers (7):** `fmt@L34`, `check@L43`, `ensure_create_coordinate@L72`, `contact_party_reference_invalid@L87`, `canonical_party_unavailable@L97`, `guard_is_shareable_transaction_guard@L112`, `require_guard@L113`
- **Public types:** `ContactPointCreateCustomerSubjectGuard@L23`
- **Detected tests (2):** `guard_is_shareable_transaction_guard@L112`, `require_guard@L113`
- **Error/guard literals:** `CONTACT_POINTS_PRIVACY_GUARD_INVALID`, `CONTACT_POINTS_PARTY_REFERENCE_INVALID`, `CONTACT_POINTS_CANONICAL_PARTY_UNAVAILABLE`
- **Rust crate references:** `crm_capability_runtime`, `crm_contact_points_capability_adapter`, `crm_core_data`, `crm_identity_resolution_topology_composition`, `crm_module_sdk`

### [`src/root.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-contact-points-capability-composition/src/root.rs) · 9 lines

- **Re-exports:** `pub use legacy::*;`, `pub use privacy::*;`

## 14. `crm-contact-points-privacy-scope-adapter` — END / B-END + D? adapter

### [`src/contract.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-contact-points-privacy-scope-adapter/src/contract.rs) · 96 lines

- **Public declarations (8):** `contact_points_privacy_scope_definition@L26`, `validate_definition@L46`, `module_id@L57`, `capability_id@L61`, `capability_version@L65`, `schema_id@L69`, `schema_version@L73`, `output_descriptor_hash@L77`
- **Internal/trait/helpers (1):** `payload_contract@L81`
- **Error/guard literals:** `CONTACT_POINTS_PRIVACY_SCOPE_DEFINITION_MISMATCH`
- **Rust crate references:** `crm_capability_runtime`, `crm_contact_points`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/digest.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-contact-points-privacy-scope-adapter/src/digest.rs) · 44 lines

- **Public declarations (2):** `encode_hex@L3`, `decode_hex@L13`
- **Internal/trait/helpers (2):** `nibble@L30`, `cursor_invalid@L38`
- **Error/guard literals:** `CONTACT_POINTS_PRIVACY_SCOPE_CURSOR_INVALID`
- **Rust crate references:** `crm_module_sdk`

### [`src/errors.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-contact-points-privacy-scope-adapter/src/errors.rs) · 166 lines

- **Public declarations (10):** `configured@L6`, `map_request_contract_error@L18`, `map_common_lineage_error@L37`, `map_canonical_party_claim_error@L91`, `lineage_invalid@L117`, `stored_state_invalid@L131`, `row_decode_error@L141`, `database_unavailable@L145`, `invalid_contract@L155`, `invalid_contract_with_reference@L159`
- **Error/guard literals:** `CONTACT_POINTS_PRIVACY_SCOPE_CONFIGURATION_INVALID`, `CONTACT_POINTS_PRIVACY_SCOPE_REQUEST_BINDING_MISMATCH`, `CONTACT_POINTS_PRIVACY_SCOPE_INPUT_CONTRACT_MISMATCH`, `CONTACT_POINTS_PRIVACY_SCOPE_INPUT_HASH_MISMATCH`, `CONTACT_POINTS_PRIVACY_SCOPE_TENANT_MISMATCH`, `CONTACT_POINTS_PRIVACY_SCOPE_CASE_ID_INVALID`, `CONTACT_POINTS_PRIVACY_SCOPE_PARTY_INVALID`, `CONTACT_POINTS_PRIVACY_SCOPE_GENERATION_INVALID`, `CONTACT_POINTS_PRIVACY_SCOPE_REGISTRY_INVALID`, `CONTACT_POINTS_PRIVACY_SCOPE_REGISTRY_UNAVAILABLE`, `CONTACT_POINTS_PRIVACY_SCOPE_REGISTRY_MISMATCH`, `CONTACT_POINTS_PRIVACY_SCOPE_PURPOSE_INVALID`, `CONTACT_POINTS_PRIVACY_SCOPE_REQUEST_TIME_INVALID`, `CONTACT_POINTS_PRIVACY_SCOPE_PAGE_SIZE_INVALID`, `CONTACT_POINTS_PRIVACY_SCOPE_LINEAGE_INVALID`, `CONTACT_POINTS_PRIVACY_SCOPE_STORED_STATE_INVALID`, `CONTACT_POINTS_PRIVACY_SCOPE_UNAVAILABLE`
- **Rust crate references:** `crm_customer_privacy_owner_scope_support`, `crm_module_sdk`

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-contact-points-privacy-scope-adapter/src/lib.rs) · 33 lines

- **Re-exports:** `pub use contract::{`, `pub use owner_action::{`, `pub use postgres::ContactPointsPrivacyScopeQueryAdapter;`

### [`src/owner_action.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-contact-points-privacy-scope-adapter/src/owner_action.rs) · 85 lines

- **Public declarations (2):** `contact_points_privacy_action_definition@L19`, `contact_points_privacy_action_planner@L23`
- **Internal/trait/helpers (7):** `owner_module_id@L28`, `capability_id@L32`, `supports_resource_type@L36`, `anonymize@L40`, `deletion_tombstone@L48`, `minimize_contact_point@L57`, `publishes_the_frozen_owner_action_coordinate@L71`
- **Public types:** `ContactPointsPrivacyActionPlanner@L13`, `ContactPointsPrivacyActionPolicy@L17`
- **Detected tests (1):** `publishes_the_frozen_owner_action_coordinate@L71`
- **Rust crate references:** `crm_capability_runtime`, `crm_contact_points_capability_adapter`, `crm_customer_privacy_owner_scope_support`, `crm_module_sdk`, `crm_contact_points`

### [`src/postgres.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-contact-points-privacy-scope-adapter/src/postgres.rs) · 295 lines

- **Public declarations (1):** `new@L43`
- **Internal/trait/helpers (7):** `fmt@L34`, `execute_query@L47`, `execute@L83`, `read_contact_point_page@L98`, `strict_contact_point_resource@L217`, `configured@L287`, `_definition_smoke@L292`
- **Public types:** `ContactPointsPrivacyScopeQueryAdapter@L29`
- **Rust crate references:** `crm_capability_runtime`, `crm_contact_points`, `crm_contact_points_capability_adapter`, `crm_core_data`, `crm_customer_privacy_owner_scope_support`, `crm_module_sdk`, `crm_query_runtime`

### [`src/request.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-contact-points-privacy-scope-adapter/src/request.rs) · 180 lines

- **Public declarations (3):** `validate_request_contract@L30`, `validate_wire_request@L35`, `encode_cursor@L90`
- **Internal/trait/helpers (3):** `decode_cursor@L113`, `cursor_digest@L147`, `cursor_invalid@L174`
- **Public types:** `ValidatedRequest@L21`
- **Error/guard literals:** `CONTACT_POINTS_PRIVACY_SCOPE_REQUEST_INVALID`, `CONTACT_POINTS_PRIVACY_SCOPE_LINEAGE_INVALID`, `CONTACT_POINTS_PRIVACY_SCOPE_CURSOR_INVALID`
- **Rust crate references:** `crm_customer_privacy_owner_scope_support`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`

### [`src/response.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-contact-points-privacy-scope-adapter/src/response.rs) · 135 lines

- **Public declarations (2):** `build_response@L23`, `typed_output@L90`
- **Internal/trait/helpers (1):** `page_digest@L106`
- **Public types:** `VerifiedContactPointResource@L18`
- **Rust crate references:** `crm_contact_points`, `crm_contact_points_capability_adapter`, `crm_customer_privacy_owner_scope_support`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/tests.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-contact-points-privacy-scope-adapter/src/tests.rs) · 238 lines

- **Internal/trait/helpers (10):** `context@L25`, `wire_request@L39`, `query_request@L64`, `assert_invalid_argument@L86`, `publishes_exact_contract_only_coordinate@L93`, `request_contract_preserves_contact_points_integrity_errors@L107`, `cursor_round_trip_is_bound_to_lineage_and_page_size@L139`, `common_lineage_failures_keep_contact_points_codes@L171`, `response_is_reference_only_deterministic_and_supports_sparse_progress@L194`, `malformed_wire_request_is_rejected_before_owner_reads@L234`
- **Detected tests (6):** `publishes_exact_contract_only_coordinate@L93`, `request_contract_preserves_contact_points_integrity_errors@L107`, `cursor_round_trip_is_bound_to_lineage_and_page_size@L139`, `common_lineage_failures_keep_contact_points_codes@L171`, `response_is_reference_only_deterministic_and_supports_sparse_progress@L194`, `malformed_wire_request_is_rejected_before_owner_reads@L234`
- **Error/guard literals:** `PRIVACY_ERASURE_SCOPE`, `CONTACT_POINTS_PRIVACY_SCOPE_REQUEST_BINDING_MISMATCH`, `CONTACT_POINTS_PRIVACY_SCOPE_INPUT_HASH_MISMATCH`, `CONTACT_POINTS_PRIVACY_SCOPE_INPUT_CONTRACT_MISMATCH`, `CONTACT_POINTS_PRIVACY_SCOPE_CURSOR_INVALID`, `CONTACT_POINTS_PRIVACY_SCOPE_TENANT_MISMATCH`, `CONTACT_POINTS_PRIVACY_SCOPE_PAGE_SIZE_INVALID`, `CONTACT_POINTS_PRIVACY_SCOPE_REQUEST_INVALID`
- **Rust crate references:** `crm_customer_privacy`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`, `crm_contact_points`

### [`tests/postgres_scope.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-contact-points-privacy-scope-adapter/tests/postgres_scope.rs) · 448 lines

- **Internal/trait/helpers (3):** `contact_points_scope_is_bounded_strict_tenant_bound_and_side_effect_free@L30`, `assert_response_omits_contact_point_state@L424`, `assert_no_query_side_writes@L435`
- **Detected tests (1):** `contact_points_scope_is_bounded_strict_tenant_bound_and_side_effect_free@L30`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`, `CONTACT_POINTS_PRIVACY_SCOPE_CURSOR_INVALID`, `CONTACT_POINTS_PRIVACY_SCOPE_LINEAGE_INVALID`, `CONTACT_POINTS_PRIVACY_SCOPE_STORED_STATE_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_contact_points_capability_adapter`, `crm_contact_points_privacy_scope_adapter`, `crm_core_data`, `crm_parties_capability_adapter`, `crm_proto_contracts`, `crm_query_runtime`, `crm_contact_points`

### [`tests/postgres_scope/support.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-contact-points-privacy-scope-adapter/tests/postgres_scope/support.rs) · 481 lines

- **Public declarations (8):** `create_party@L47`, `create_contact_point@L79`, `verify_contact_point@L120`, `update_contact_point_status@L154`, `scope_request@L230`, `insert_canonical_redirect@L288`, `corrupt_contact_point_metadata@L381`, `write_surface_counts@L441`
- **Internal/trait/helpers (1):** `capability_request@L194`
- **Public types:** `WriteSurfaceCounts@L430`
- **Error/guard literals:** `PRIVACY_ERASURE_SCOPE`
- **Rust crate references:** `crm_capability_runtime`, `crm_contact_points_capability_adapter`, `crm_contact_points_privacy_scope_adapter`, `crm_customer_privacy`, `crm_identity_resolution_capability_adapter`, `crm_module_sdk`, `crm_parties_capability_adapter`, `crm_proto_contracts`, `crm_query_runtime`, `crm_parties`, `crm_contact_points`

## 15. `crm-contact-points-query-adapter` — NOW / A? + B-NOW-if-gap + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-contact-points-query-adapter/src/lib.rs) · 793 lines

- **Public declarations (3):** `new@L58`, `query_capability_definitions@L78`, `query_capability_definition@L85`
- **Internal/trait/helpers (28):** `fmt@L46`, `validate@L118`, `execute@L152`, `execute_get@L169`, `execute_list@L213`, `collect_contact_points@L244`, `has_more_visible_contact_point@L299`, `validate_list@L338`, `contact_point_matches_filters@L411`, `contact_point_to_wire_with_visibility@L461`, `resolve_page_size@L494`, `cursor_binding@L503`, `decode_after@L522`, `encode_next@L551`, `contact_point_filter_hash@L572`, `decode_input@L598`, `validate_record_id@L629`, `contact_point_record_type@L634`, `enforce_scan_limit@L638`, `resource_not_found@L646`, `unsupported_query@L655`, `cursor_error@L664`, `scan_limit_error@L674`, `config_error@L683`, `contact_point@L701`, `publishes_get_and_list_as_personal_read_only_queries@L724`, `field_visibility_redacts_personal_endpoint_fields_without_hiding_identity@L742`, `typed_filters_are_exact_and_cursor_bound@L769`
- **Public types:** `ContactPointQueryAdapter@L38`
- **Detected tests (3):** `publishes_get_and_list_as_personal_read_only_queries@L724`, `field_visibility_redacts_personal_endpoint_fields_without_hiding_identity@L742`, `typed_filters_are_exact_and_cursor_bound@L769`
- **Error/guard literals:** `CONTACT_POINTS_QUERY_CURSOR_INVALID`, `CONTACT_POINTS_QUERY_INPUT_CONTRACT_MISMATCH`, `CONTACT_POINTS_QUERY_INPUT_PROTOBUF_INVALID`, `QUERY_RESOURCE_NOT_FOUND`, `CONTACT_POINTS_QUERY_CAPABILITY_UNSUPPORTED`, `CONTACT_POINTS_QUERY_VISIBILITY_SCAN_LIMIT_EXCEEDED`, `CONTACT_POINTS_QUERY_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_contact_points`, `crm_contact_points_capability_adapter`, `crm_core_data`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`

## 16. `crm-core-data` — RETIRE-REVIEW / D? + B? invariants

### [`src/aggregate_executor.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/src/aggregate_executor.rs) · 288 lines

- **Public declarations (2):** `new@L80`, `guarded@L88`
- **Internal/trait/helpers (13):** `target@L30`, `plan@L36`, `check@L48`, `fmt@L63`, `execute@L102`, `target@L144`, `plan@L155`, `target_resolution_is_deterministic_and_infrastructure_free@L203`, `definition@L214`, `request@L230`, `reference@L255`, `contract@L262`, `payload@L274`
- **Public types:** `AggregatePresence@L12`, `AggregateTarget@L18`, `TransactionalAggregatePlanner@L29`, `TransactionalAggregateGuard@L47`, `PostgresTransactionalAggregateExecutor@L56`
- **Detected tests (1):** `target_resolution_is_deterministic_and_infrastructure_free@L203`
- **Rust crate references:** `crm_capability_runtime`, `crm_module_sdk`

### [`src/aggregate_guard_chain.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/src/aggregate_guard_chain.rs) · 76 lines

- **Public declarations (3):** `new@L25`, `len@L40`, `is_empty@L44`
- **Internal/trait/helpers (3):** `fmt@L16`, `check@L50`, `empty_chain_is_rejected@L69`
- **Public types:** `TransactionalAggregateGuardChain@L11`
- **Detected tests (1):** `empty_chain_is_rejected@L69`
- **Error/guard literals:** `TRANSACTIONAL_GUARD_CHAIN_EMPTY`
- **Rust crate references:** `crm_capability_runtime`, `crm_module_sdk`

### [`src/audit.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/src/audit.rs) · 293 lines

- **Public declarations (2):** `validate@L22`, `materialize_audit_chain@L95`
- **Internal/trait/helpers (11):** `fmt@L69`, `source@L81`, `from@L90`, `audit_record_hash@L167`, `postgres_timestamp_nanos@L203`, `append_field@L207`, `context@L225`, `intent@L245`, `hash_is_deterministic_and_chain_sensitive@L255`, `hash_uses_postgresql_microsecond_timestamp_precision@L269`, `intent_is_bounded_and_requires_canonical_evidence@L286`
- **Public types:** `AuditIntent@L14`, `MaterializedAuditRecord@L51`, `AuditMaterializationError@L62`
- **Detected tests (3):** `hash_is_deterministic_and_chain_sensitive@L255`, `hash_uses_postgresql_microsecond_timestamp_precision@L269`, `intent_is_bounded_and_requires_canonical_evidence@L286`
- **Rust crate references:** `crm_module_sdk`

### [`src/audited_read.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/src/audited_read.rs) · 180 lines

- **Public declarations (2):** `validate@L13`, `record_audited_read@L25`
- **Internal/trait/helpers (5):** `bind_execution_context@L43`, `insert_completion_marker@L69`, `insert_audit_record@L96`, `audit_materialization_to_data_error@L131`, `audited_read_plan_requires_valid_context_and_audit_evidence@L151`
- **Public types:** `AuditedReadPlan@L7`
- **Detected tests (1):** `audited_read_plan_requires_valid_context_and_audit_evidence@L151`
- **Rust crate references:** `crm_module_sdk`

### [`src/capability_executor.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/src/capability_executor.rs) · 544 lines

- **Public declarations (8):** `new@L63`, `from_runtime@L71`, `capability_idempotency_scope@L109`, `validate_executor_definition@L116`, `validate_execution_plan@L130`, `validate_transactional_aggregate_execution_plan@L143`, `capability_batch_error_to_sdk@L201`, `affected_resources@L238`
- **Internal/trait/helpers (23):** `plan@L23`, `execute_batch@L31`, `execute_batch@L38`, `fmt@L53`, `execute@L80`, `validate_execution_plan_identity@L156`, `validate_planned_output@L174`, `invalid_execution_plan@L192`, `relationship_resource@L265`, `execute_batch@L306`, `plan@L325`, `executes_one_batch_and_propagates_replay_state@L335`, `rejects_context_mismatch_before_batch_runtime@L366`, `maps_idempotency_key_reuse_without_human_text_parsing@L387`, `idempotency_scope_is_capability_version_bound@L407`, `definition@L414`, `contract@L430`, `request@L442`, `context@L451`, `execution_plan@L471`, `record_ref@L514`, `payload@L521`, `empty_result@L535`
- **Public types:** `CapabilityBatchExecutionPlan@L13`, `CapabilityBatchPlanner@L22`, `BatchMutationRuntime@L30`, `PostgresTransactionalCapabilityExecutor@L47`
- **Detected tests (5):** `executes_one_batch_and_propagates_replay_state@L335`, `rejects_context_mismatch_before_batch_runtime@L366`, `maps_idempotency_key_reuse_without_human_text_parsing@L387`, `idempotency_scope_is_capability_version_bound@L407`, `definition@L414`
- **Error/guard literals:** `CAPABILITY_EXECUTOR_DEFINITION_INVALID`, `CAPABILITY_EXECUTION_PLAN_INVALID`, `CAPABILITY_CONFLICT`, `CAPABILITY_IDEMPOTENCY_KEY_REUSED`, `CAPABILITY_IDEMPOTENCY_IN_PROGRESS`, `CAPABILITY_STORAGE_UNAVAILABLE`, `CAPABILITY_IDEMPOTENCY_STATE_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_module_sdk`

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/src/lib.rs) · 68 lines

- **Re-exports:** `pub use aggregate_executor::*;`, `pub use aggregate_guard_chain::TransactionalAggregateGuardChain;`, `pub use audit::AuditIntent;`, `pub use audited_read::*;`, `pub use capability_executor::*;`, `pub use crm_capability_runtime::CapabilityRequest;`, `pub use crm_core_files::{`, `pub use crm_module_sdk::RecordSnapshot;`, `pub use metadata_capability_executor::*;`, `pub use metadata_query_store::*;`, `pub use metadata_store::*;`, `pub use postgres::*;`, `pub use postgres_batch::*;`, `pub use postgres_bound_read::BoundReadTransaction;`, `pub use postgres_event_delivery::EventDeliveryQuery;`, `pub use postgres_event_delivery_ledger::{`, `pub use postgres_file_artifact::PostgresImmutableFileArtifactStore;`, `pub use postgres_file_artifact_capability::{`, `pub use postgres_query::{`, `pub use postgres_related_query::{`, `pub use privacy_owner_action::*;`, `pub use subject_policy::{CustomerSubjectOperationClass, TransactionalCustomerSubjectPolicyPort};`, `pub use sqlx as postgres_sqlx;`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_files`, `crm_module_sdk`

### [`src/metadata_capability_executor.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/src/metadata_capability_executor.rs) · 804 lines

- **Public declarations (1):** `new@L44`
- **Internal/trait/helpers (20):** `execute@L50`, `decode_metadata_command@L144`, `apply_metadata_command@L205`, `apply_publish@L250`, `apply_activate@L342`, `activation_applied@L421`, `apply_rollback@L448`, `revision_resource@L531`, `activation_resource@L539`, `generation_i64@L547`, `metadata_audit_intent@L555`, `metadata_audit_record_id@L617`, `sha256_hex@L634`, `hex@L638`, `audit_materialization_to_batch_error@L648`, `metadata_persistence_error_to_sdk@L658`, `metadata_configuration_error@L726`, `request@L740`, `strict_publish_command_is_decoded_before_any_database_work@L766`, `mutation_executor_rejects_query_coordinates@L788`
- **Public types:** `PostgresMetadataCapabilityExecutor@L39`
- **Detected tests (2):** `strict_publish_command_is_decoded_before_any_database_work@L766`, `mutation_executor_rejects_query_coordinates@L788`
- **Error/guard literals:** `METADATA_MUTATION_ROUTE_UNSUPPORTED`, `METADATA_MUTATION_BINDING_MISMATCH`, `METADATA_AUDIT_ENVELOPE_SERIALIZATION_FAILED`, `METADATA_REVISION_NOT_FOUND`, `METADATA_GENERATION_CONFLICT`, `METADATA_BREAKING_CHANGE_CONFIRMATION_REQUIRED`, `METADATA_ROLLBACK_UNAVAILABLE`, `METADATA_STORAGE_UNAVAILABLE`, `METADATA_MUTATION_INVALID`, `METADATA_REVISION_IDENTITY_COLLISION`, `METADATA_STORED_VALUE_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_metadata_api_adapter`, `crm_metadata_runtime`, `crm_module_sdk`, `crm_proto_contracts`, `crm_metadata_schema`

### [`src/metadata_query_store.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/src/metadata_query_store.rs) · 440 lines

- **Public declarations (1):** `new@L25`
- **Internal/trait/helpers (20):** `load_revision@L29`, `load_tenant_state@L41`, `load_impact@L52`, `impact_for@L89`, `revision@L101`, `tenant_state@L113`, `bind_tenant_scope@L125`, `load_bundle@L136`, `load_state@L242`, `impact_from_runtime@L277`, `metadata_key_from_stored@L304`, `parse_kind@L313`, `revision_id_from_bytes@L329`, `stored_u64@L340`, `stored_usize@L346`, `fmt@L363`, `source@L387`, `from@L397`, `from@L403`, `metadata_query_error_to_sdk@L408`
- **Public types:** `PostgresMetadataQueryStore@L20`
- **Error/guard literals:** `METADATA_REVISION_NOT_FOUND`, `METADATA_QUERY_STORE_UNAVAILABLE`, `METADATA_QUERY_STORED_CONTRACT_INVALID`, `METADATA_QUERY_STORED_VALUE_INVALID`
- **Rust crate references:** `crm_metadata_query_adapter`, `crm_metadata_runtime`, `crm_module_sdk`

### [`src/metadata_store.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/src/metadata_store.rs) · 1097 lines

- **Public declarations (19):** `new@L22`, `publish@L26`, `revision@L110`, `tenant_state@L124`, `impact_for@L136`, `activate@L156`, `rollback@L254`, `transitions@L324`, `lock_activation@L527`, `insert_documents@L539`, `load_bundle@L595`, `load_state@L701`, `load_impact@L745`, `require_generation@L802`, `insert_rollback_stack_entry@L812`, `load_rollback_target@L841`, `delete_rollback_stack_entry@L862`, `upsert_activation_head@L885`, `insert_transition@L929`
- **Internal/trait/helpers (18):** `as_str@L368`, `parse@L376`, `fmt@L419`, `source@L460`, `from@L477`, `from@L483`, `validate_operation@L488`, `bind_execution_context@L501`, `impact_from_runtime@L775`, `transition_id@L985`, `decode_transition@L1004`, `metadata_key_from_stored@L1026`, `kind_name@L1035`, `parse_kind@L1048`, `revision_id_from_bytes@L1064`, `stored_u64@L1073`, `stored_usize@L1079`, `database_i64@L1087`
- **Public types:** `PostgresMetadataStore@L17`, `MetadataTransitionAction@L361`, `MetadataTransitionEvidence@L389`, `MetadataPersistenceError@L405`, `MetadataTransitionWrite@L920`
- **Rust crate references:** `crm_metadata_runtime`, `crm_module_sdk`

### [`src/module_activation.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/src/module_activation.rs) · 220 lines

- **Public declarations (1):** `bootstrap_activate_published_modules@L23`
- **Internal/trait/helpers (6):** `bootstrap_activate_tenant_modules@L36`, `bind_bootstrap_context@L151`, `bootstrap_grant_digest@L179`, `bootstrap_context_missing@L191`, `database_unavailable@L201`, `stored_value_invalid@L211`
- **Error/guard literals:** `APPLICATION_BOOTSTRAP_TRANSACTION_CONTEXT_MISSING`, `APPLICATION_MODULE_INSTALLATION_DATABASE_UNAVAILABLE`, `APPLICATION_BOOTSTRAP_TRANSACTION_CONTEXT_INVALID`
- **Rust crate references:** `crm_module_sdk`

### [`src/postgres_batch.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/src/postgres_batch.rs) · 31 lines

- **Rust crate references:** `crm_capability_runtime`, `crm_module_sdk`

### [`src/postgres_batch/evidence.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/src/postgres_batch/evidence.rs) · 198 lines

- **Public declarations (3):** `insert_audit_record@L45`, `complete_capability_idempotency@L143`, `insert_completion_marker@L165`
- **Internal/trait/helpers (3):** `insert_outbox_event@L1`, `complete_batch_idempotency@L80`, `complete_idempotency_response@L100`

### [`src/postgres_batch/executor.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/src/postgres_batch/executor.rs) · 457 lines

- **Public declarations (7):** `execute_batch@L7`, `execute_batch_with_fault@L15`, `execute_transactional_aggregate@L50`, `capability_idempotency@L123`, `bind_execution_context@L261`, `load_capability_replay@L314`, `insert_idempotency_claim@L414`
- **Internal/trait/helpers (7):** `validate_target_presence@L142`, `validate_target_mutation@L163`, `apply_planned_batch@L211`, `load_batch_replay@L287`, `load_idempotency_response@L346`, `validate_response_metadata@L396`, `capability_result_descriptor_hash@L454`
- **Error/guard literals:** `CAPABILITY_AGGREGATE_ALREADY_EXISTS`, `CAPABILITY_AGGREGATE_NOT_FOUND`

### [`src/postgres_batch/helpers.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/src/postgres_batch/helpers.rs) · 120 lines

- **Public declarations (3):** `batch_error_to_sdk@L53`, `parse_data_class@L92`, `parse_payload_encoding@L109`
- **Internal/trait/helpers (6):** `checked_size@L1`, `relationship_key@L5`, `batch_result_descriptor_hash@L16`, `data_class_name@L20`, `payload_encoding_name@L34`, `audit_materialization_to_batch_error@L43`
- **Error/guard literals:** `DATA_INVALID`, `DATA_CONFLICT`, `DATA_IDEMPOTENCY_IN_PROGRESS`, `DATA_UNAVAILABLE`

### [`src/postgres_batch/model.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/src/postgres_batch/model.rs) · 294 lines

- **Public declarations (2):** `validate@L75`, `validate_transactional_aggregate@L90`
- **Internal/trait/helpers (7):** `reference@L21`, `payload@L27`, `relationship@L47`, `validate_common@L106`, `fmt@L257`, `source@L276`, `from@L290`
- **Public types:** `RecordMutation@L8`, `RelationshipMutation@L36`, `EventEvidence@L56`, `BatchMutationPlan@L65`, `BatchMutationResult@L238`, `BatchError@L246`

### [`src/postgres_batch/records.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/src/postgres_batch/records.rs) · 302 lines

- **Public declarations (1):** `load_record_for_update@L3`
- **Internal/trait/helpers (7):** `lock_aggregate_target@L44`, `decode_locked_record@L69`, `apply_record_mutation@L110`, `insert_record@L127`, `update_record@L167`, `link_relationship@L227`, `unlink_relationship@L268`

### [`src/postgres_batch/tests.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/src/postgres_batch/tests.rs) · 152 lines

- **Internal/trait/helpers (13):** `context@L10`, `reference@L30`, `payload@L37`, `event@L51`, `audit@L67`, `plan@L76`, `accepts_valid_batch@L96`, `rejects_duplicate_record_mutation@L101`, `rejects_duplicate_audit_record_identity@L112`, `rejects_empty_audit_envelope@L119`, `aggregate_noop_requires_audit_but_not_outbox@L126`, `aggregate_noop_rejects_false_outbox_evidence@L136`, `descriptor_hash_is_stable@L147`
- **Detected tests (7):** `accepts_valid_batch@L96`, `rejects_duplicate_record_mutation@L101`, `rejects_duplicate_audit_record_identity@L112`, `rejects_empty_audit_envelope@L119`, `aggregate_noop_requires_audit_but_not_outbox@L126`, `aggregate_noop_rejects_false_outbox_evidence@L136`, `descriptor_hash_is_stable@L147`
- **Rust crate references:** `crm_module_sdk`

### [`src/postgres_bound_read.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/src/postgres_bound_read.rs) · 77 lines

- **Public declarations (2):** `commit@L41`, `begin_bound_read_transaction@L50`
- **Internal/trait/helpers (4):** `fmt@L17`, `deref@L29`, `deref_mut@L35`, `database_unavailable@L68`
- **Public types:** `BoundReadTransaction@L12`
- **Error/guard literals:** `DATA_BOUND_READ_UNAVAILABLE`
- **Rust crate references:** `crm_module_sdk`

### [`src/postgres_event_delivery_ledger.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/src/postgres_event_delivery_ledger.rs) · 345 lines

- **Public declarations (4):** `claim_event_delivery@L41`, `complete_event_delivery@L147`, `retry_event_delivery@L188`, `dead_letter_event_delivery@L239`
- **Internal/trait/helpers (8):** `database_status@L27`, `bind_tenant@L281`, `validate_worker_id@L293`, `validate_error_code@L303`, `delivery_invalid@L313`, `delivery_conflict@L317`, `stored_value_invalid@L326`, `database_unavailable@L336`
- **Public types:** `ClaimedEventDelivery@L7`, `EventDeliveryClaim@L13`, `EventDeliveryCompletion@L21`
- **Error/guard literals:** `EVENT_DELIVERY_LEASE_INVALID`, `EVENT_DELIVERY_RETRY_INVALID`, `EVENT_DELIVERY_WORKER_INVALID`, `EVENT_DELIVERY_ERROR_CODE_INVALID`, `EVENT_DELIVERY_LEASE_CONFLICT`, `EVENT_DELIVERY_STORED_VALUE_INVALID`, `EVENT_DELIVERY_UNAVAILABLE`
- **Rust crate references:** `crm_module_sdk`

### [`src/postgres_event_delivery.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/src/postgres_event_delivery.rs) · 335 lines

- **Public declarations (2):** `get_event_delivery@L27`, `is_module_active@L79`
- **Internal/trait/helpers (9):** `bind_read_tenant@L104`, `decode_event_delivery@L120`, `deterministic_delivery_id@L254`, `hash_field@L274`, `parse_identifier@L279`, `batch_decode_error@L291`, `stored_value_invalid@L295`, `database_unavailable@L305`, `delivery_identity_is_consumer_scoped_and_stable@L320`
- **Public types:** `EventDeliveryQuery@L15`
- **Detected tests (1):** `delivery_identity_is_consumer_scoped_and_stable@L320`
- **Error/guard literals:** `EVENT_DELIVERY_STORED_VALUE_INVALID`, `EVENT_DELIVERY_UNAVAILABLE`
- **Rust crate references:** `crm_module_sdk`

### [`src/postgres_file_artifact_capability.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/src/postgres_file_artifact_capability.rs) · 600 lines

- **Public declarations (1):** `execute_file_artifact_capability@L70`
- **Internal/trait/helpers (11):** `validate@L44`, `apply_file_artifact_mutation@L151`, `load_metadata_for_update@L388`, `verify_chunks@L409`, `decode_metadata@L469`, `require_owner@L537`, `data_class_to_db@L548`, `data_class_from_db@L562`, `file_not_found@L579`, `owner_mismatch@L588`, `file_conflict@L597`
- **Public types:** `FileArtifactCapabilityMutation@L22`, `FileArtifactCapabilityMutationResult@L29`, `FileArtifactCapabilityEvidence@L36`
- **Error/guard literals:** `FILE_ARTIFACT_AUDIT_MATERIALIZATION_FAILED`, `FILE_ARTIFACT_CREATE_CONFLICT`, `FILE_ARTIFACT_ALREADY_FINALIZED`, `FILE_ARTIFACT_CHUNK_REPLAY_CONFLICT`, `FILE_ARTIFACT_CHUNK_OUT_OF_ORDER`, `FILE_ARTIFACT_SIZE_OVERFLOW`, `FILE_ARTIFACT_SIZE_EXCEEDED`, `FILE_ARTIFACT_UPLOAD_INCOMPLETE`, `FILE_ARTIFACT_DIGEST_MISMATCH`, `FILE_ARTIFACT_NOT_FOUND`, `FILE_ARTIFACT_OWNER_MISMATCH`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_files`, `crm_module_sdk`

### [`src/postgres_file_artifact_evidence.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/src/postgres_file_artifact_evidence.rs) · 73 lines

- **Public declarations (1):** `insert_file_artifact_outbox_event@L5`
- **Internal/trait/helpers (2):** `data_class_name@L51`, `payload_encoding_name@L65`
- **Rust crate references:** `crm_module_sdk`

### [`src/postgres_file_artifact.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/src/postgres_file_artifact.rs) · 603 lines

- **Public declarations (1):** `new@L19`
- **Internal/trait/helpers (18):** `create@L25`, `append_chunk@L96`, `finalize@L239`, `read_finalized@L294`, `bind_tenant@L331`, `load_metadata_for_update@L343`, `load_metadata@L365`, `load_and_verify_chunks@L386`, `decode_metadata@L449`, `require_owner@L513`, `data_class_to_db@L524`, `data_class_from_db@L538`, `artifact_not_found@L553`, `owner_mismatch@L562`, `artifact_conflict@L571`, `database_unavailable@L575`, `invalid_stored_value@L585`, `file_error@L595`
- **Public types:** `PostgresImmutableFileArtifactStore@L14`
- **Error/guard literals:** `FILE_ARTIFACT_CREATE_CONFLICT`, `FILE_ARTIFACT_ALREADY_FINALIZED`, `FILE_ARTIFACT_CHUNK_REPLAY_CONFLICT`, `FILE_ARTIFACT_CHUNK_OUT_OF_ORDER`, `FILE_ARTIFACT_SIZE_OVERFLOW`, `FILE_ARTIFACT_SIZE_EXCEEDED`, `FILE_ARTIFACT_UPLOAD_INCOMPLETE`, `FILE_ARTIFACT_NOT_FINALIZED`, `FILE_ARTIFACT_DIGEST_MISMATCH`, `FILE_ARTIFACT_NOT_FOUND`, `FILE_ARTIFACT_OWNER_MISMATCH`, `FILE_ARTIFACT_DATABASE_UNAVAILABLE`, `FILE_ARTIFACT_STORED_VALUE_INVALID`
- **Rust crate references:** `crm_core_files`, `crm_module_sdk`

### [`src/postgres_projection.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/src/postgres_projection.rs) · 553 lines

- **Public declarations (6):** `list_event_history@L11`, `apply_projection_event@L119`, `projection_checkpoint@L306`, `reset_projection@L346`, `projection_document@L373`, `projection_documents@L424`
- **Internal/trait/helpers (7):** `bind_projection_tenant@L470`, `decode_checkpoint@L482`, `validate_projection_id@L509`, `projection_request_invalid@L516`, `projection_conflict@L525`, `projection_stored_value_invalid@L534`, `projection_database_error@L544`
- **Error/guard literals:** `PROJECTION_REQUEST_INVALID`, `PROJECTION_CHECKPOINT_CONFLICT`, `PROJECTION_STORED_VALUE_INVALID`, `PROJECTION_STORAGE_UNAVAILABLE`
- **Rust crate references:** `crm_core_events`, `crm_module_sdk`

### [`src/postgres_query.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/src/postgres_query.rs) · 439 lines

- **Public declarations (5):** `id@L18`, `validate@L33`, `validate@L66`, `get_record_for_query@L87`, `list_records_for_query@L128`
- **Internal/trait/helpers (9):** `bind_read_context@L262`, `decode_query_record@L278`, `batch_decode_error@L354`, `invalid_query@L358`, `stored_value_invalid@L362`, `database_unavailable@L372`, `record_list_query_rejects_zero_and_excessive_page_sizes@L387`, `record_query_continuation_requires_canonical_epoch_microseconds@L411`, `record_query_sort_ids_are_stable_contract_values@L428`
- **Public types:** `RecordQuerySort@L12`, `RecordQueryContinuation@L27`, `RecordGetQuery@L48`, `RecordListQuery@L56`, `RecordQueryPage@L81`
- **Detected tests (3):** `record_list_query_rejects_zero_and_excessive_page_sizes@L387`, `record_query_continuation_requires_canonical_epoch_microseconds@L411`, `record_query_sort_ids_are_stable_contract_values@L428`
- **Error/guard literals:** `DATA_QUERY_CONTINUATION_INVALID`, `DATA_QUERY_PAGE_SIZE_INVALID`, `DATA_QUERY_STORED_VALUE_INVALID`, `DATA_QUERY_UNAVAILABLE`
- **Rust crate references:** `crm_module_sdk`

### [`src/postgres_related_query.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/src/postgres_related_query.rs) · 277 lines

- **Public declarations (2):** `validate@L28`, `list_related_records_for_query@L46`
- **Internal/trait/helpers (7):** `bind_read_context@L125`, `decode_related_record@L141`, `batch_decode_error@L217`, `invalid_query@L221`, `stored_value_invalid@L225`, `database_unavailable@L235`, `related_query_rejects_zero_and_oversized_pages@L250`
- **Public types:** `RelatedRecordListQuery@L16`, `RelatedRecordQueryPage@L40`
- **Detected tests (1):** `related_query_rejects_zero_and_oversized_pages@L250`
- **Error/guard literals:** `DATA_RELATED_QUERY_PAGE_SIZE_INVALID`, `DATA_QUERY_STORED_VALUE_INVALID`, `DATA_QUERY_UNAVAILABLE`
- **Rust crate references:** `crm_module_sdk`

### [`src/postgres.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/src/postgres.rs) · 564 lines

- **Public declarations (9):** `validate@L35`, `connect@L137`, `from_pool@L146`, `observe_event_delivery@L153`, `pool@L159`, `create_record@L163`, `create_record_with_fault@L172`, `get_record@L206`, `database_error_to_sdk@L546`
- **Internal/trait/helpers (16):** `require_plan@L61`, `fmt@L85`, `source@L98`, `from@L108`, `fmt@L122`, `from@L130`, `bind_execution_context@L247`, `insert_record@L273`, `insert_idempotency@L323`, `insert_outbox_event@L355`, `insert_audit_record@L410`, `audit_materialization_to_data_error@L454`, `insert_completion_marker@L464`, `decode_record@L495`, `enum_name@L533`, `parse_enum@L541`
- **Public types:** `IdempotencyEvidence@L16`, `RecordCreatePlan@L24`, `FaultInjection@L68`, `DataError@L77`, `PostgresDataStore@L116`
- **Error/guard literals:** `DATA_INVALID`, `DATA_UNAVAILABLE`
- **Rust crate references:** `crm_module_sdk`

### [`src/privacy_owner_action.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/src/privacy_owner_action.rs) · 424 lines

- **Public declarations (1):** `new@L85`
- **Internal/trait/helpers (15):** `target@L54`, `plan@L60`, `fmt@L75`, `execute@L91`, `validate_request_identity@L199`, `evidence_plan@L214`, `validate_owner_plan@L237`, `apply_owner_record_action@L258`, `insert_owner_outbox_event@L319`, `data_class_name@L364`, `payload_encoding_name@L378`, `audit_materialization_to_batch_error@L387`, `owner_record_not_found@L397`, `owner_record_stale@L406`, `invalid_plan@L415`
- **Public types:** `PrivacyOwnerRecordAction@L28`, `PrivacyOwnerActionTarget@L34`, `PrivacyOwnerActionPlan@L40`, `PrivacyOwnerActionPlanner@L53`, `PostgresPrivacyOwnerActionExecutor@L69`
- **Error/guard literals:** `PRIVACY_OWNER_RECORD_NOT_FOUND`, `PRIVACY_OWNER_RECORD_STALE`, `PRIVACY_OWNER_ACTION_PLAN_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_module_sdk`

### [`src/projection_store.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/src/projection_store.rs) · 207 lines

- **Internal/trait/helpers (12):** `projection_checkpoint@L10`, `list_event_history@L22`, `apply_projection_event@L29`, `mark_projection_failed@L36`, `reset_projection@L40`, `ensure_projection_active_or_absent@L52`, `record_projection_failure@L118`, `bind_projection_tenant@L160`, `validate_projection_id@L172`, `projection_request_invalid@L179`, `projection_stored_value_invalid@L188`, `projection_storage_error@L198`
- **Error/guard literals:** `PROJECTION_CHECKPOINT_FAILED`, `PROJECTION_REQUEST_INVALID`, `PROJECTION_STORED_VALUE_INVALID`, `PROJECTION_STORAGE_UNAVAILABLE`
- **Rust crate references:** `crm_core_events`, `crm_module_sdk`

### [`src/search_generation_store.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/src/search_generation_store.rs) · 126 lines

- **Internal/trait/helpers (6):** `active_generation@L9`, `register_building_generation@L58`, `activate_generation@L82`, `bind_search_tenant@L95`, `search_stored_value_invalid@L107`, `search_storage_error@L117`
- **Error/guard literals:** `SEARCH_GENERATION_STATUS_INVALID`, `SEARCH_STORED_VALUE_INVALID`, `SEARCH_STORAGE_UNAVAILABLE`
- **Rust crate references:** `crm_module_sdk`, `crm_search_runtime`

### [`src/search_store.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/src/search_store.rs) · 392 lines

- **Public declarations (2):** `register_search_generation@L25`, `activate_search_generation@L67`
- **Internal/trait/helpers (11):** `search_candidates@L16`, `search_projection_candidates@L117`, `decode_search_candidate@L268`, `string_field@L308`, `string_map_field@L315`, `bind_search_tenant@L336`, `validate_coordinate@L348`, `search_request_invalid@L355`, `search_generation_conflict@L364`, `search_stored_value_invalid@L373`, `search_storage_error@L383`
- **Error/guard literals:** `SEARCH_GENERATION_NOT_BUILDING`, `SEARCH_INDEX_NOT_ACTIVE`, `SEARCH_REQUEST_INVALID`, `SEARCH_GENERATION_COORDINATE_CONFLICT`, `SEARCH_STORED_VALUE_INVALID`, `SEARCH_STORAGE_UNAVAILABLE`
- **Rust crate references:** `crm_module_sdk`, `crm_search_runtime`

### [`src/subject_policy.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/src/subject_policy.rs) · 62 lines

- **Public declarations (1):** `label@L14`
- **Internal/trait/helpers (2):** `lock_and_enforce@L33`, `operation_classes_are_explicit_and_non_granting@L47`
- **Public types:** `CustomerSubjectOperationClass@L8`, `TransactionalCustomerSubjectPolicyPort@L32`
- **Detected tests (1):** `operation_classes_are_explicit_and_non_granting@L47`
- **Rust crate references:** `crm_capability_runtime`, `crm_module_sdk`

### [`tests/postgres_advanced.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/tests/postgres_advanced.rs) · 25 lines

- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_module_sdk`

### [`tests/postgres_advanced/database.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/tests/postgres_advanced/database.rs) · 88 lines

- **Internal/trait/helpers (4):** `bind_context@L1`, `audit_head@L27`, `record_count@L48`, `relationship_count@L68`

### [`tests/postgres_advanced/plans.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/tests/postgres_advanced/plans.rs) · 177 lines

- **Internal/trait/helpers (5):** `faulted_multi_record_plan@L1`, `create_and_link_plan@L40`, `update_plan@L104`, `unlink_plan@L129`, `concurrent_create_plan@L154`

### [`tests/postgres_advanced/scenario.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/tests/postgres_advanced/scenario.rs) · 297 lines

- **Internal/trait/helpers (2):** `batch_executor_is_atomic_idempotent_and_optimistic@L2`, `transaction_aware_executor_locks_authoritative_state_and_replays_original_output@L160`
- **Detected tests (2):** `batch_executor_is_atomic_idempotent_and_optimistic@L2`, `transaction_aware_executor_locks_authoritative_state_and_replays_original_output@L160`
- **Error/guard literals:** `DATABASE_URL`, `CAPABILITY_AGGREGATE_ALREADY_EXISTS`
- **Rust crate references:** `crm_module_sdk`

### [`tests/postgres_advanced/support.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/tests/postgres_advanced/support.rs) · 386 lines

- **Internal/trait/helpers (15):** `context@L6`, `payload@L27`, `record@L41`, `relationship@L48`, `record_event@L56`, `audit@L79`, `idempotency@L88`, `target@L101`, `plan@L119`, `aggregate_definition@L204`, `aggregate_contract@L220`, `aggregate_request@L232`, `target@L267`, `plan@L285`, `creating_request@L358`
- **Error/guard literals:** `TEST_AGGREGATE_NOT_FOUND`, `TEST_AGGREGATE_VERSION_CONFLICT`, `TEST_AGGREGATE_ALREADY_EXISTS`
- **Rust crate references:** `crm_module_sdk`

### [`tests/postgres_event_delivery.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/tests/postgres_event_delivery.rs) · 392 lines

- **Internal/trait/helpers (5):** `reconstructs_restart_safe_delivery_and_enforces_active_installation@L28`, `seed_plan@L271`, `set_installation_status@L340`, `tenant@L385`, `module@L389`
- **Detected tests (1):** `reconstructs_restart_safe_delivery_and_enforces_active_installation@L28`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`, `EVENT_DELIVERY_LEASE_CONFLICT`, `TARGET_TEMPORARILY_UNAVAILABLE`
- **Rust crate references:** `crm_core_data`, `crm_core_events`, `crm_module_sdk`

### [`tests/postgres_file_artifact.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/tests/postgres_file_artifact.rs) · 169 lines

- **Internal/trait/helpers (5):** `immutable_artifact_upload_is_sequential_replay_safe_finalized_and_tenant_isolated@L22`, `append@L119`, `chunk_command@L131`, `context@L140`, `cleanup@L162`
- **Detected tests (1):** `immutable_artifact_upload_is_sequential_replay_safe_finalized_and_tenant_isolated@L22`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`, `FILE_ARTIFACT_CHUNK_OUT_OF_ORDER`, `FILE_ARTIFACT_ALREADY_FINALIZED`, `FILE_ARTIFACT_NOT_FOUND`
- **Rust crate references:** `crm_core_data`, `crm_core_files`, `crm_module_sdk`

### [`tests/postgres_foundation.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/tests/postgres_foundation.rs) · 443 lines

- **Internal/trait/helpers (9):** `context@L18`, `payload@L44`, `plan@L58`, `postgres_adapter_enforces_atomicity_and_tenant_visibility@L91`, `file_artifact_capability_commits_business_state_and_evidence_atomically@L152`, `file_capability_definition@L377`, `file_capability_request@L406`, `write_artifact_diagnostic@L427`, `count@L435`
- **Detected tests (2):** `postgres_adapter_enforces_atomicity_and_tenant_visibility@L91`, `file_artifact_capability_commits_business_state_and_evidence_atomically@L152`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`, `TEST_FILE_EVIDENCE_FAILURE`, `CARGO_MANIFEST_DIR`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_core_files`, `crm_module_sdk`

### [`tests/postgres_metadata_capability.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/tests/postgres_metadata_capability.rs) · 438 lines

- **Internal/trait/helpers (10):** `governed_metadata_lifecycle_is_atomic_audited_replay_safe_and_pop_only@L22`, `bundle_v1_definitions@L268`, `bundle_v2_definitions@L272`, `object_definition@L276`, `field_definition@L283`, `request@L290`, `assert_transaction_marker@L333`, `assert_latest_audit@L361`, `cleanup@L393`, `evidence_counts@L416`
- **Detected tests (1):** `governed_metadata_lifecycle_is_atomic_audited_replay_safe_and_pop_only@L22`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`, `METADATA_BREAKING_CHANGE_CONFIRMATION_REQUIRED`, `METADATA_ROLLBACK_UNAVAILABLE`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_metadata_api_adapter`, `crm_metadata_schema`, `crm_module_sdk`, `crm_proto_contracts`

### [`tests/postgres_metadata_query.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/tests/postgres_metadata_query.rs) · 216 lines

- **Internal/trait/helpers (6):** `metadata_query_store_is_tenant_only_non_mutating_and_runtime_impact_driven@L19`, `bundle@L125`, `key@L150`, `context@L154`, `cleanup@L181`, `evidence_counts@L200`
- **Detected tests (1):** `metadata_query_store_is_tenant_only_non_mutating_and_runtime_impact_driven@L19`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`, `METADATA_REVISION_NOT_FOUND`
- **Rust crate references:** `crm_core_data`, `crm_metadata_query_adapter`, `crm_metadata_runtime`, `crm_module_sdk`

### [`tests/postgres_metadata.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/tests/postgres_metadata.rs) · 454 lines

- **Internal/trait/helpers (7):** `durable_metadata_publication_is_tenant_isolated_optimistic_and_rollback_safe@L20`, `bundle@L306`, `key@L331`, `context@L335`, `cleanup@L362`, `assert_force_rls@L379`, `assert_immutable_revision@L405`
- **Detected tests (1):** `durable_metadata_publication_is_tenant_isolated_optimistic_and_rollback_safe@L20`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`
- **Rust crate references:** `crm_core_data`, `crm_metadata_runtime`, `crm_module_sdk`

### [`tests/postgres_projection_runtime.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/tests/postgres_projection_runtime.rs) · 133 lines

- **Internal/trait/helpers (2):** `failed_projection_checkpoint_preserves_last_success_and_requires_reset@L13`, `cleanup@L123`
- **Detected tests (1):** `failed_projection_checkpoint_preserves_last_success_and_requires_reset@L13`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`, `TEST_PROJECTION_POISON`, `PROJECTION_CHECKPOINT_FAILED`
- **Rust crate references:** `crm_core_data`, `crm_core_events`, `crm_module_sdk`

### [`tests/postgres_query.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/tests/postgres_query.rs) · 341 lines

- **Internal/trait/helpers (11):** `tenant_scoped_get_and_keyset_list_are_stable_and_read_only@L41`, `seed_plan@L188`, `reference@L252`, `payload@L259`, `record_ids@L273`, `tenant@L280`, `module@L284`, `record_type@L288`, `record_id@L292`, `evidence_counts@L296`, `scalar_count@L326`
- **Detected tests (1):** `tenant_scoped_get_and_keyset_list_are_stable_and_read_only@L41`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`
- **Rust crate references:** `crm_core_data`, `crm_module_sdk`

### [`tests/postgres_search.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-data/tests/postgres_search.rs) · 310 lines

- **Internal/trait/helpers (6):** `active_generation_search_is_deterministic_switchable_and_tenant_isolated@L23`, `request@L153`, `unfiltered_request@L168`, `seed_projection_generation@L183`, `seed_non_active_party@L239`, `cleanup@L283`
- **Detected tests (1):** `active_generation_search_is_deterministic_switchable_and_tenant_isolated@L23`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`
- **Rust crate references:** `crm_core_data`, `crm_module_sdk`, `crm_search_runtime`

## 17. `crm-core-events` — LATER / B-CONDITIONAL / D?

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-events/src/lib.rs) · 185 lines

- **Public declarations (5):** `validate@L21`, `effective_page_size@L39`, `validate@L73`, `validate@L95`, `validate@L136`
- **Internal/trait/helpers (5):** `projection_checkpoint@L158`, `list_event_history@L164`, `apply_projection_event@L169`, `mark_projection_failed@L174`, `reset_projection@L176`
- **Public types:** `EventHistoryCursor@L15`, `EventHistoryRequest@L30`, `EventHistoryPage@L58`, `ProjectionDocumentWrite@L65`, `ProjectionEventApplication@L88`, `ProjectionApplyResult@L113`, `ProjectionCheckpoint@L119`, `ProjectionFailure@L127`, `ProjectionStoreFuture@L150`, `ProjectionStore@L157`
- **Rust crate references:** `crm_module_sdk`

## 18. `crm-core-files` — LATER / B-CONDITIONAL / D?

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-files/src/lib.rs) · 234 lines

- **Public declarations (4):** `validate@L39`, `validate@L76`, `validate@L91`, `validate@L123`
- **Internal/trait/helpers (8):** `create@L140`, `append_chunk@L146`, `finalize@L152`, `read_finalized@L158`, `validate_media_type@L165`, `validate_expected_size_and_hash@L179`, `file_error@L195`, `rejects_oversized_artifacts_and_chunks@L212`
- **Public types:** `FileArtifactStatus@L19`, `FileArtifactMetadata@L25`, `CreateImmutableFileArtifact@L65`, `AppendImmutableFileChunk@L83`, `FileArtifactAppendResult@L111`, `FinalizedFileArtifact@L117`, `ImmutableFileArtifactStore@L139`
- **Detected tests (1):** `rejects_oversized_artifacts_and_chunks@L212`
- **Error/guard literals:** `FILE_ARTIFACT_RECEIVED_SIZE_INVALID`, `FILE_ARTIFACT_FINALIZED_SIZE_INVALID`, `FILE_ARTIFACT_FINALIZED_BYTES_INVALID`, `SDK_INVALID_ARGUMENT`
- **Rust crate references:** `crm_module_sdk`

## 19. `crm-core-integrations` — END / C?/D?

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-integrations/src/lib.rs) · 5 lines


## 20. `crm-core-permissions` — END / C?/D?

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-permissions/src/lib.rs) · 5 lines


## 21. `crm-core-workflow` — END / C?/D?

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-core-workflow/src/lib.rs) · 5 lines


## 22. `crm-customer-360-composition` — NOW/NEXT / A? + B-CONDITIONAL

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-360-composition/src/lib.rs) · 1327 lines

- **Public declarations (9):** `from_json@L133`, `validate@L140`, `affects_party@L189`, `resource_id@L195`, `customer_360_projection_registry@L437`, `new@L460`, `run_batch@L466`, `rebuild@L476`, `runner@L482`
- **Internal/trait/helpers (32):** `storage_key@L108`, `active_privacy_lifecycle@L222`, `project@L275`, `party_contribution@L487`, `party_privacy_contribution@L524`, `account_contribution@L541`, `contact_point_contribution@L620`, `party_relationship_contribution@L664`, `contribution_document@L731`, `validate_party_privacy_action@L751`, `require_canonical_json_field@L806`, `canonical_json_string_field@L815`, `validate_contract@L845`, `validate_snapshot_identity@L869`, `resource_version@L883`, `contact_point_kind@L896`, `contact_point_status@L909`, `verification_status@L919`, `decode@L929`, `configuration_invalid@L936`, `event_invalid@L946`, `contribution_invalid@L956`, `account_update_document_replaces_the_complete_canonical_root_set@L976`, `contact_point_verification_snapshot_is_projected_as_owner_state_not_consent@L1025`, `relationship_contribution_targets_both_canonical_party_roots@L1076`, `active_party_documents_and_legacy_json_default_to_active_lifecycle@L1124`, `privacy_actions_replace_party_root_membership_with_non_personal_tombstones@L1180`, `privacy_action_rejects_noncanonical_fields_and_invalid_version_lineage@L1205`, `registry_subscribes_to_owner_snapshots_and_party_privacy_actions@L1223`, `privacy_delivery@L1237`, `privacy_delivery_with_bytes@L1249`, `delivery@L1286`
- **Public types:** `Customer360ContributionKind@L100`, `Customer360ContributionDocument@L120`, `Customer360ContributionSnapshot@L206`, `PartyContributionSnapshot@L215`, `AccountContributionSnapshot@L228`, `AccountPartyAssociationContribution@L236`, `ContactPointContributionSnapshot@L243`, `PartyRelationshipContributionSnapshot@L259`, `Customer360ProjectionWorker@L455`
- **Detected tests (7):** `account_update_document_replaces_the_complete_canonical_root_set@L976`, `contact_point_verification_snapshot_is_projected_as_owner_state_not_consent@L1025`, `relationship_contribution_targets_both_canonical_party_roots@L1076`, `active_party_documents_and_legacy_json_default_to_active_lifecycle@L1124`, `privacy_actions_replace_party_root_membership_with_non_personal_tombstones@L1180`, `privacy_action_rejects_noncanonical_fields_and_invalid_version_lineage@L1205`, `registry_subscribes_to_owner_snapshots_and_party_privacy_actions@L1223`
- **Error/guard literals:** `CUSTOMER_360_CONFIGURATION_INVALID`, `CUSTOMER_360_SOURCE_EVENT_INVALID`, `CUSTOMER_360_CONTRIBUTION_INVALID`
- **Rust crate references:** `crm_core_data`, `crm_core_events`, `crm_module_sdk`, `crm_projection_runtime`, `crm_proto_contracts`

## 23. `crm-customer-accounts-capability-adapter` — NOW / A? + B-NOW-if-gap + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-accounts-capability-adapter/src/lib.rs) · 132 lines

- **Public declarations (2):** `capability_definitions@L36`, `capability_definition@L43`
- **Internal/trait/helpers (4):** `configured@L78`, `configuration_error@L88`, `publishes_exact_create_and_update_coordinates_as_personal_mutations@L97`, `rejects_unknown_account_mutation_coordinate@L127`
- **Re-exports:** `pub use planner::*;`
- **Detected tests (2):** `publishes_exact_create_and_update_coordinates_as_personal_mutations@L97`, `rejects_unknown_account_mutation_coordinate@L127`
- **Error/guard literals:** `CUSTOMER_ACCOUNTS_CAPABILITY_UNSUPPORTED`, `CUSTOMER_ACCOUNTS_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_module_sdk`

### [`src/planner.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-accounts-capability-adapter/src/planner.rs) · 465 lines

- **Public declarations (6):** `account_to_wire@L244`, `persisted_contract@L279`, `persisted_payload@L290`, `account_from_snapshot@L298`, `referenced_party_ids_from_create@L314`, `referenced_party_ids_from_update@L326`
- **Internal/trait/helpers (14):** `target@L25`, `plan@L69`, `plan_create@L84`, `plan_update@L149`, `mutation_plan@L215`, `account_id_from_ref@L338`, `associations_from_wire@L347`, `party_references_from_wire@L367`, `account_party_role_from_wire@L384`, `account_status_from_wire@L395`, `ensure_definition@L406`, `invalid_plan@L419`, `unsupported_capability@L428`, `wire_mapping_preserves_roles_status_and_version_metadata@L442`
- **Public types:** `CustomerAccountCapabilityPlanner@L22`
- **Detected tests (1):** `wire_mapping_preserves_roles_status_and_version_metadata@L442`
- **Error/guard literals:** `CUSTOMER_ACCOUNTS_PERSISTED_ACCOUNT_IDENTITY_INVALID`, `CUSTOMER_ACCOUNTS_CAPABILITY_PLAN_INVALID`, `CUSTOMER_ACCOUNTS_CAPABILITY_UNSUPPORTED`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_accounts`, `crm_module_sdk`, `crm_proto_contracts`

## 24. `crm-customer-accounts-privacy-scope-adapter` — END / B-END + D? adapter

### [`src/contract.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-accounts-privacy-scope-adapter/src/contract.rs) · 96 lines

- **Public declarations (8):** `customer_accounts_privacy_scope_definition@L26`, `validate_definition@L46`, `module_id@L57`, `capability_id@L61`, `capability_version@L65`, `schema_id@L69`, `schema_version@L73`, `output_descriptor_hash@L77`
- **Internal/trait/helpers (1):** `payload_contract@L81`
- **Error/guard literals:** `CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_DEFINITION_MISMATCH`
- **Rust crate references:** `crm_capability_runtime`, `crm_customer_accounts`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/digest.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-accounts-privacy-scope-adapter/src/digest.rs) · 44 lines

- **Public declarations (2):** `encode_hex@L3`, `decode_hex@L13`
- **Internal/trait/helpers (2):** `nibble@L30`, `cursor_invalid@L38`
- **Error/guard literals:** `CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_CURSOR_INVALID`
- **Rust crate references:** `crm_module_sdk`

### [`src/errors.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-accounts-privacy-scope-adapter/src/errors.rs) · 166 lines

- **Public declarations (10):** `configured@L6`, `map_request_contract_error@L18`, `map_common_lineage_error@L37`, `map_canonical_party_claim_error@L91`, `lineage_invalid@L117`, `stored_state_invalid@L131`, `row_decode_error@L141`, `database_unavailable@L145`, `invalid_contract@L155`, `invalid_contract_with_reference@L159`
- **Error/guard literals:** `CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_CONFIGURATION_INVALID`, `CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_REQUEST_BINDING_MISMATCH`, `CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_INPUT_CONTRACT_MISMATCH`, `CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_INPUT_HASH_MISMATCH`, `CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_TENANT_MISMATCH`, `CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_CASE_ID_INVALID`, `CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_PARTY_INVALID`, `CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_GENERATION_INVALID`, `CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_REGISTRY_INVALID`, `CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_REGISTRY_UNAVAILABLE`, `CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_REGISTRY_MISMATCH`, `CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_PURPOSE_INVALID`, `CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_REQUEST_TIME_INVALID`, `CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_PAGE_SIZE_INVALID`, `CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_LINEAGE_INVALID`, `CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_STORED_STATE_INVALID`, `CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_UNAVAILABLE`
- **Rust crate references:** `crm_customer_privacy_owner_scope_support`, `crm_module_sdk`

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-accounts-privacy-scope-adapter/src/lib.rs) · 33 lines

- **Re-exports:** `pub use contract::{`, `pub use owner_action::{`, `pub use postgres::CustomerAccountsPrivacyScopeQueryAdapter;`

### [`src/owner_action.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-accounts-privacy-scope-adapter/src/owner_action.rs) · 160 lines

- **Public declarations (2):** `customer_accounts_privacy_action_definition@L20`, `customer_accounts_privacy_action_planner@L24`
- **Internal/trait/helpers (12):** `owner_module_id@L29`, `capability_id@L33`, `supports_resource_type@L37`, `anonymize@L41`, `deletion_tombstone@L49`, `minimize_account@L58`, `minimized_account_name@L82`, `hex_prefix@L97`, `account@L113`, `publishes_the_frozen_owner_action_coordinate@L127`, `minimized_name_is_deterministic_and_non_identifying@L142`, `delete_changes_lifecycle_even_when_name_is_already_minimized@L151`
- **Public types:** `CustomerAccountsPrivacyActionPlanner@L14`, `CustomerAccountsPrivacyActionPolicy@L18`
- **Detected tests (3):** `publishes_the_frozen_owner_action_coordinate@L127`, `minimized_name_is_deterministic_and_non_identifying@L142`, `delete_changes_lifecycle_even_when_name_is_already_minimized@L151`
- **Rust crate references:** `crm_capability_runtime`, `crm_customer_accounts`, `crm_customer_accounts_capability_adapter`, `crm_customer_privacy_owner_scope_support`, `crm_module_sdk`

### [`src/postgres.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-accounts-privacy-scope-adapter/src/postgres.rs) · 297 lines

- **Public declarations (1):** `new@L42`
- **Internal/trait/helpers (7):** `fmt@L33`, `execute_query@L46`, `execute@L82`, `read_account_page@L97`, `strict_account_resource@L215`, `configured@L289`, `_definition_smoke@L294`
- **Public types:** `CustomerAccountsPrivacyScopeQueryAdapter@L28`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_accounts`, `crm_customer_accounts_capability_adapter`, `crm_customer_privacy_owner_scope_support`, `crm_module_sdk`, `crm_query_runtime`

### [`src/request.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-accounts-privacy-scope-adapter/src/request.rs) · 181 lines

- **Public declarations (3):** `validate_request_contract@L30`, `validate_wire_request@L35`, `encode_cursor@L91`
- **Internal/trait/helpers (3):** `decode_cursor@L114`, `cursor_digest@L148`, `cursor_invalid@L175`
- **Public types:** `ValidatedRequest@L21`
- **Error/guard literals:** `CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_REQUEST_INVALID`, `CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_LINEAGE_INVALID`, `CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_CURSOR_INVALID`
- **Rust crate references:** `crm_customer_privacy_owner_scope_support`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`

### [`src/response.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-accounts-privacy-scope-adapter/src/response.rs) · 134 lines

- **Public declarations (2):** `build_response@L23`, `typed_output@L92`
- **Internal/trait/helpers (1):** `page_digest@L108`
- **Public types:** `VerifiedAccountResource@L18`
- **Rust crate references:** `crm_customer_accounts`, `crm_customer_accounts_capability_adapter`, `crm_customer_privacy_owner_scope_support`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/tests.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-accounts-privacy-scope-adapter/src/tests.rs) · 232 lines

- **Internal/trait/helpers (10):** `context@L25`, `wire_request@L39`, `query_request@L64`, `assert_invalid_argument@L88`, `publishes_exact_contract_only_coordinate@L95`, `request_contract_preserves_customer_accounts_integrity_errors@L109`, `cursor_round_trip_is_bound_to_lineage_and_page_size@L141`, `common_lineage_failures_keep_customer_accounts_codes@L165`, `response_is_reference_only_deterministic_and_supports_sparse_progress@L188`, `malformed_wire_request_is_rejected_before_owner_reads@L228`
- **Detected tests (6):** `publishes_exact_contract_only_coordinate@L95`, `request_contract_preserves_customer_accounts_integrity_errors@L109`, `cursor_round_trip_is_bound_to_lineage_and_page_size@L141`, `common_lineage_failures_keep_customer_accounts_codes@L165`, `response_is_reference_only_deterministic_and_supports_sparse_progress@L188`, `malformed_wire_request_is_rejected_before_owner_reads@L228`
- **Error/guard literals:** `PRIVACY_ERASURE_SCOPE`, `CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_REQUEST_BINDING_MISMATCH`, `CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_INPUT_HASH_MISMATCH`, `CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_INPUT_CONTRACT_MISMATCH`, `CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_CURSOR_INVALID`, `CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_TENANT_MISMATCH`, `CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_PAGE_SIZE_INVALID`, `CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_REQUEST_INVALID`
- **Rust crate references:** `crm_customer_privacy`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`, `crm_customer_accounts`

### [`tests/postgres_scope.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-accounts-privacy-scope-adapter/tests/postgres_scope.rs) · 406 lines

- **Internal/trait/helpers (3):** `customer_accounts_scope_is_bounded_strict_tenant_bound_and_side_effect_free@L27`, `assert_response_omits_account_state@L382`, `assert_no_query_side_writes@L393`
- **Detected tests (1):** `customer_accounts_scope_is_bounded_strict_tenant_bound_and_side_effect_free@L27`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`, `CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_CURSOR_INVALID`, `CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_LINEAGE_INVALID`, `CUSTOMER_ACCOUNTS_PRIVACY_SCOPE_STORED_STATE_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_accounts_capability_adapter`, `crm_customer_accounts_privacy_scope_adapter`, `crm_parties_capability_adapter`, `crm_proto_contracts`, `crm_query_runtime`, `crm_customer_accounts`

### [`tests/postgres_scope/support.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-accounts-privacy-scope-adapter/tests/postgres_scope/support.rs) · 401 lines

- **Public declarations (6):** `create_party@L42`, `create_account@L73`, `scope_request@L150`, `insert_canonical_redirect@L208`, `corrupt_account_metadata@L301`, `write_surface_counts@L361`
- **Internal/trait/helpers (1):** `capability_request@L114`
- **Public types:** `WriteSurfaceCounts@L350`
- **Error/guard literals:** `PRIVACY_ERASURE_SCOPE`
- **Rust crate references:** `crm_capability_runtime`, `crm_customer_accounts_capability_adapter`, `crm_customer_accounts_privacy_scope_adapter`, `crm_customer_privacy`, `crm_identity_resolution_capability_adapter`, `crm_module_sdk`, `crm_parties_capability_adapter`, `crm_proto_contracts`, `crm_query_runtime`, `crm_parties`, `crm_customer_accounts`

## 25. `crm-customer-accounts-query-adapter` — NOW / A? + B-NOW-if-gap + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-accounts-query-adapter/src/lib.rs) · 650 lines

- **Public declarations (3):** `new@L58`, `query_capability_definitions@L78`, `query_capability_definition@L85`
- **Internal/trait/helpers (28):** `fmt@L46`, `validate@L118`, `execute@L149`, `execute_get@L166`, `execute_list@L201`, `collect_accounts@L232`, `has_more_visible_account@L284`, `validate_list@L323`, `account_matches_status@L361`, `account_to_wire_with_visibility@L370`, `resolve_page_size@L387`, `cursor_binding@L396`, `decode_after@L415`, `encode_next@L444`, `account_filter_hash@L465`, `decode_input@L470`, `validate_record_id@L501`, `account_record_type@L507`, `enforce_scan_limit@L511`, `resource_not_found@L519`, `unsupported_query@L528`, `cursor_error@L537`, `scan_limit_error@L547`, `config_error@L556`, `account@L574`, `publishes_get_and_list_as_personal_read_only_queries@L588`, `field_visibility_redacts_relationship_data_without_hiding_resource_identity@L606`, `status_filter_is_exact_and_cursor_bound@L624`
- **Public types:** `AccountQueryAdapter@L38`
- **Detected tests (3):** `publishes_get_and_list_as_personal_read_only_queries@L588`, `field_visibility_redacts_relationship_data_without_hiding_resource_identity@L606`, `status_filter_is_exact_and_cursor_bound@L624`
- **Error/guard literals:** `CUSTOMER_ACCOUNTS_QUERY_CURSOR_INVALID`, `CUSTOMER_ACCOUNTS_QUERY_INPUT_CONTRACT_MISMATCH`, `CUSTOMER_ACCOUNTS_QUERY_INPUT_PROTOBUF_INVALID`, `QUERY_RESOURCE_NOT_FOUND`, `CUSTOMER_ACCOUNTS_QUERY_CAPABILITY_UNSUPPORTED`, `CUSTOMER_ACCOUNTS_QUERY_VISIBILITY_SCAN_LIMIT_EXCEEDED`, `CUSTOMER_ACCOUNTS_QUERY_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_accounts`, `crm_customer_accounts_capability_adapter`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`

### [`src/production_contribution.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-accounts-query-adapter/src/production_contribution.rs) · 194 lines

- **Public declarations (2):** `mutation_capability_definitions@L90`, `build_contribution@L96`
- **Internal/trait/helpers (8):** `fmt@L33`, `new@L42`, `validate@L48`, `customer_accounts_cursor@L146`, `production_composition_error@L158`, `reference_unavailable@L168`, `configuration_error@L177`, `unsupported_capability@L186`
- **Public types:** `CustomerAccountsProductionDependencies@L80`
- **Error/guard literals:** `CUSTOMER_ACCOUNTS_CURSOR_CONFIGURATION_INVALID`, `CUSTOMER_ACCOUNTS_PRODUCTION_COMPOSITION_INVALID`, `CUSTOMER_ACCOUNTS_PARTY_REFERENCE_UNAVAILABLE`, `CUSTOMER_ACCOUNTS_REFERENCE_VALIDATION_CONFIGURATION_INVALID`, `CUSTOMER_ACCOUNTS_COMPOSITION_CAPABILITY_UNSUPPORTED`
- **Rust crate references:** `crm_application_composition`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_accounts_capability_adapter`, `crm_module_sdk`, `crm_party_reference_composition`, `crm_query_runtime`

### [`src/root.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-accounts-query-adapter/src/root.rs) · 11 lines

- **Re-exports:** `pub use production_contribution::{`, `pub use query::*;`

## 26. `crm-customer-data-operations-capability-adapter` — NEXT/LATER / B-CONDITIONAL + D? adapter

### [`src/export_boundary_planner.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-capability-adapter/src/export_boundary_planner.rs) · 199 lines

- **Public declarations (7):** `harden_party_export_start_plan@L29`, `export_selection_boundary_persisted_contract@L70`, `export_selection_boundary_persisted_payload@L81`, `export_selection_boundary_record_ref@L91`, `export_selection_progress_persisted_contract@L101`, `export_selection_progress_persisted_payload@L112`, `export_selection_progress_record_ref@L122`
- **Internal/trait/helpers (2):** `specification@L140`, `publishes_strict_personal_boundary_and_progress_persistence_contracts@L149`
- **Detected tests (1):** `publishes_strict_personal_boundary_and_progress_persistence_contracts@L149`
- **Error/guard literals:** `CUSTOMER_DATA_EXPORT_SELECTION_BOUNDARY_JOB_MISSING`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_data_operations`, `crm_module_sdk`

### [`src/export_execution_outcome_support.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-capability-adapter/src/export_execution_outcome_support.rs) · 90 lines

- **Public declarations (4):** `export_execution_outcome_persisted_contract@L15`, `export_execution_outcome_persisted_payload@L26`, `export_execution_outcome_record_ref@L36`, `export_execution_outcome_from_snapshot@L46`
- **Internal/trait/helpers (1):** `publishes_strict_personal_execution_outcome_contract@L70`
- **Detected tests (1):** `publishes_strict_personal_execution_outcome_contract@L70`
- **Error/guard literals:** `CUSTOMER_DATA_EXPORT_EXECUTION_OUTCOME_REFERENCE_INVALID`
- **Rust crate references:** `crm_capability_plan_support`, `crm_customer_data_operations`, `crm_module_sdk`

### [`src/export_execution_planner.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-capability-adapter/src/export_execution_planner.rs) · 627 lines

- **Public declarations (2):** `internal_export_execution_capability_definitions@L67`, `internal_export_execution_capability_definition@L75`
- **Internal/trait/helpers (21):** `target@L120`, `plan@L157`, `plan_stage_execution@L179`, `plan_commit_outcome@L275`, `plan_complete_execution@L371`, `export_job_id_from_ref@L463`, `export_job_record_ref@L473`, `ensure_job_identity@L481`, `exclusion_reason@L495`, `bytes_to_sha256_hex@L513`, `decode_request@L528`, `ensure_definition@L535`, `configured@L551`, `invalid_plan@L563`, `stored_state_invalid@L572`, `execution_state_invalid@L576`, `execution_result_required@L585`, `invalid_utf8_row@L592`, `completion_evidence_required@L599`, `unsupported_internal_capability@L606`, `private_execution_capabilities_remain_outside_public_mutation_catalog@L621`
- **Public types:** `PartyExportExecutionOutcomePlanner@L65`
- **Detected tests (1):** `private_execution_capabilities_remain_outside_public_mutation_catalog@L621`
- **Error/guard literals:** `CUSTOMER_DATA_EXPORT_EXECUTION_CONFIGURATION_INVALID`, `CUSTOMER_DATA_EXPORT_EXECUTION_PLAN_INVALID`, `CUSTOMER_DATA_EXPORT_EXECUTION_STORED_STATE_INVALID`, `CUSTOMER_DATA_EXPORT_EXECUTION_STATE_INVALID`, `CUSTOMER_DATA_EXPORT_EXECUTION_INTERNAL_CAPABILITY_UNSUPPORTED`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_data_operations`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/export_execution_stage_support.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-capability-adapter/src/export_execution_stage_support.rs) · 99 lines

- **Public declarations (5):** `export_execution_stage_persisted_contract@L15`, `export_execution_stage_persisted_payload@L26`, `export_execution_stage_record_ref@L36`, `export_execution_stage_record_ref_for_job_position@L42`, `export_execution_stage_from_snapshot@L54`
- **Internal/trait/helpers (1):** `publishes_strict_personal_execution_stage_contract@L77`
- **Detected tests (1):** `publishes_strict_personal_execution_stage_contract@L77`
- **Error/guard literals:** `CUSTOMER_DATA_EXPORT_EXECUTION_STAGE_REFERENCE_INVALID`
- **Rust crate references:** `crm_capability_plan_support`, `crm_customer_data_operations`, `crm_module_sdk`

### [`src/export_planner.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-capability-adapter/src/export_planner.rs) · 702 lines

- **Public declarations (6):** `export_capability_definitions@L66`, `export_capability_definition@L73`, `export_job_persisted_contract@L360`, `export_job_persisted_payload@L371`, `export_job_from_snapshot@L381`, `export_job_to_wire@L397`
- **Internal/trait/helpers (21):** `target@L116`, `plan@L155`, `plan_create@L171`, `plan_start@L224`, `plan_cancel@L278`, `single_mutation_plan@L332`, `specification_from_wire@L454`, `specification_to_wire@L533`, `status_to_wire@L566`, `export_job_id_from_ref@L578`, `export_job_record_ref@L588`, `ensure_snapshot_identity@L596`, `decode_request@L608`, `ensure_definition@L615`, `sha256_hex_to_bytes@L628`, `configured@L638`, `invalid_plan@L648`, `unsupported_capability@L657`, `configuration_error@L666`, `publishes_exact_export_mutation_coordinates@L675`, `rejects_unknown_export_mutation_coordinate@L697`
- **Public types:** `PartyExportCapabilityPlanner@L64`
- **Detected tests (2):** `publishes_exact_export_mutation_coordinates@L675`, `rejects_unknown_export_mutation_coordinate@L697`
- **Error/guard literals:** `CUSTOMER_DATA_EXPORT_PERSISTED_JOB_IDENTITY_INVALID`, `CUSTOMER_DATA_EXPORT_CONFIGURATION_INVALID`, `CUSTOMER_DATA_EXPORT_CAPABILITY_PLAN_INVALID`, `CUSTOMER_DATA_EXPORT_CAPABILITY_UNSUPPORTED`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_data_operations`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/export_selection_planner.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-capability-adapter/src/export_selection_planner.rs) · 553 lines

- **Public declarations (5):** `internal_export_selection_capability_definitions@L59`, `internal_export_selection_capability_definition@L67`, `export_selection_item_persisted_contract@L363`, `export_selection_item_persisted_payload@L374`, `export_selection_item_record_ref@L384`
- **Internal/trait/helpers (15):** `target@L105`, `plan@L143`, `plan_commit_selection_page@L162`, `plan_finalize_selection@L281`, `export_selection_progress_from_snapshot@L394`, `source_continuation_from_wire@L414`, `export_job_id_from_ref@L436`, `export_job_record_ref@L446`, `bytes_to_sha256_hex@L454`, `ensure_definition@L469`, `decode_request@L484`, `configured@L515`, `invalid_plan@L527`, `stored_state_invalid@L536`, `unsupported_internal_capability@L545`
- **Public types:** `PartyExportSelectionOutcomePlanner@L57`
- **Error/guard literals:** `CUSTOMER_DATA_EXPORT_INTERNAL_INPUT_CONTRACT_MISMATCH`, `CUSTOMER_DATA_EXPORT_INTERNAL_INPUT_PROTOBUF_INVALID`, `CUSTOMER_DATA_EXPORT_INTERNAL_CONFIGURATION_INVALID`, `CUSTOMER_DATA_EXPORT_INTERNAL_PLAN_INVALID`, `CUSTOMER_DATA_EXPORT_INTERNAL_STORED_STATE_INVALID`, `CUSTOMER_DATA_EXPORT_INTERNAL_CAPABILITY_UNSUPPORTED`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_data_operations`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-capability-adapter/src/lib.rs) · 305 lines

- **Public declarations (3):** `capability_definitions@L128`, `import_capability_definitions@L145`, `capability_definition@L152`
- **Internal/trait/helpers (7):** `target@L214`, `plan@L228`, `configured@L245`, `configuration_error@L255`, `routing_error@L259`, `publishes_exact_customer_data_mutation_coordinates@L273`, `import_definition_rejects_unknown_import_coordinate@L300`
- **Public types:** `CustomerDataOperationsCapabilityPlanner@L211`
- **Re-exports:** `pub use export_boundary_planner::*;`, `pub use export_execution_outcome_support::*;`, `pub use export_execution_planner::*;`, `pub use export_execution_stage_support::*;`, `pub use export_planner::*;`, `pub use export_selection_planner::*;`, `pub use planner::*;`
- **Detected tests (2):** `publishes_exact_customer_data_mutation_coordinates@L273`, `import_definition_rejects_unknown_import_coordinate@L300`
- **Error/guard literals:** `CUSTOMER_DATA_IMPORT_CAPABILITY_UNSUPPORTED`, `CUSTOMER_DATA_IMPORT_CONFIGURATION_INVALID`, `CUSTOMER_DATA_OPERATION_CAPABILITY_UNSUPPORTED`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_module_sdk`

### [`src/planner.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-capability-adapter/src/planner.rs) · 1192 lines

- **Public declarations (7):** `import_job_persisted_contract@L695`, `import_row_persisted_contract@L706`, `import_job_persisted_payload@L717`, `import_row_persisted_payload@L727`, `import_job_from_snapshot@L737`, `job_to_wire@L753`, `import_row_to_wire@L794`
- **Internal/trait/helpers (30):** `target@L52`, `plan@L137`, `plan_create@L161`, `plan_validate_rows@L223`, `plan_finalize_validation@L486`, `plan_start_execution@L547`, `plan_cancel@L607`, `single_mutation_plan@L667`, `source_from_wire@L847`, `parser_profile_from_wire@L862`, `source_to_wire@L938`, `parser_profile_to_wire@L948`, `mapping_from_wire@L960`, `mapping_to_wire@L978`, `partial_execution_policy_from_wire@L988`, `import_job_status_to_wire@L1001`, `import_row_status_to_wire@L1011`, `prepared_party_to_wire@L1021`, `target_party_id_for_row@L1034`, `party_kind_for_row@L1056`, `required_mapped_value@L1082`, `optional_mapped_value@L1103`, `import_job_id_from_ref@L1112`, `import_job_record_ref@L1122`, `import_row_record_ref@L1130`, `sha256_bytes_to_hex@L1138`, `sha256_hex_to_bytes@L1152`, `ensure_definition@L1162`, `invalid_plan@L1175`, `unsupported_capability@L1184`
- **Public types:** `CustomerDataImportCapabilityPlanner@L49`
- **Error/guard literals:** `CUSTOMER_DATA_IMPORT_JOB_NOT_CREATED`, `EXTERNAL_ROW_KEY_MISSING`, `EXTERNAL_ROW_KEY_INVALID`, `SOURCE_EXTERNAL_ID_MISSING`, `SOURCE_EXTERNAL_ID_INVALID`, `DISPLAY_NAME_MISSING`, `TARGET_PARTY_DUPLICATE_IN_BATCH`, `CUSTOMER_DATA_IMPORT_PERSISTED_JOB_IDENTITY_INVALID`, `TARGET_PARTY_ID_MISSING`, `TARGET_PARTY_ID_INVALID`, `PARTY_KIND_MISSING`, `PARTY_KIND_INVALID`, `CUSTOMER_DATA_IMPORT_CAPABILITY_PLAN_INVALID`, `CUSTOMER_DATA_IMPORT_CAPABILITY_UNSUPPORTED`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_data_operations`, `crm_module_sdk`, `crm_proto_contracts`

### [`tests/contract_surface.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-capability-adapter/tests/contract_surface.rs) · 132 lines

- **Internal/trait/helpers (6):** `source@L15`, `mapping@L26`, `publishes_exact_governed_mutation_surface@L38`, `job_and_row_persistence_contracts_are_distinct_and_bounded@L76`, `wire_job_preserves_parser_profile_and_source_system_identity@L89`, `wire_row_keeps_external_identifier_as_digest_not_party_identity@L114`
- **Detected tests (4):** `publishes_exact_governed_mutation_surface@L38`, `job_and_row_persistence_contracts_are_distinct_and_bounded@L76`, `wire_job_preserves_parser_profile_and_source_system_identity@L89`, `wire_row_keeps_external_identifier_as_digest_not_party_identity@L114`
- **Rust crate references:** `crm_capability_runtime`, `crm_customer_data_operations`, `crm_customer_data_operations_capability_adapter`

### [`tests/export_selection_private_surface.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-capability-adapter/tests/export_selection_private_surface.rs) · 33 lines

- **Internal/trait/helpers (1):** `worker_selection_capabilities_are_versioned_but_absent_from_public_mutation_catalog@L9`
- **Detected tests (1):** `worker_selection_capabilities_are_versioned_but_absent_from_public_mutation_catalog@L9`
- **Rust crate references:** `crm_customer_data_operations_capability_adapter`

## 27. `crm-customer-data-operations-execution-composition` — NEXT/LATER / B-CONDITIONAL + D? adapter

### [`src/export_execution_reader.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-execution-composition/src/export_execution_reader.rs) · 212 lines

- **Public declarations (4):** `new@L20`, `load_stage@L24`, `load_outcome@L50`, `load_outcomes@L83`
- **Internal/trait/helpers (6):** `module_id@L108`, `execution_evidence_missing@L112`, `execution_evidence_invalid@L121`, `configuration_error@L130`, `exact_stage_and_outcome_persisted_payloads_are_personal@L152`, `outcome_lookup_identity_is_kind_independent_and_position_scoped@L180`
- **Public types:** `PostgresPartyExportExecutionReader@L15`
- **Detected tests (2):** `exact_stage_and_outcome_persisted_payloads_are_personal@L152`, `outcome_lookup_identity_is_kind_independent_and_position_scoped@L180`
- **Error/guard literals:** `CUSTOMER_DATA_EXPORT_EXECUTION_EVIDENCE_MISSING`, `CUSTOMER_DATA_EXPORT_EXECUTION_EVIDENCE_INVALID`, `CUSTOMER_DATA_EXPORT_EXECUTION_READER_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_core_data`, `crm_customer_data_operations`, `crm_customer_data_operations_capability_adapter`, `crm_module_sdk`

### [`src/export_execution_sink.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-execution-composition/src/export_execution_sink.rs) · 404 lines

- **Public declarations (5):** `new@L51`, `stage@L55`, `commit_emitted@L95`, `commit_excluded@L129`, `complete@L159`
- **Internal/trait/helpers (11):** `fmt@L41`, `authorize_plan_and_execute@L199`, `prepare_internal_request@L244`, `internal_request@L260`, `exclusion_reason_to_wire@L286`, `decode_sha256@L302`, `batch_error_to_sdk@L314`, `execution_state_invalid@L360`, `configuration_error@L369`, `hex@L379`, `private_execution_capabilities_remain_outside_public_mutation_catalog@L394`
- **Public types:** `PartyExportEmittedOutcome@L26`, `PostgresPartyExportExecutionSink@L35`
- **Detected tests (1):** `private_execution_capabilities_remain_outside_public_mutation_catalog@L394`
- **Error/guard literals:** `CUSTOMER_DATA_EXPORT_EXECUTION_OUTCOME_PERMISSION_DENIED`, `CUSTOMER_DATA_EXPORT_EXECUTION_OUTCOME_CONFLICT`, `CUSTOMER_DATA_EXPORT_EXECUTION_OUTCOME_IDEMPOTENCY_CONFLICT`, `CUSTOMER_DATA_EXPORT_EXECUTION_OUTCOME_IN_PROGRESS`, `CUSTOMER_DATA_EXPORT_EXECUTION_OUTCOME_STORAGE_UNAVAILABLE`, `CUSTOMER_DATA_EXPORT_EXECUTION_OUTCOME_PLAN_INVALID`, `CUSTOMER_DATA_EXPORT_EXECUTION_OUTCOME_REPLAY_INVALID`, `CUSTOMER_DATA_EXPORT_EXECUTION_OUTCOME_STATE_INVALID`, `CUSTOMER_DATA_EXPORT_EXECUTION_OUTCOME_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_capability_adapters`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_data_operations`, `crm_customer_data_operations_capability_adapter`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/export_execution_source.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-execution-composition/src/export_execution_source.rs) · 66 lines

- **Internal/trait/helpers (2):** `get@L44`, `source_result_has_exact_bounded_terminal_categories@L55`
- **Public types:** `PartyExportExecutionSourceKind@L11`, `PartyExportExecutionSourceResult@L17`, `PartyExportExecutionSourceRequest@L30`, `PartyExportExecutionSource@L43`
- **Re-exports:** `pub use import_execution_types::*;`, `pub use import_execution_coordinator::*;`
- **Detected tests (1):** `source_result_has_exact_bounded_terminal_categories@L55`
- **Rust crate references:** `crm_module_sdk`

### [`src/export_execution_worker.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-execution-composition/src/export_execution_worker.rs) · 659 lines

- **Public declarations (3):** `new@L84`, `actor_id@L108`, `run_tenant_cycle@L112`
- **Internal/trait/helpers (14):** `fmt@L66`, `artifact_blueprint@L406`, `stage_from_source@L459`, `worker_context@L537`, `cycle_identity@L572`, `module_id@L580`, `sha256@L584`, `decode_sha256@L588`, `hash_part@L600`, `hex@L605`, `configuration_error@L614`, `worker_state_invalid@L624`, `worker_state_unavailable@L633`, `artifact_identity_is_stable_for_same_job_phase_and_position@L647`
- **Public types:** `ExportExecutionTenantCycle@L43`, `PartyExportExecutionWorker@L52`
- **Detected tests (1):** `artifact_identity_is_stable_for_same_job_phase_and_position@L647`
- **Error/guard literals:** `CUSTOMER_DATA_EXPORT_EXECUTION_WORKER_CONFIGURATION_INVALID`, `CUSTOMER_DATA_EXPORT_EXECUTION_WORKER_STATE_INVALID`, `CUSTOMER_DATA_EXPORT_EXECUTION_WORKER_STATE_UNAVAILABLE`
- **Rust crate references:** `crm_core_data`, `crm_core_files`, `crm_customer_data_operations`, `crm_customer_data_operations_capability_adapter`, `crm_module_sdk`

### [`src/export_selection_reader.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-execution-composition/src/export_selection_reader.rs) · 224 lines

- **Public declarations (4):** `new@L33`, `load_evidence@L37`, `load_manifest@L102`, `prove_finalization@L156`
- **Internal/trait/helpers (5):** `module_id@L177`, `selection_evidence_missing@L181`, `selection_evidence_invalid@L190`, `configuration_error@L199`, `deterministic_progress_and_manifest_ids_are_job_bound@L215`
- **Public types:** `PartyExportSelectionEvidence@L22`, `PostgresPartyExportSelectionReader@L28`
- **Detected tests (1):** `deterministic_progress_and_manifest_ids_are_job_bound@L215`
- **Error/guard literals:** `CUSTOMER_DATA_EXPORT_SELECTION_EVIDENCE_MISSING`, `CUSTOMER_DATA_EXPORT_SELECTION_EVIDENCE_INVALID`, `CUSTOMER_DATA_EXPORT_SELECTION_READER_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_core_data`, `crm_customer_data_operations`, `crm_customer_data_operations_capability_adapter`, `crm_module_sdk`

### [`src/export_selection_sink.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-execution-composition/src/export_selection_sink.rs) · 298 lines

- **Public declarations (3):** `new@L39`, `commit_page@L43`, `finalize@L72`
- **Internal/trait/helpers (11):** `fmt@L29`, `authorize_plan_and_execute@L99`, `prepare_internal_request@L148`, `internal_request@L164`, `decode_sha256@L190`, `batch_error_to_sdk@L202`, `selection_state_invalid@L248`, `configuration_error@L257`, `hex@L267`, `private_selection_capabilities_remain_outside_public_mutation_catalog@L282`, `strict_sha256_decoder_accepts_only_exact_hex_digest@L292`
- **Public types:** `PostgresPartyExportSelectionSink@L23`
- **Detected tests (2):** `private_selection_capabilities_remain_outside_public_mutation_catalog@L282`, `strict_sha256_decoder_accepts_only_exact_hex_digest@L292`
- **Error/guard literals:** `CUSTOMER_DATA_EXPORT_SELECTION_OUTCOME_PERMISSION_DENIED`, `CUSTOMER_DATA_EXPORT_SELECTION_OUTCOME_CONFLICT`, `CUSTOMER_DATA_EXPORT_SELECTION_OUTCOME_IDEMPOTENCY_CONFLICT`, `CUSTOMER_DATA_EXPORT_SELECTION_OUTCOME_IN_PROGRESS`, `CUSTOMER_DATA_EXPORT_SELECTION_OUTCOME_STORAGE_UNAVAILABLE`, `CUSTOMER_DATA_EXPORT_SELECTION_OUTCOME_PLAN_INVALID`, `CUSTOMER_DATA_EXPORT_SELECTION_OUTCOME_REPLAY_INVALID`, `CUSTOMER_DATA_EXPORT_SELECTION_OUTCOME_STATE_INVALID`, `CUSTOMER_DATA_EXPORT_SELECTION_OUTCOME_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_capability_adapters`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_data_operations`, `crm_customer_data_operations_capability_adapter`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/export_selection_worker.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-execution-composition/src/export_selection_worker.rs) · 452 lines

- **Public declarations (5):** `new@L148`, `try_with_page_size@L166`, `actor_id@L215`, `execution_worker@L219`, `run_tenant_cycle@L223`
- **Internal/trait/helpers (8):** `list_page@L80`, `capability_authorizer@L93`, `get@L102`, `fmt@L132`, `worker_context@L382`, `configuration_error@L414`, `worker_state_unavailable@L424`, `worker_context_uses_private_export_selection_identity@L438`
- **Public types:** `PartyExportSelectionSourceKind@L43`, `PartyExportSelectionSourceContinuation@L49`, `PartyExportSelectionSourceRequest@L55`, `PartyExportSelectionSourceCandidate@L67`, `PartyExportSelectionSourcePage@L73`, `PartyExportSelectionSource@L79`, `PartyExportSelectionExecutionSource@L90`, `ExportSelectionTenantCycle@L111`, `PartyExportSelectionWorker@L119`
- **Detected tests (1):** `worker_context_uses_private_export_selection_identity@L438`
- **Error/guard literals:** `CUSTOMER_DATA_EXPORT_SELECTION_WORKER_CONFIGURATION_INVALID`, `CUSTOMER_DATA_EXPORT_SELECTION_WORKER_STATE_UNAVAILABLE`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_data_operations`, `crm_customer_data_operations_capability_adapter`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/import_execution_coordinator.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-execution-composition/src/import_execution_coordinator.rs) · 231 lines

- **Public declarations (4):** `new@L40`, `execute_next@L50`, `party_create_invocation@L128`, `target_context@L171`
- **Internal/trait/helpers (6):** `fmt@L30`, `validate_target_outcome@L186`, `configured_capability_id@L205`, `configured_capability_version@L209`, `configuration_error@L213`, `execution_error@L223`
- **Public types:** `PartyImportExecutionCoordinator@L24`
- **Error/guard literals:** `CUSTOMER_DATA_IMPORT_EXECUTION_PREPARED_PARTY_MISSING`, `CUSTOMER_DATA_IMPORT_EXECUTION_PENDING_ROW`, `CUSTOMER_DATA_IMPORT_EXECUTION_SUCCEEDED_ROW_AFTER_CHECKPOINT`, `CUSTOMER_DATA_IMPORT_EXECUTION_ROW_NOT_EXECUTABLE`, `CUSTOMER_DATA_IMPORT_EXECUTION_TARGET_RESULT_MISMATCH`, `CUSTOMER_DATA_IMPORT_EXECUTION_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_customer_data_operations`, `crm_module_sdk`, `crm_parties_capability_adapter`, `crm_proto_contracts`

### [`src/import_execution_types.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-execution-composition/src/import_execution_types.rs) · 143 lines

- **Public declarations (5):** `try_new@L15`, `job@L43`, `rows@L47`, `next_row@L51`, `validate_partial_execution_policy@L119`
- **Internal/trait/helpers (5):** `skip_invalid@L73`, `record_success@L80`, `record_retryable_failure@L88`, `complete@L96`, `execution_error@L136`
- **Public types:** `ImportExecutionSnapshot@L8`, `ImportExecutionOutcomeSink@L72`, `ExecutionCycleOutcome@L104`
- **Error/guard literals:** `CUSTOMER_DATA_IMPORT_EXECUTION_JOB_NOT_EXECUTING`, `CUSTOMER_DATA_IMPORT_EXECUTION_ROW_JOB_MISMATCH`, `CUSTOMER_DATA_IMPORT_EXECUTION_ROW_REFERENCE_MISSING`, `CUSTOMER_DATA_IMPORT_EXECUTION_INVALID_ROW_POLICY_CONFLICT`
- **Rust crate references:** `crm_customer_data_operations`, `crm_module_sdk`

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-execution-composition/src/lib.rs) · 45 lines

- **Re-exports:** `pub use production_contribution::*;`, `pub use crm_core_files::{`, `pub use export_execution_reader::*;`, `pub use export_execution_sink::*;`, `pub use export_execution_source::*;`, `pub use export_execution_worker::*;`, `pub use export_selection_reader::*;`, `pub use export_selection_sink::*;`, `pub use export_selection_worker::*;`, `pub use privacy_export::*;`, `pub use postgres_reader::*;`, `pub use outcome_plan::*;`, `pub use postgres_outcome_sink::*;`, `pub use worker::*;`
- **Rust crate references:** `crm_core_files`

### [`src/outcome_plan.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-execution-composition/src/outcome_plan.rs) · 365 lines

- **Public declarations (8):** `expected_version@L14`, `after@L18`, `expected_version@L30`, `after@L34`, `plan_skip_invalid@L60`, `plan_success@L93`, `plan_retryable_failure@L145`, `plan_completion@L181`
- **Internal/trait/helpers (8):** `require_same_job@L199`, `outcome_error@L211`, `success_plans_row_and_job_updates_from_the_same_authoritative_versions@L230`, `retryable_failure_updates_only_the_row_and_preserves_the_job_checkpoint@L255`, `invalid_skip_advances_only_the_next_sequential_checkpoint@L272`, `executing_job@L289`, `valid_row@L332`, `invalid_row@L351`
- **Public types:** `PlannedImportJobUpdate@L8`, `PlannedImportRowUpdate@L24`, `ImportExecutionOutcomePlan@L40`
- **Detected tests (3):** `success_plans_row_and_job_updates_from_the_same_authoritative_versions@L230`, `retryable_failure_updates_only_the_row_and_preserves_the_job_checkpoint@L255`, `invalid_skip_advances_only_the_next_sequential_checkpoint@L272`
- **Error/guard literals:** `CUSTOMER_DATA_IMPORT_EXECUTION_SKIP_ROW_NOT_INVALID`, `CUSTOMER_DATA_IMPORT_EXECUTION_ATTEMPTS_EXHAUSTED`, `CUSTOMER_DATA_IMPORT_EXECUTION_FAILURE_CODE_MISSING`, `CUSTOMER_DATA_IMPORT_EXECUTION_OUTCOME_JOB_MISMATCH`, `PARTY_TEMPORARY`, `DISPLAY_NAME_MISSING`
- **Rust crate references:** `crm_customer_data_operations`, `crm_module_sdk`

### [`src/postgres_outcome_sink.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-execution-composition/src/postgres_outcome_sink.rs) · 688 lines

- **Public declarations (3):** `new@L67`, `internal_capability_definitions@L247`, `internal_capability_definition@L254`
- **Internal/trait/helpers (24):** `fmt@L73`, `skip_invalid@L83`, `record_success@L107`, `record_retryable_failure@L141`, `complete@L171`, `authorize_and_execute@L194`, `prepare_outcome_commit@L228`, `internal_request@L293`, `batch_from_plan@L319`, `skipped_invalid_batch@L344`, `success_batch@L391`, `retryable_failure_batch@L468`, `completion_batch@L509`, `job_payload@L550`, `row_payload@L558`, `job_record_ref@L566`, `row_record_ref@L574`, `job_ref@L582`, `job_ref_wire@L586`, `row_ref@L592`, `batch_error_to_sdk@L601`, `configuration_error@L647`, `hex@L657`, `internal_outcome_capabilities_are_typed_idempotent_and_not_public_mutation_coordinates@L672`
- **Public types:** `PostgresImportExecutionOutcomeSink@L61`
- **Detected tests (1):** `internal_outcome_capabilities_are_typed_idempotent_and_not_public_mutation_coordinates@L672`
- **Error/guard literals:** `CUSTOMER_DATA_IMPORT_EXECUTION_OUTCOME_PERMISSION_DENIED`, `CUSTOMER_DATA_IMPORT_EXECUTION_INTERNAL_CAPABILITY_UNSUPPORTED`, `CUSTOMER_DATA_IMPORT_EXECUTION_OUTCOME_CONFLICT`, `CUSTOMER_DATA_IMPORT_EXECUTION_OUTCOME_IDEMPOTENCY_CONFLICT`, `CUSTOMER_DATA_IMPORT_EXECUTION_OUTCOME_IN_PROGRESS`, `CUSTOMER_DATA_IMPORT_EXECUTION_OUTCOME_STORAGE_UNAVAILABLE`, `CUSTOMER_DATA_IMPORT_EXECUTION_OUTCOME_PLAN_INVALID`, `CUSTOMER_DATA_IMPORT_EXECUTION_OUTCOME_REPLAY_INVALID`, `CUSTOMER_DATA_IMPORT_EXECUTION_OUTCOME_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_capability_adapters`, `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_data_operations`, `crm_customer_data_operations_capability_adapter`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/postgres_reader.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-execution-composition/src/postgres_reader.rs) · 215 lines

- **Public declarations (2):** `new@L40`, `try_with_page_size@L47`
- **Internal/trait/helpers (11):** `load@L20`, `load@L54`, `validate_page_size@L137`, `module_id@L150`, `job_record_type@L154`, `row_record_type@L158`, `job_not_found@L162`, `configuration_error@L171`, `reader_error@L181`, `rejects_zero_and_oversized_page_sizes_before_database_access@L195`, `accepts_the_default_bounded_page_size@L211`
- **Public types:** `ImportExecutionSnapshotReader@L19`, `PostgresImportExecutionSnapshotReader@L34`
- **Detected tests (2):** `rejects_zero_and_oversized_page_sizes_before_database_access@L195`, `accepts_the_default_bounded_page_size@L211`
- **Error/guard literals:** `CUSTOMER_DATA_IMPORT_EXECUTION_ROW_IDENTITY_INVALID`, `CUSTOMER_DATA_IMPORT_EXECUTION_ROW_SET_OVERSIZED`, `CUSTOMER_DATA_IMPORT_EXECUTION_READER_PAGE_SIZE_INVALID`, `CUSTOMER_DATA_IMPORT_EXECUTION_JOB_NOT_FOUND`, `CUSTOMER_DATA_IMPORT_EXECUTION_READER_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_core_data`, `crm_customer_data_operations`, `crm_customer_data_operations_capability_adapter`, `crm_module_sdk`

### [`src/privacy_export.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-execution-composition/src/privacy_export.rs) · 643 lines

- **Public declarations (2):** `new@L91`, `request@L95`
- **Internal/trait/helpers (24):** `fmt@L81`, `load_job@L214`, `persist_job@L229`, `label@L335`, `build@L352`, `execution_context@L382`, `validate_job_snapshot@L413`, `validate_artifact_metadata@L439`, `validate_finalized_artifact@L459`, `job_state_bytes@L474`, `event_payload@L497`, `audit_envelope@L506`, `job_contract@L516`, `request_hash@L527`, `stable_id@L534`, `hash_field@L544`, `sha256@L549`, `module_id@L553`, `short_hex@L557`, `hex@L561`, `batch_error_to_sdk@L571`, `configuration_error@L606`, `job_state_invalid@L616`, `frozen_target_coordinate_and_private_job_contract_are_exact@L631`
- **Public types:** `PrivacyManifestExportRequest@L46`, `PrivacyManifestExportResult@L63`, `PrivacyManifestExportPublisher@L75`
- **Detected tests (1):** `frozen_target_coordinate_and_private_job_contract_are_exact@L631`
- **Error/guard literals:** `CUSTOMER_DATA_PRIVACY_EXPORT_IDEMPOTENCY_CONFLICT`, `CUSTOMER_DATA_PRIVACY_EXPORT_IN_PROGRESS`, `CUSTOMER_DATA_PRIVACY_EXPORT_STORE_UNAVAILABLE`, `CUSTOMER_DATA_PRIVACY_EXPORT_REPLAY_INVALID`, `CUSTOMER_DATA_PRIVACY_EXPORT_CONFIGURATION_INVALID`, `CUSTOMER_DATA_PRIVACY_EXPORT_STATE_INVALID`
- **Rust crate references:** `crm_capability_plan_support`, `crm_core_data`, `crm_core_files`, `crm_customer_data_operations_capability_adapter`, `crm_module_sdk`

### [`src/production_contribution.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-execution-composition/src/production_contribution.rs) · 155 lines

- **Public declarations (3):** `mutation_capability_definitions@L43`, `query_capability_definitions@L49`, `build_contribution@L53`
- **Internal/trait/helpers (3):** `ordinary_mutation_capability_definitions@L122`, `cursor@L134`, `production_composition_error@L146`
- **Public types:** `CustomerDataOperationsProductionDependencies@L35`
- **Error/guard literals:** `CUSTOMER_DATA_OPERATIONS_CURSOR_CONFIGURATION_INVALID`, `CUSTOMER_DATA_OPERATIONS_PRODUCTION_COMPOSITION_INVALID`
- **Rust crate references:** `crm_application_composition`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_data_operations_capability_adapter`, `crm_customer_data_operations_query_adapter`, `crm_customer_data_operations_source_composition`, `crm_module_sdk`, `crm_query_runtime`

### [`src/worker.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-execution-composition/src/worker.rs) · 240 lines

- **Public declarations (4):** `new@L61`, `try_with_page_size@L77`, `actor_id@L102`, `run_tenant_cycle@L106`
- **Internal/trait/helpers (5):** `fmt@L47`, `worker_context@L166`, `configuration_error@L198`, `worker_state_unavailable@L208`, `worker_context_uses_a_private_worker_identity_and_internal_cycle_coordinate@L222`
- **Public types:** `ImportExecutionTenantCycle@L29`, `PartyImportExecutionWorker@L36`
- **Detected tests (1):** `worker_context_uses_a_private_worker_identity_and_internal_cycle_coordinate@L222`
- **Error/guard literals:** `CUSTOMER_DATA_IMPORT_EXECUTION_WORKER_CONFIGURATION_INVALID`, `CUSTOMER_DATA_IMPORT_EXECUTION_WORKER_STATE_UNAVAILABLE`
- **Rust crate references:** `crm_core_data`, `crm_customer_data_operations`, `crm_customer_data_operations_capability_adapter`, `crm_module_sdk`

### [`tests/export_execution_runtime_contract.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-execution-composition/tests/export_execution_runtime_contract.rs) · 15 lines

- **Internal/trait/helpers (2):** `assert_send_sync@L6`, `export_execution_production_types_are_thread_safe@L9`
- **Detected tests (1):** `export_execution_production_types_are_thread_safe@L9`
- **Rust crate references:** `crm_customer_data_operations_execution_composition`

### [`tests/internal_outcome_surface.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-execution-composition/tests/internal_outcome_surface.rs) · 26 lines

- **Internal/trait/helpers (1):** `worker_only_outcome_capabilities_are_typed_idempotent_and_absent_from_public_mutations@L8`
- **Detected tests (1):** `worker_only_outcome_capabilities_are_typed_idempotent_and_absent_from_public_mutations@L8`
- **Rust crate references:** `crm_customer_data_operations_capability_adapter`, `crm_customer_data_operations_execution_composition`, `crm_module_sdk`

### [`tests/public_surface.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-execution-composition/tests/public_surface.rs) · 137 lines

- **Internal/trait/helpers (5):** `public_composition_surface_builds_only_the_exact_governed_party_create_invocation@L17`, `execution_snapshot_rejects_an_incomplete_authoritative_row_set_before_target_execution@L42`, `invalid_row_cannot_build_a_target_party_invocation@L56`, `executing_job@L77`, `valid_row@L115`
- **Detected tests (3):** `public_composition_surface_builds_only_the_exact_governed_party_create_invocation@L17`, `execution_snapshot_rejects_an_incomplete_authoritative_row_set_before_target_execution@L42`, `invalid_row_cannot_build_a_target_party_invocation@L56`
- **Error/guard literals:** `CUSTOMER_DATA_IMPORT_EXECUTION_ROW_SET_INCOMPLETE`, `DISPLAY_NAME_MISSING`, `CUSTOMER_DATA_IMPORT_EXECUTION_ROW_NOT_EXECUTABLE`
- **Rust crate references:** `crm_customer_data_operations`, `crm_customer_data_operations_execution_composition`, `crm_module_sdk`, `crm_parties_capability_adapter`

### [`tests/reader_surface.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-execution-composition/tests/reader_surface.rs) · 16 lines

- **Internal/trait/helpers (2):** `production_reader_implements_the_execution_snapshot_port@L11`, `assert_reader_port@L12`
- **Detected tests (2):** `production_reader_implements_the_execution_snapshot_port@L11`, `assert_reader_port@L12`
- **Rust crate references:** `crm_core_data`, `crm_customer_data_operations_execution_composition`

## 28. `crm-customer-data-operations-privacy-scope-adapter` — END / B-END + D? adapter

### [`src/contract.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-privacy-scope-adapter/src/contract.rs) · 92 lines

- **Public declarations (8):** `customer_data_privacy_scope_definition@L31`, `validate_definition@L47`, `module_id@L57`, `capability_id@L61`, `capability_version@L65`, `schema_id@L69`, `schema_version@L73`, `output_descriptor_hash@L77`
- **Internal/trait/helpers (1):** `payload_contract@L81`
- **Error/guard literals:** `CUSTOMER_DATA_PRIVACY_SCOPE_DEFINITION_MISMATCH`
- **Rust crate references:** `crm_capability_runtime`, `crm_customer_data_operations_capability_adapter`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/digest.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-privacy-scope-adapter/src/digest.rs) · 44 lines

- **Public declarations (2):** `encode_hex@L3`, `decode_hex@L13`
- **Internal/trait/helpers (2):** `nibble@L30`, `cursor_invalid@L38`
- **Error/guard literals:** `CUSTOMER_DATA_PRIVACY_SCOPE_CURSOR_INVALID`
- **Rust crate references:** `crm_module_sdk`

### [`src/errors.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-privacy-scope-adapter/src/errors.rs) · 221 lines

- **Public declarations (15):** `configured@L6`, `map_request_contract_error@L18`, `map_common_lineage_error@L37`, `map_canonical_party_claim_error@L91`, `invalid_contract@L117`, `invalid_contract_with_reference@L121`, `lineage_invalid@L129`, `import_row_state_invalid@L143`, `export_selection_state_invalid@L151`, `export_stage_state_invalid@L159`, `export_outcome_state_invalid@L167`, `association_state_invalid@L175`, `scan_limit_exceeded@L183`, `canonical_resolution_unavailable@L193`, `database_unavailable@L203`
- **Internal/trait/helpers (1):** `stored_state_invalid@L213`
- **Error/guard literals:** `CUSTOMER_DATA_PRIVACY_SCOPE_CONFIGURATION_INVALID`, `CUSTOMER_DATA_PRIVACY_SCOPE_REQUEST_BINDING_MISMATCH`, `CUSTOMER_DATA_PRIVACY_SCOPE_INPUT_CONTRACT_MISMATCH`, `CUSTOMER_DATA_PRIVACY_SCOPE_INPUT_HASH_MISMATCH`, `CUSTOMER_DATA_PRIVACY_SCOPE_TENANT_MISMATCH`, `CUSTOMER_DATA_PRIVACY_SCOPE_CASE_ID_INVALID`, `CUSTOMER_DATA_PRIVACY_SCOPE_PARTY_INVALID`, `CUSTOMER_DATA_PRIVACY_SCOPE_GENERATION_INVALID`, `CUSTOMER_DATA_PRIVACY_SCOPE_REGISTRY_INVALID`, `CUSTOMER_DATA_PRIVACY_SCOPE_REGISTRY_UNAVAILABLE`, `CUSTOMER_DATA_PRIVACY_SCOPE_REGISTRY_MISMATCH`, `CUSTOMER_DATA_PRIVACY_SCOPE_PURPOSE_INVALID`, `CUSTOMER_DATA_PRIVACY_SCOPE_REQUEST_TIME_INVALID`, `CUSTOMER_DATA_PRIVACY_SCOPE_PAGE_SIZE_INVALID`, `CUSTOMER_DATA_PRIVACY_SCOPE_LINEAGE_INVALID`, `CUSTOMER_DATA_PRIVACY_SCOPE_IMPORT_ROW_STATE_INVALID`, `CUSTOMER_DATA_PRIVACY_SCOPE_EXPORT_SELECTION_STATE_INVALID`, `CUSTOMER_DATA_PRIVACY_SCOPE_EXPORT_STAGE_STATE_INVALID`, `CUSTOMER_DATA_PRIVACY_SCOPE_EXPORT_OUTCOME_STATE_INVALID`, `CUSTOMER_DATA_PRIVACY_SCOPE_ASSOCIATION_STATE_INVALID`, `CUSTOMER_DATA_PRIVACY_SCOPE_SCAN_LIMIT_EXCEEDED`, `CUSTOMER_DATA_PRIVACY_SCOPE_CANONICAL_RESOLUTION_UNAVAILABLE`, `CUSTOMER_DATA_PRIVACY_SCOPE_UNAVAILABLE`
- **Rust crate references:** `crm_customer_privacy_owner_scope_support`, `crm_module_sdk`

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-privacy-scope-adapter/src/lib.rs) · 36 lines

- **Re-exports:** `pub use contract::{`, `pub use owner_action::{`, `pub use postgres::CustomerDataOperationsPrivacyScopeQueryAdapter;`

### [`src/owner_action.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-privacy-scope-adapter/src/owner_action.rs) · 379 lines

- **Public declarations (2):** `customer_data_operations_privacy_action_definition@L40`, `customer_data_operations_privacy_action_planner@L45`
- **Internal/trait/helpers (22):** `owner_module_id@L51`, `capability_id@L55`, `supports_resource_type@L59`, `anonymize@L69`, `deletion_tombstone@L92`, `minimize_import_row@L102`, `minimized_prepared_party@L127`, `validate_resource@L142`, `validate_import_row@L152`, `validate_export_selection_item@L173`, `validate_export_execution_stage@L194`, `validate_export_execution_outcome@L215`, `validate_payload@L236`, `import_row_payload@L262`, `unsupported@L277`, `transition_invalid@L281`, `stored_state_invalid@L291`, `hex_prefix@L301`, `publishes_the_frozen_owner_action_coordinate@L318`, `supports_exactly_the_four_discovered_resource_families@L330`, `import_row_without_prepared_party_remains_rehydratable_after_privacy_transition@L340`, `minimized_name_is_deterministic_and_non_identifying@L364`
- **Public types:** `CustomerDataOperationsPrivacyActionPlanner@L34`, `CustomerDataOperationsPrivacyActionPolicy@L38`
- **Detected tests (4):** `publishes_the_frozen_owner_action_coordinate@L318`, `supports_exactly_the_four_discovered_resource_families@L330`, `import_row_without_prepared_party_remains_rehydratable_after_privacy_transition@L340`, `minimized_name_is_deterministic_and_non_identifying@L364`
- **Error/guard literals:** `CUSTOMER_DATA_PRIVACY_TRANSITION_INVALID`, `CUSTOMER_DATA_PRIVACY_STORED_STATE_INVALID`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_customer_data_operations`, `crm_customer_data_operations_capability_adapter`, `crm_customer_privacy_owner_scope_support`, `crm_module_sdk`

### [`src/postgres.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-privacy-scope-adapter/src/postgres.rs) · 666 lines

- **Public declarations (1):** `new@L59`
- **Internal/trait/helpers (16):** `fmt@L50`, `execute_query@L63`, `execute@L101`, `new@L152`, `resolves_to_subject@L159`, `read_customer_data_page@L220`, `resource_after_cursor@L399`, `selection_key@L410`, `load_record_rows@L431`, `decode_stored_row@L493`, `strict_import_record@L534`, `strict_selection_record@L557`, `strict_stage_record@L579`, `strict_outcome_record@L596`, `strict_snapshot@L613`, `positive_version@L663`
- **Public types:** `CustomerDataOperationsPrivacyScopeQueryAdapter@L45`
- **Error/guard literals:** `IDENTITY_RESOLUTION_CANONICAL_PARTY_MISMATCH`, `IDENTITY_RESOLUTION_TOPOLOGY_GENERATION_STALE`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_data_operations`, `crm_customer_data_operations_capability_adapter`, `crm_customer_privacy_owner_scope_support`, `crm_identity_resolution`, `crm_identity_resolution_topology_composition`, `crm_module_sdk`, `crm_query_runtime`, `crm_capability_plan_support`

### [`src/request.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-privacy-scope-adapter/src/request.rs) · 248 lines

- **Public declarations (4):** `token@L30`, `validate_request_contract@L66`, `validate_wire_request@L71`, `encode_cursor@L132`
- **Internal/trait/helpers (4):** `parse@L39`, `decode_cursor@L161`, `cursor_digest@L210`, `cursor_invalid@L242`
- **Public types:** `ResourceFamily@L22`, `CursorState@L51`, `ValidatedRequest@L57`
- **Error/guard literals:** `CUSTOMER_DATA_PRIVACY_SCOPE_REQUEST_INVALID`, `CUSTOMER_DATA_PRIVACY_SCOPE_LINEAGE_INVALID`, `CUSTOMER_DATA_PRIVACY_SCOPE_CURSOR_INVALID`
- **Rust crate references:** `crm_customer_privacy_owner_scope_support`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`

### [`src/response.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-privacy-scope-adapter/src/response.rs) · 173 lines

- **Public declarations (2):** `build_response@L57`, `typed_output@L128`
- **Internal/trait/helpers (3):** `resource_type@L34`, `retention_policy_id@L43`, `page_digest@L144`
- **Public types:** `VerifiedCustomerDataResource@L27`
- **Rust crate references:** `crm_customer_data_operations`, `crm_customer_data_operations_capability_adapter`, `crm_customer_privacy_owner_scope_support`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/tests.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-privacy-scope-adapter/src/tests.rs) · 312 lines

- **Internal/trait/helpers (9):** `context@L31`, `wire_request@L45`, `query_request@L70`, `assert_invalid_argument@L92`, `publishes_exact_contract_only_coordinate_and_frozen_bounds@L99`, `request_contract_preserves_customer_data_integrity_errors@L116`, `four_family_cursor_round_trip_is_bound_to_lineage_and_page_size@L148`, `response_is_four_family_reference_only_and_deterministic@L204`, `invalid_lineage_page_size_and_owner_errors_are_stable@L278`
- **Detected tests (5):** `publishes_exact_contract_only_coordinate_and_frozen_bounds@L99`, `request_contract_preserves_customer_data_integrity_errors@L116`, `four_family_cursor_round_trip_is_bound_to_lineage_and_page_size@L148`, `response_is_four_family_reference_only_and_deterministic@L204`, `invalid_lineage_page_size_and_owner_errors_are_stable@L278`
- **Error/guard literals:** `PRIVACY_ERASURE_SCOPE`, `CUSTOMER_DATA_PRIVACY_SCOPE_REQUEST_BINDING_MISMATCH`, `CUSTOMER_DATA_PRIVACY_SCOPE_INPUT_HASH_MISMATCH`, `CUSTOMER_DATA_PRIVACY_SCOPE_INPUT_CONTRACT_MISMATCH`, `CUSTOMER_DATA_PRIVACY_SCOPE_CURSOR_INVALID`, `CUSTOMER_DATA_PRIVACY_SCOPE_TENANT_MISMATCH`, `CUSTOMER_DATA_PRIVACY_SCOPE_PAGE_SIZE_INVALID`, `CUSTOMER_DATA_PRIVACY_SCOPE_REQUEST_INVALID`, `CUSTOMER_DATA_PRIVACY_SCOPE_ASSOCIATION_STATE_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_customer_data_operations_capability_adapter`, `crm_customer_privacy`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`

### [`tests/postgres_scope.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-privacy-scope-adapter/tests/postgres_scope.rs) · 350 lines

- **Internal/trait/helpers (3):** `customer_data_scope_is_alias_aware_complete_reference_only_and_side_effect_free@L29`, `decode@L323`, `assert_response_omits_private_customer_data@L328`
- **Detected tests (1):** `customer_data_scope_is_alias_aware_complete_reference_only_and_side_effect_free@L29`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`, `CUSTOMER_DATA_PRIVACY_SCOPE_LINEAGE_INVALID`, `CUSTOMER_DATA_PRIVACY_SCOPE_CURSOR_INVALID`, `CUSTOMER_DATA_PRIVACY_SCOPE_EXPORT_SELECTION_STATE_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_data_operations_privacy_scope_adapter`, `crm_identity_resolution_capability_adapter`, `crm_parties_capability_adapter`, `crm_proto_contracts`, `crm_query_runtime`

### [`tests/postgres_scope/support.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-privacy-scope-adapter/tests/postgres_scope/support.rs) · 549 lines

- **Public declarations (9):** `create_party@L50`, `merge_party@L81`, `insert_valid_import_row@L130`, `insert_pending_import_row@L181`, `insert_export_evidence@L215`, `scope_request@L390`, `current_generation@L447`, `write_surface_counts@L479`, `corrupt_selection_metadata@L520`
- **Internal/trait/helpers (2):** `insert_record@L290`, `capability_request@L344`
- **Public types:** `ExportEvidenceIds@L209`, `WriteSurfaceCounts@L468`
- **Error/guard literals:** `PRIVACY_ERASURE_SCOPE`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_customer_data_operations`, `crm_customer_data_operations_capability_adapter`, `crm_customer_data_operations_privacy_scope_adapter`, `crm_customer_privacy`, `crm_identity_resolution_capability_adapter`, `crm_module_sdk`, `crm_parties_capability_adapter`, `crm_proto_contracts`, `crm_query_runtime`, `crm_parties`, `crm_identity_resolution`

## 29. `crm-customer-data-operations-query-adapter` — NEXT/LATER / B-CONDITIONAL + D? adapter

### [`src/artifact_download.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-query-adapter/src/artifact_download.rs) · 264 lines

- **Public declarations (4):** `new@L51`, `resolve@L55`, `artifact_download_capability_definition@L109`, `artifact_download_request_payload@L133`
- **Internal/trait/helpers (12):** `fmt@L41`, `validate_download_request@L149`, `decode_sha256@L168`, `module_id@L180`, `export_job_record_type@L184`, `invalid_request@L188`, `resource_not_found@L197`, `artifact_not_ready@L206`, `stored_state_invalid@L215`, `configuration_error@L224`, `download_capability_is_high_risk_read_only_and_job_bound@L239`, `sha256_decoder_is_exact@L259`
- **Public types:** `PartyExportArtifactDownloadEvidence@L23`, `PartyExportArtifactDownloadResolver@L35`
- **Detected tests (2):** `download_capability_is_high_risk_read_only_and_job_bound@L239`, `sha256_decoder_is_exact@L259`
- **Error/guard literals:** `CUSTOMER_DATA_EXPORT_ARTIFACT_DOWNLOAD_REQUEST_INVALID`, `CUSTOMER_DATA_EXPORT_ARTIFACT_NOT_FOUND`, `CUSTOMER_DATA_EXPORT_ARTIFACT_NOT_READY`, `CUSTOMER_DATA_EXPORT_ARTIFACT_STORED_STATE_INVALID`, `CUSTOMER_DATA_EXPORT_ARTIFACT_DOWNLOAD_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_data_operations`, `crm_customer_data_operations_capability_adapter`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`

### [`src/export_query.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-query-adapter/src/export_query.rs) · 597 lines

- **Public declarations (5):** `new@L62`, `validate_request@L81`, `execute_request@L108`, `export_query_capability_definitions@L281`, `export_query_capability_definition@L288`
- **Internal/trait/helpers (25):** `fmt@L50`, `execute_get_job@L120`, `execute_list_jobs@L140`, `get_visible_job_snapshot@L165`, `collect_jobs@L190`, `has_more_visible_job@L242`, `export_job_to_wire_with_visibility@L327`, `job_matches_status@L358`, `validate_status_filter@L381`, `export_job_record_id@L391`, `cursor_binding@L406`, `filter_hash@L423`, `decode_after@L428`, `encode_next@L449`, `decode_input@L470`, `module_id@L501`, `export_job_record_type@L505`, `enforce_scan_limit@L509`, `resource_not_found@L517`, `unsupported_query@L526`, `cursor_invalid@L535`, `cursor_error@L544`, `scan_limit_error@L554`, `config_error@L563`, `publishes_two_personal_read_only_export_queries@L578`
- **Public types:** `PartyExportQueryAdapter@L42`
- **Detected tests (1):** `publishes_two_personal_read_only_export_queries@L578`
- **Error/guard literals:** `CUSTOMER_DATA_EXPORT_QUERY_INPUT_CONTRACT_MISMATCH`, `CUSTOMER_DATA_EXPORT_QUERY_INPUT_PROTOBUF_INVALID`, `QUERY_RESOURCE_NOT_FOUND`, `CUSTOMER_DATA_EXPORT_QUERY_CAPABILITY_UNSUPPORTED`, `CUSTOMER_DATA_EXPORT_QUERY_CURSOR_INVALID`, `CUSTOMER_DATA_EXPORT_QUERY_VISIBILITY_SCAN_LIMIT_EXCEEDED`, `CUSTOMER_DATA_EXPORT_QUERY_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_data_operations`, `crm_customer_data_operations_capability_adapter`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-query-adapter/src/lib.rs) · 1036 lines

- **Public declarations (3):** `new@L96`, `query_capability_definitions@L119`, `query_capability_definition@L128`
- **Internal/trait/helpers (45):** `fmt@L84`, `validate@L171`, `execute@L223`, `execute_get_job@L246`, `execute_list_jobs@L267`, `execute_list_rows@L293`, `get_visible_job_snapshot@L333`, `collect_jobs@L358`, `has_more_visible_job@L410`, `collect_rows@L448`, `has_more_visible_row@L503`, `import_row_from_snapshot@L545`, `job_to_wire_with_visibility@L563`, `import_row_to_wire_with_visibility@L590`, `job_matches_status@L621`, `row_matches_status@L632`, `validate_job_status_filter@L645`, `validate_row_status_filter@L655`, `jobs_cursor_binding@L665`, `rows_cursor_binding@L682`, `decode_job_after@L699`, `encode_job_next@L720`, `decode_row_after@L741`, `encode_row_next@L759`, `job_filter_hash@L780`, `row_filter_hash@L785`, `decode_input@L793`, `import_job_record_id@L824`, `module_id@L839`, `import_job_record_type@L843`, `import_row_record_type@L847`, `import_job_row_relationship_type@L851`, `enforce_scan_limit@L855`, `resource_not_found@L863`, `unsupported_query@L872`, `cursor_invalid@L881`, `cursor_error@L890`, `scan_limit_error@L900`, `stored_data_error@L909`, `config_error@L918`, `job@L937`, `publishes_five_personal_read_only_queries@L963`, `job_visibility_redacts_source_mapping_and_operational_fields@L981`, `row_visibility_redacts_source_identifier_evidence@L1000`, `row_cursor_filter_is_bound_to_job_and_status@L1025`
- **Public types:** `CustomerDataOperationsQueryAdapter@L75`
- **Re-exports:** `pub use artifact_download::*;`, `pub use export_query::{`
- **Detected tests (4):** `publishes_five_personal_read_only_queries@L963`, `job_visibility_redacts_source_mapping_and_operational_fields@L981`, `row_visibility_redacts_source_identifier_evidence@L1000`, `row_cursor_filter_is_bound_to_job_and_status@L1025`
- **Error/guard literals:** `CUSTOMER_DATA_IMPORT_PERSISTED_ROW_IDENTITY_INVALID`, `CUSTOMER_DATA_IMPORT_QUERY_INPUT_CONTRACT_MISMATCH`, `CUSTOMER_DATA_IMPORT_QUERY_INPUT_PROTOBUF_INVALID`, `QUERY_RESOURCE_NOT_FOUND`, `CUSTOMER_DATA_IMPORT_QUERY_CAPABILITY_UNSUPPORTED`, `CUSTOMER_DATA_IMPORT_QUERY_CURSOR_INVALID`, `CUSTOMER_DATA_IMPORT_QUERY_VISIBILITY_SCAN_LIMIT_EXCEEDED`, `CUSTOMER_DATA_IMPORT_QUERY_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_data_operations`, `crm_customer_data_operations_capability_adapter`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`

### [`tests/public_surface.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-query-adapter/tests/public_surface.rs) · 35 lines

- **Internal/trait/helpers (1):** `publishes_exact_permission_aware_customer_data_query_surface@L8`
- **Detected tests (1):** `publishes_exact_permission_aware_customer_data_query_surface@L8`
- **Rust crate references:** `crm_customer_data_operations_query_adapter`, `crm_module_sdk`

## 30. `crm-customer-data-operations-source-composition` — NEXT/LATER / B-CONDITIONAL + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-source-composition/src/lib.rs) · 1295 lines

- **Public declarations (3):** `source_capability_definitions@L102`, `source_capability_definition@L109`, `new@L171`
- **Internal/trait/helpers (38):** `fmt@L185`, `execute@L196`, `create_source_artifact@L226`, `append_source_chunk@L262`, `finalize_source_artifact@L293`, `create_job_from_source@L316`, `validate_source_batch@L426`, `source_artifact_create_evidence@L659`, `source_chunk_append_evidence@L695`, `source_artifact_finalize_evidence@L735`, `file_artifact_evidence@L772`, `file_artifact_record_ref@L799`, `source_artifact_to_wire@L807`, `file_artifact_version@L820`, `sha256_array@L835`, `validate_source_row@L841`, `target_party_id_for_row@L956`, `party_kind_for_row@L978`, `required_mapped_value@L1004`, `optional_mapped_value@L1025`, `parser_profile_from_wire@L1034`, `mapping_from_wire@L1110`, `partial_execution_policy_from_wire@L1128`, `import_job_id_from_ref@L1141`, `artifact_id_from_ref@L1151`, `import_job_record_ref@L1163`, `import_row_record_ref@L1171`, `import_job_persisted_payload@L1179`, `import_row_persisted_payload@L1187`, `reauthorize@L1195`, `ensure_definition@L1217`, `hex_sha256@L1232`, `invalid_plan@L1241`, `identifier_configuration_error@L1245`, `configuration_error@L1249`, `source_conflict@L1259`, `source_not_found@L1263`, `publishes_only_artifact_backed_job_create_and_validation_coordinates@L1277`
- **Public types:** `CustomerDataOperationsSourceExecutor@L164`
- **Detected tests (1):** `publishes_only_artifact_backed_job_create_and_validation_coordinates@L1277`
- **Error/guard literals:** `CUSTOMER_DATA_IMPORT_SOURCE_ARTIFACT_NOT_FINALIZED`, `CUSTOMER_DATA_IMPORT_JOB_NOT_CREATED`, `CUSTOMER_DATA_IMPORT_SOURCE_ARTIFACT_BINDING_MISSING`, `CUSTOMER_DATA_IMPORT_SOURCE_ARTIFACT_DIGEST_MISMATCH`, `CUSTOMER_DATA_IMPORT_SOURCE_ARTIFACT_ROW_COUNT_MISMATCH`, `EXTERNAL_ROW_KEY_MISSING`, `EXTERNAL_ROW_KEY_INVALID`, `SOURCE_EXTERNAL_ID_MISSING`, `SOURCE_EXTERNAL_ID_INVALID`, `DISPLAY_NAME_MISSING`, `TARGET_PARTY_DUPLICATE_IN_BATCH`, `TARGET_PARTY_ID_MISSING`, `TARGET_PARTY_ID_INVALID`, `PARTY_KIND_MISSING`, `PARTY_KIND_INVALID`, `CAPABILITY_PERMISSION_DENIED`, `CUSTOMER_DATA_IMPORT_SOURCE_CONFIGURATION_INVALID`, `CUSTOMER_DATA_IMPORT_SOURCE_NOT_FOUND`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_core_files`, `crm_customer_data_operations`, `crm_customer_data_operations_capability_adapter`, `crm_module_sdk`, `crm_proto_contracts`

### [`tests/source_capability_surface.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-data-operations-source-composition/tests/source_capability_surface.rs) · 43 lines

- **Internal/trait/helpers (2):** `production_source_capabilities_are_artifact_backed_and_idempotent@L10`, `legacy_preparsed_coordinates_are_not_part_of_the_source_capability_surface@L39`
- **Detected tests (2):** `production_source_capabilities_are_artifact_backed_and_idempotent@L10`, `legacy_preparsed_coordinates_are_not_part_of_the_source_capability_surface@L39`
- **Rust crate references:** `crm_customer_data_operations_source_composition`, `crm_module_sdk`

## 31. `crm-customer-enrichment-application-adapter` — LATER / B-LATER + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-application-adapter/src/lib.rs) · 868 lines

- **Public declarations (12):** `application_capability_definitions@L56`, `apply_party_display_name_capability_definition@L63`, `record_application_outcome_capability_definition@L72`, `new@L115`, `new@L250`, `application_attempt_persisted_contract@L378`, `application_attempt_from_snapshot@L389`, `application_attempt_persisted_payload@L418`, `application_attempt_record_ref@L428`, `suggestion_from_application_snapshot@L436`, `review_from_application_snapshot@L442`, `application_attempt_to_wire@L448`
- **Internal/trait/helpers (28):** `application_definition@L82`, `application_contract@L103`, `application_attempt@L122`, `target@L157`, `plan@L169`, `target@L259`, `plan@L271`, `recorded_application_outcome_to_wire@L488`, `application_outcome_to_wire@L500`, `application_outcome_from_wire@L545`, `outcome_attempt_record_ref@L586`, `ensure_attempt_lineage@L613`, `ensure_suggestion_ref@L635`, `ensure_review_ref@L659`, `ensure_definition@L683`, `request_started_at_unix_ms@L700`, `positive_u64@L712`, `non_negative_u64@L723`, `checked_i64@L728`, `target_field_to_wire@L732`, `suggestion_status_to_wire@L738`, `configured@L755`, `application_attempt_not_found@L767`, `application_conflict@L776`, `application_plan_invalid@L786`, `application_state_invalid@L796`, `definitions_are_exact_personal_mutations@L837`, `persisted_contract_is_exact_and_bounded@L856`
- **Public types:** `CustomerEnrichmentApplicationAttemptPlanner@L109`, `CustomerEnrichmentApplicationOutcomePlanner@L244`
- **Detected tests (2):** `definitions_are_exact_personal_mutations@L837`, `persisted_contract_is_exact_and_bounded@L856`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_APPLICATION_CONFIGURATION_INVALID`, `CUSTOMER_ENRICHMENT_APPLICATION_ATTEMPT_NOT_FOUND`, `CUSTOMER_ENRICHMENT_APPLICATION_CONFLICT`, `CUSTOMER_ENRICHMENT_APPLICATION_PLAN_INVALID`, `CUSTOMER_ENRICHMENT_APPLICATION_STATE_INVALID`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_review_adapter`, `crm_module_sdk`, `crm_proto_contracts`

## 32. `crm-customer-enrichment-application-composition` — LATER / B-LATER + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-application-composition/src/lib.rs) · 371 lines

- **Public declarations (5):** `new@L53`, `execute@L57`, `new@L125`, `authorized@L132`, `execute@L139`
- **Internal/trait/helpers (16):** `load_required@L91`, `fmt@L112`, `authorize@L179`, `load_required@L199`, `ensure_exact_definition@L213`, `required_suggestion_ref@L230`, `required_review_ref@L244`, `required_attempt_ref@L258`, `required_record_ref@L274`, `suggestion_not_found@L284`, `review_not_found@L293`, `application_attempt_not_found@L302`, `application_outcome_permission_denied@L311`, `application_input_invalid@L320`, `application_store_unavailable@L330`, `record_references_are_exact_and_typed@L345`
- **Public types:** `PostgresCustomerEnrichmentApplicationAttemptExecutor@L48`, `PostgresCustomerEnrichmentApplicationOutcomeExecutor@L106`
- **Re-exports:** `pub use orchestration::*;`, `pub use owner_application::*;`, `pub use worker::*;`
- **Detected tests (1):** `record_references_are_exact_and_typed@L345`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_SUGGESTION_NOT_FOUND`, `CUSTOMER_ENRICHMENT_REVIEW_DECISION_NOT_FOUND`, `CUSTOMER_ENRICHMENT_APPLICATION_ATTEMPT_NOT_FOUND`, `CUSTOMER_ENRICHMENT_APPLICATION_OUTCOME_PERMISSION_DENIED`, `CUSTOMER_ENRICHMENT_APPLICATION_INPUT_INVALID`, `CUSTOMER_ENRICHMENT_APPLICATION_STORE_UNAVAILABLE`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment`, `crm_customer_enrichment_application_adapter`, `crm_customer_enrichment_capability_adapter`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/orchestration.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-application-composition/src/orchestration.rs) · 723 lines

- **Public declarations (2):** `postgres@L78`, `execute@L100`
- **Internal/trait/helpers (23):** `fmt@L64`, `new@L230`, `load@L234`, `load_required@L262`, `validate_attempt_request@L286`, `validate_committed_attempt@L304`, `build_policy_request@L374`, `build_owner_request@L405`, `owner_result_to_wire@L432`, `policy_denied_outcome@L493`, `build_outcome_request@L502`, `validate_completed_attempt@L565`, `validate_policy_decision@L599`, `decode_output@L614`, `application_attempt_id@L643`, `nonnegative_unix_ms@L651`, `checked_i64@L660`, `validate_internal_id@L664`, `validate_version@L677`, `configured@L690`, `orchestration_input_invalid@L694`, `orchestration_output_invalid@L704`, `orchestration_owner_invalid@L714`
- **Public types:** `PartyDisplayNameApplicationOrchestrationResult@L43`, `CustomerEnrichmentPartyApplicationOrchestrator@L52`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_APPLICATION_ORCHESTRATION_INPUT_INVALID`, `CUSTOMER_ENRICHMENT_APPLICATION_ORCHESTRATION_OUTPUT_INVALID`, `CUSTOMER_ENRICHMENT_APPLICATION_OWNER_RESULT_INVALID`
- **Rust crate references:** `crm_capability_ingress`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment`, `crm_customer_enrichment_application_adapter`, `crm_customer_enrichment_capability_adapter`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/owner_application.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-application-composition/src/owner_application.rs) · 574 lines

- **Public declarations (1):** `new@L40`
- **Internal/trait/helpers (25):** `invoke_owner@L53`, `resolve_stale_target@L86`, `fmt@L114`, `apply@L125`, `owner_call_context@L163`, `validate_application_request@L195`, `validate_owner_outcome@L230`, `ensure_exact_owner_coordinate@L282`, `nonnegative_unix_ms@L293`, `configured@L302`, `owner_configuration_invalid@L306`, `owner_response_invalid@L316`, `new@L344`, `requests@L351`, `get@L360`, `invokes_exact_party_update_with_attempt_derived_lineage@L375`, `maps_live_gateway_denial_to_authorization_outcome@L432`, `resolves_version_conflict_through_governed_party_snapshot@L458`, `rejects_inconsistent_success_evidence@L487`, `application_request@L504`, `attempt_id@L519`, `typed_id@L523`, `party_snapshot@L527`, `successful_outcome@L536`, `typed_test_ids_match_domain_types@L568`
- **Public types:** `GatewayPartyDisplayNameApplicationPort@L33`
- **Detected tests (5):** `invokes_exact_party_update_with_attempt_derived_lineage@L375`, `maps_live_gateway_denial_to_authorization_outcome@L432`, `resolves_version_conflict_through_governed_party_snapshot@L458`, `rejects_inconsistent_success_evidence@L487`, `typed_test_ids_match_domain_types@L568`
- **Error/guard literals:** `PARTIES_PARTY_VERSION_CONFLICT`, `CAPABILITY_PERMISSION_DENIED`, `CUSTOMER_ENRICHMENT_OWNER_APPLICATION_CONFIGURATION_INVALID`, `CUSTOMER_ENRICHMENT_OWNER_APPLICATION_RESPONSE_INVALID`
- **Rust crate references:** `crm_customer_enrichment`, `crm_customer_enrichment_application_adapter`, `crm_customer_enrichment_capability_adapter`, `crm_module_sdk`, `crm_parties_capability_adapter`, `crm_proto_contracts`

### [`src/stale_guard.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-application-composition/src/stale_guard.rs) · 122 lines

- **Public declarations (1):** `new@L16`
- **Internal/trait/helpers (3):** `check@L22`, `store_unavailable@L103`, `state_invalid@L113`
- **Public types:** `PostgresSuggestionMutationGuard@L11`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_SUGGESTION_SUPERSEDED`, `CUSTOMER_ENRICHMENT_STALE_GUARD_STORE_UNAVAILABLE`, `CUSTOMER_ENRICHMENT_STALE_GUARD_STATE_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_module_sdk`

### [`src/worker.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-application-composition/src/worker.rs) · 365 lines

- **Public declarations (3):** `new@L67`, `with_page_size@L75`, `run_cycle@L94`
- **Internal/trait/helpers (10):** `fmt@L52`, `process_delivery@L182`, `run_tenant_cycle@L277`, `decode_reviewed_event@L295`, `module_id@L319`, `event_type@L323`, `worker_identifier_invalid@L327`, `worker_configuration_invalid@L331`, `review_event_invalid@L341`, `worker_identity_and_coordinate_are_canonical@L356`
- **Public types:** `PartyDisplayNameApplicationCycle@L36`, `CustomerEnrichmentPartyApplicationWorker@L44`
- **Detected tests (1):** `worker_identity_and_coordinate_are_canonical@L356`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_APPLICATION_WORKER_CONFIGURATION_INVALID`, `CUSTOMER_ENRICHMENT_APPLICATION_REVIEW_EVENT_INVALID`
- **Rust crate references:** `crm_application_composition`, `crm_capability_ingress`, `crm_capability_runtime`, `crm_core_data`, `crm_core_events`, `crm_customer_enrichment_application_adapter`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_review_adapter`, `crm_module_sdk`, `crm_proto_contracts`

### [`tests/postgres_application_orchestration_process.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-application-composition/tests/postgres_application_orchestration_process.rs) · 419 lines

- **Internal/trait/helpers (9):** `evaluate@L46`, `apply@L64`, `recovers_owner_success_and_skips_completed_replay@L93`, `owner_request@L201`, `apply_request@L220`, `seed@L247`, `context@L289`, `evidence@L310`, `scalar@L416`
- **Detected tests (1):** `recovers_owner_success_and_skips_completed_replay@L93`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_TEST_OWNER_REPLAY_CONFLICT`, `DATABASE_URL`, `ADMIN_DATABASE_URL`
- **Rust crate references:** `crm_capability_ingress`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment`, `crm_customer_enrichment_application_adapter`, `crm_customer_enrichment_application_composition`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_review_adapter`, `crm_module_sdk`, `crm_proto_contracts`

### [`tests/postgres_application_process.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-application-composition/tests/postgres_application_process.rs) · 627 lines

- **Internal/trait/helpers (13):** `postgres_application_attempt_outcome_replay_and_conflict_are_deterministic@L45`, `accepted_review@L262`, `seed_suggestion@L277`, `seed_suggestion_with_suffix@L284`, `seed_review@L314`, `seed_record@L346`, `apply_request@L396`, `outcome_request@L435`, `context@L479`, `suggestion@L505`, `refreshed_suggestion@L514`, `suggestion_at@L523`, `scalar@L621`
- **Detected tests (1):** `postgres_application_attempt_outcome_replay_and_conflict_are_deterministic@L45`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`, `CUSTOMER_ENRICHMENT_APPLICATION_OUTCOME_CONFLICT`, `CUSTOMER_ENRICHMENT_SUGGESTION_SUPERSEDED`
- **Rust crate references:** `crm_capability_ingress`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment`, `crm_customer_enrichment_application_adapter`, `crm_customer_enrichment_application_composition`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_review_adapter`, `crm_module_sdk`, `crm_proto_contracts`

### [`tests/postgres_application_worker_process.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-application-composition/tests/postgres_application_worker_process.rs) · 246 lines

- **Internal/trait/helpers (6):** `is_active@L23`, `accepted_review_worker_checkpoints_and_applies_exactly_once@L33`, `worker_state@L164`, `set_status@L184`, `attempt_count@L226`, `attempt_version@L236`
- **Detected tests (1):** `accepted_review_worker_checkpoints_and_applies_exactly_once@L33`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`
- **Rust crate references:** `crm_application_composition`, `crm_customer_enrichment_application_composition`, `crm_module_sdk`

## 33. `crm-customer-enrichment-capability-adapter` — LATER / B-LATER + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-capability-adapter/src/lib.rs) · 637 lines

- **Public declarations (10):** `prepare_request_dispatch@L118`, `prepare_provider_response@L143`, `capability_definitions@L234`, `capability_definition@L243`, `provider_profile_capability_definition@L247`, `mapping_capability_definition@L256`, `request_create_capability_definition@L265`, `request_cancel_capability_definition@L274`, `request_dispatch_capability_definition@L285`, `provider_response_capability_definition@L296`
- **Internal/trait/helpers (21):** `target@L186`, `plan@L209`, `mutation_definition@L305`, `configured@L330`, `configuration_error@L334`, `dispatch_conflict@L343`, `response_conflict@L352`, `unsupported_capability@L361`, `enrichment_request@L380`, `dispatched_request@L429`, `response_evidence@L443`, `implemented_mutation_catalog_is_exact@L457`, `request_lifecycle_definitions_are_personal_and_registered@L482`, `worker_definitions_are_personal_but_remain_outside_production_catalog@L498`, `created_request_transitions_directly_to_dispatched@L514`, `queued_request_transitions_to_dispatched@L530`, `retryable_request_increments_generation_before_dispatch@L547`, `stale_dispatch_expectation_is_rejected_without_mutation@L564`, `valid_provider_response_creates_receipt_and_binds_request@L581`, `stale_response_expectation_is_rejected_without_mutation@L599`, `invalid_response_evidence_leaves_request_dispatched@L619`
- **Public types:** `DispatchExpectation@L88`, `ResponseExpectation@L95`, `ProviderResponseEvidence@L102`, `CustomerEnrichmentProviderProfileCapabilityPlanner@L183`
- **Re-exports:** `pub use mapping_planner::{`, `pub use mapping_reference_planner::CustomerEnrichmentMappingReferencePlanner;`, `pub use mapping_snapshot::mapping_from_snapshot;`, `pub use provider_profile_planner::{`, `pub use provider_profile_snapshot::provider_profile_from_snapshot;`, `pub use request_cancel_planner::*;`, `pub use request_planner::*;`, `pub use request_reference_planner::CustomerEnrichmentRequestReferencePlanner;`, `pub use request_snapshot::enrichment_request_from_snapshot;`, `pub use semantic_validator::CustomerEnrichmentCapabilitySemanticValidator;`
- **Detected tests (10):** `implemented_mutation_catalog_is_exact@L457`, `request_lifecycle_definitions_are_personal_and_registered@L482`, `worker_definitions_are_personal_but_remain_outside_production_catalog@L498`, `created_request_transitions_directly_to_dispatched@L514`, `queued_request_transitions_to_dispatched@L530`, `retryable_request_increments_generation_before_dispatch@L547`, `stale_dispatch_expectation_is_rejected_without_mutation@L564`, `valid_provider_response_creates_receipt_and_binds_request@L581`, `stale_response_expectation_is_rejected_without_mutation@L599`, `invalid_response_evidence_leaves_request_dispatched@L619`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_CONFIGURATION_INVALID`, `CUSTOMER_ENRICHMENT_REQUEST_DISPATCH_CONFLICT`, `CUSTOMER_ENRICHMENT_RESPONSE_EXPECTATION_CONFLICT`, `CUSTOMER_ENRICHMENT_CAPABILITY_UNSUPPORTED`, `CUSTOMER_ENRICHMENT_RESPONSE_DIGEST_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment`, `crm_module_sdk`

### [`src/mapping_planner.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-capability-adapter/src/mapping_planner.rs) · 333 lines

- **Public declarations (6):** `mapping_from_definition@L100`, `mapping_to_wire@L129`, `provider_profile_version_id_from_external@L151`, `mapping_persisted_contract@L169`, `mapping_persisted_payload@L180`, `mapping_record_ref@L190`
- **Internal/trait/helpers (15):** `target@L25`, `plan@L39`, `target_field_from_wire@L198`, `target_field_to_wire@L208`, `normalization_from_wire@L214`, `normalization_to_wire@L226`, `decode_request@L234`, `ensure_definition@L243`, `invalid_provider_profile_id@L258`, `invalid_plan@L265`, `profile_id@L279`, `definition@L283`, `wire_definition_round_trips_through_domain@L298`, `provider_profile_identity_format_is_strict@L309`, `persisted_contract_is_exact_and_confidential@L322`
- **Public types:** `CustomerEnrichmentMappingCapabilityPlanner@L22`
- **Detected tests (3):** `wire_definition_round_trips_through_domain@L298`, `provider_profile_identity_format_is_strict@L309`, `persisted_contract_is_exact_and_confidential@L322`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_MAPPING_CAPABILITY_PLAN_INVALID`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/mapping_reference_planner.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-capability-adapter/src/mapping_reference_planner.rs) · 31 lines

- **Internal/trait/helpers (2):** `target@L14`, `plan@L22`
- **Public types:** `CustomerEnrichmentMappingReferencePlanner@L11`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_module_sdk`

### [`src/mapping_snapshot.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-capability-adapter/src/mapping_snapshot.rs) · 38 lines

- **Public declarations (1):** `mapping_from_snapshot@L7`
- **Internal/trait/helpers (1):** `invalid_snapshot@L29`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_MAPPING_PERSISTED_STATE_INVALID`
- **Rust crate references:** `crm_customer_enrichment`, `crm_module_sdk`

### [`src/provider_profile_planner.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-capability-adapter/src/provider_profile_planner.rs) · 341 lines

- **Public declarations (5):** `provider_profile_from_definition@L103`, `provider_profile_to_wire@L160`, `provider_profile_persisted_contract@L193`, `provider_profile_persisted_payload@L204`, `provider_profile_record_ref@L214`
- **Internal/trait/helpers (12):** `target@L26`, `plan@L40`, `target_field_from_wire@L224`, `target_field_to_wire@L234`, `non_negative_timestamp@L240`, `decode_request@L246`, `ensure_definition@L255`, `invalid_plan@L270`, `definition@L284`, `wire_definition_round_trips_through_domain@L303`, `negative_timestamps_and_unspecified_policy_are_rejected@L316`, `persisted_contract_is_exact_and_confidential@L327`
- **Public types:** `CustomerEnrichmentProviderProfileCapabilityPlanner@L23`
- **Detected tests (3):** `wire_definition_round_trips_through_domain@L303`, `negative_timestamps_and_unspecified_policy_are_rejected@L316`, `persisted_contract_is_exact_and_confidential@L327`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_PROVIDER_PROFILE_CAPABILITY_PLAN_INVALID`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/provider_profile_snapshot.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-capability-adapter/src/provider_profile_snapshot.rs) · 40 lines

- **Public declarations (1):** `provider_profile_from_snapshot@L7`
- **Internal/trait/helpers (1):** `invalid_snapshot@L31`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_PROVIDER_PROFILE_PERSISTED_STATE_INVALID`
- **Rust crate references:** `crm_customer_enrichment`, `crm_module_sdk`

### [`src/request_cancel_planner.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-capability-adapter/src/request_cancel_planner.rs) · 183 lines

- **Public declarations (1):** `cancel_request_record_ref@L114`
- **Internal/trait/helpers (6):** `target@L33`, `plan@L45`, `ensure_definition@L141`, `request_started_at_unix_ms@L156`, `request_not_found@L165`, `invalid_plan@L174`
- **Public types:** `CustomerEnrichmentRequestCancelPlanner@L30`
- **Re-exports:** `pub use dispatch_planner::CustomerEnrichmentRequestDispatchPlanner;`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_REQUEST_NOT_FOUND`, `CUSTOMER_ENRICHMENT_REQUEST_CANCEL_PLAN_INVALID`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/request_dispatch_planner.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-capability-adapter/src/request_dispatch_planner.rs) · 351 lines

- **Internal/trait/helpers (14):** `target@L40`, `plan@L52`, `dispatch_command@L177`, `dispatch_request_record_ref@L188`, `dispatch_record_ref_from_command@L192`, `dispatch_status@L215`, `provider_usage_entry_persisted_payload@L229`, `provider_usage_entry_record_ref@L246`, `provider_usage_entry_to_wire@L254`, `request_started_at_unix_ms@L289`, `checked_i64@L298`, `ensure_definition@L302`, `request_not_found@L317`, `invalid_plan@L326`
- **Public types:** `CustomerEnrichmentRequestDispatchPlanner@L37`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_REQUEST_NOT_FOUND`, `CUSTOMER_ENRICHMENT_PROVIDER_DISPATCH_PLAN_INVALID`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/request_planner.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-capability-adapter/src/request_planner.rs) · 472 lines

- **Public declarations (6):** `enrichment_request_from_create_request@L132`, `enrichment_request_to_wire@L225`, `mapping_version_id_from_external@L278`, `enrichment_request_persisted_contract@L294`, `enrichment_request_persisted_payload@L305`, `enrichment_request_record_ref@L315`
- **Internal/trait/helpers (15):** `target@L45`, `plan@L58`, `request_party_link_payload@L325`, `ensure_definition@L340`, `request_started_at_unix_ms@L355`, `positive_u64@L364`, `checked_i64@L371`, `target_field_from_wire@L375`, `target_field_to_wire@L385`, `request_status_to_wire@L391`, `configured_relationship_type@L414`, `configured_record_type@L418`, `invalid_mapping_version_id@L422`, `invalid_plan@L429`, `configuration_error@L439`
- **Public types:** `CustomerEnrichmentRequestCreateCapabilityPlanner@L42`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_REQUEST_CREATE_PLAN_INVALID`, `CUSTOMER_ENRICHMENT_REQUEST_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/request_reference_planner.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-capability-adapter/src/request_reference_planner.rs) · 696 lines

- **Internal/trait/helpers (31):** `target@L49`, `plan@L61`, `request_create_target@L79`, `plan_request_create@L87`, `provider_response_target@L105`, `plan_provider_response@L121`, `provider_usage_events@L278`, `provider_response_request_record_ref@L306`, `provider_response_command@L312`, `provider_response_record_ref_from_command@L323`, `provider_response_evidence@L346`, `provider_usage_entries@L377`, `provider_response_receipt_persisted_payload@L416`, `provider_usage_entry_persisted_payload@L433`, `provider_response_receipt_record_ref@L450`, `provider_usage_entry_record_ref@L460`, `provider_response_receipt_to_wire@L468`, `provider_usage_entry_to_wire@L501`, `provider_response_class@L535`, `provider_response_class_to_wire@L552`, `provider_usage_kind_to_wire@L565`, `ensure_response_definition@L607`, `non_negative_time@L622`, `checked_i64@L628`, `party_record_ref@L632`, `target_unavailable@L640`, `stale_target@L649`, `response_request_not_found@L659`, `response_plan_invalid@L668`, `unsupported_reference_capability@L678`, `configuration_error@L687`
- **Public types:** `CustomerEnrichmentRequestReferencePlanner@L46`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_REQUEST_TARGET_UNAVAILABLE`, `CUSTOMER_ENRICHMENT_REQUEST_TARGET_STALE`, `CUSTOMER_ENRICHMENT_REQUEST_NOT_FOUND`, `CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_PLAN_INVALID`, `CUSTOMER_ENRICHMENT_REFERENCE_CAPABILITY_UNSUPPORTED`, `CUSTOMER_ENRICHMENT_REQUEST_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/request_snapshot.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-capability-adapter/src/request_snapshot.rs) · 42 lines

- **Public declarations (1):** `enrichment_request_from_snapshot@L9`
- **Internal/trait/helpers (1):** `invalid_snapshot@L33`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_REQUEST_PERSISTED_STATE_INVALID`
- **Rust crate references:** `crm_customer_enrichment`, `crm_module_sdk`

### [`src/semantic_validator.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-capability-adapter/src/semantic_validator.rs) · 123 lines

- **Public declarations (1):** `new@L21`
- **Internal/trait/helpers (8):** `validate@L27`, `decode_mapping@L67`, `module_id@L79`, `provider_profile_record_type@L83`, `reference_unavailable@L87`, `target_field_unsupported@L96`, `unsupported_capability@L105`, `configuration_error@L114`
- **Public types:** `CustomerEnrichmentCapabilitySemanticValidator@L16`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_MAPPING_PROVIDER_PROFILE_UNAVAILABLE`, `CUSTOMER_ENRICHMENT_MAPPING_TARGET_FIELD_UNSUPPORTED`, `CUSTOMER_ENRICHMENT_COMPOSITION_CAPABILITY_UNSUPPORTED`, `CUSTOMER_ENRICHMENT_COMPOSITION_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_module_sdk`, `crm_proto_contracts`, `crm_customer_enrichment`

### [`tests/provider_dispatch_atomic_plan.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-capability-adapter/tests/provider_dispatch_atomic_plan.rs) · 225 lines

- **Internal/trait/helpers (6):** `created_request_dispatch_is_one_valid_atomic_batch@L23`, `retryable_failure_dispatch_increments_generation_atomically@L73`, `stale_retry_generation_is_rejected_before_batch_creation@L92`, `fixture@L100`, `domain_request@L166`, `status_to_wire@L215`
- **Detected tests (3):** `created_request_dispatch_is_one_valid_atomic_batch@L23`, `retryable_failure_dispatch_increments_generation_atomically@L73`, `stale_retry_generation_is_rejected_before_batch_creation@L92`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_REQUEST_DISPATCH_CONFLICT`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_module_sdk`, `crm_proto_contracts`

### [`tests/provider_response_atomic_plan.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-capability-adapter/tests/provider_response_atomic_plan.rs) · 237 lines

- **Internal/trait/helpers (6):** `metered_response_is_one_valid_atomic_batch@L24`, `zero_metered_units_emit_only_response_received_usage@L75`, `stale_generation_is_rejected_before_batch_creation@L95`, `zero_digest_is_rejected_before_batch_creation@L107`, `fixture@L115`, `dispatched_request@L179`
- **Detected tests (4):** `metered_response_is_one_valid_atomic_batch@L24`, `zero_metered_units_emit_only_response_received_usage@L75`, `stale_generation_is_rejected_before_batch_creation@L95`, `zero_digest_is_rejected_before_batch_creation@L107`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_RESPONSE_EXPECTATION_CONFLICT`, `CUSTOMER_ENRICHMENT_RESPONSE_DIGEST_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_module_sdk`, `crm_proto_contracts`

### [`tests/request_create_planner.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-capability-adapter/tests/request_create_planner.rs) · 132 lines

- **Internal/trait/helpers (7):** `provider_profile_version_id@L16`, `mapping_version_id@L20`, `capability_request@L24`, `party_snapshot@L80`, `target_locks_the_exact_party_aggregate@L92`, `stale_party_version_is_rejected_before_write_planning@L108`, `exact_party_version_produces_one_atomic_personal_request_plan@L119`
- **Detected tests (3):** `target_locks_the_exact_party_aggregate@L92`, `stale_party_version_is_rejected_before_write_planning@L108`, `exact_party_version_produces_one_atomic_personal_request_plan@L119`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_REQUEST_TARGET_STALE`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment_capability_adapter`, `crm_module_sdk`, `crm_proto_contracts`

## 34. `crm-customer-enrichment-capability-composition` — LATER / B-LATER + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-capability-composition/src/lib.rs) · 608 lines

- **Public declarations (1):** `new@L72`
- **Internal/trait/helpers (24):** `execute_create@L90`, `execute_cancel@L122`, `load_and_validate_definitions@L147`, `validate_profile_policy@L200`, `validate_current_party_visibility@L253`, `validate_party@L273`, `validate_consent@L323`, `fmt@L370`, `execute@L384`, `authorize_query@L401`, `consent_get_query_request@L423`, `validate_consent_authorization@L464`, `module_id@L505`, `record_type@L509`, `nonnegative_u64@L513`, `definition_unavailable@L522`, `definition_invalid@L531`, `policy_denied@L541`, `consent_denied@L551`, `target_unavailable@L561`, `enrichment_request_not_found@L570`, `stale_target@L579`, `contract_invalid@L588`, `configuration_error@L597`
- **Public types:** `CustomerEnrichmentCapabilityExecutor@L62`
- **Re-exports:** `pub use production_contribution::*;`, `pub use reference_guards::*;`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_PARTY_PERMISSION_DENIED`, `CUSTOMER_ENRICHMENT_CONSENT_PERMISSION_DENIED`, `CUSTOMER_ENRICHMENT_REQUEST_DEFINITION_UNAVAILABLE`, `CUSTOMER_ENRICHMENT_REQUEST_DEFINITION_INVALID`, `CUSTOMER_ENRICHMENT_REQUEST_POLICY_DENIED`, `CUSTOMER_ENRICHMENT_REQUEST_CONSENT_DENIED`, `QUERY_RESOURCE_NOT_FOUND`, `CUSTOMER_ENRICHMENT_REQUEST_NOT_FOUND`, `CUSTOMER_ENRICHMENT_REQUEST_TARGET_STALE`, `CUSTOMER_ENRICHMENT_REQUEST_CONTRACT_INVALID`, `CUSTOMER_ENRICHMENT_REQUEST_COMPOSITION_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_consents_capability_adapter`, `crm_consents_query_adapter`, `crm_core_data`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_module_sdk`, `crm_parties_query_adapter`, `crm_proto_contracts`, `crm_query_runtime`

### [`src/production_contribution.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-capability-composition/src/production_contribution.rs) · 194 lines

- **Public declarations (3):** `mutation_capability_definitions@L49`, `query_capability_definitions@L53`, `build_contribution@L60`
- **Internal/trait/helpers (3):** `add_queries@L153`, `cursor@L173`, `production_composition_error@L185`
- **Public types:** `CustomerEnrichmentProductionDependencies@L41`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_CURSOR_CONFIGURATION_INVALID`, `CUSTOMER_ENRICHMENT_PRODUCTION_COMPOSITION_INVALID`
- **Rust crate references:** `crm_application_composition`, `crm_capability_runtime`, `crm_consents_query_adapter`, `crm_core_data`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_query_adapter`, `crm_customer_enrichment_request_list_query_adapter`, `crm_customer_enrichment_suggestion_query_adapter`, `crm_module_sdk`, `crm_parties_query_adapter`, `crm_query_runtime`

### [`src/reference_guards.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-capability-composition/src/reference_guards.rs) · 211 lines

- **Internal/trait/helpers (9):** `check@L22`, `check@L90`, `mapping_reference_unavailable@L145`, `mapping_target_unsupported@L154`, `mapping_store_unavailable@L163`, `mapping_state_invalid@L173`, `request_target_unavailable@L183`, `request_target_stale@L192`, `request_store_unavailable@L202`
- **Public types:** `PostgresCustomerEnrichmentMappingReferenceGuard@L19`, `PostgresCustomerEnrichmentRequestPartyGuard@L87`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_MAPPING_PROVIDER_PROFILE_UNAVAILABLE`, `CUSTOMER_ENRICHMENT_MAPPING_TARGET_FIELD_UNSUPPORTED`, `CUSTOMER_ENRICHMENT_MAPPING_REFERENCE_STORE_UNAVAILABLE`, `CUSTOMER_ENRICHMENT_MAPPING_PROVIDER_PROFILE_STATE_INVALID`, `CUSTOMER_ENRICHMENT_REQUEST_TARGET_UNAVAILABLE`, `CUSTOMER_ENRICHMENT_REQUEST_TARGET_STALE`, `CUSTOMER_ENRICHMENT_REQUEST_TARGET_STORE_UNAVAILABLE`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_module_sdk`, `crm_proto_contracts`

## 35. `crm-customer-enrichment-materialization-adapter` — LATER / B-LATER + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-materialization-adapter/src/lib.rs) · 615 lines

- **Public declarations (2):** `suggestion_materialization_capability_definition@L50`, `new@L82`
- **Internal/trait/helpers (21):** `materialization_contract@L69`, `target@L96`, `plan@L110`, `materialization_command@L258`, `ensure_receipt_ref@L269`, `request_record_ref@L293`, `candidate_from_wire@L316`, `suggestion_persisted_payload@L385`, `suggestion_record_ref@L400`, `suggestion_to_wire@L408`, `ensure_definition@L475`, `request_started_at_unix_ms@L491`, `positive_u64@L503`, `non_negative_u64@L514`, `target_field_from_wire@L519`, `target_field_to_wire@L529`, `suggestion_status_to_wire@L535`, `checked_i64@L552`, `configured@L593`, `request_not_found@L597`, `materialization_plan_invalid@L606`
- **Public types:** `CustomerEnrichmentSuggestionMaterializationPlanner@L75`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_MATERIALIZATION_RECEIPT_CONFLICT`, `CUSTOMER_ENRICHMENT_REQUEST_NOT_FOUND`, `CUSTOMER_ENRICHMENT_MATERIALIZATION_PLAN_INVALID`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_module_sdk`, `crm_proto_contracts`

### [`tests/suggestion_materialization_atomic_plan.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-materialization-adapter/tests/suggestion_materialization_atomic_plan.rs) · 334 lines

- **Internal/trait/helpers (8):** `success_materialization_is_one_atomic_batch@L24`, `no_match_updates_only_the_request@L87`, `wrong_receipt_reference_is_rejected_before_locking@L113`, `stale_candidate_target_is_rejected_before_batch_creation@L129`, `planner@L156`, `fixture@L164`, `success_candidates@L299`, `candidate@L306`
- **Detected tests (4):** `success_materialization_is_one_atomic_batch@L24`, `no_match_updates_only_the_request@L87`, `wrong_receipt_reference_is_rejected_before_locking@L113`, `stale_candidate_target_is_rejected_before_batch_creation@L129`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_MATERIALIZATION_RECEIPT_CONFLICT`, `CUSTOMER_ENRICHMENT_CANDIDATE_TARGET_CONFLICT`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_materialization_adapter`, `crm_module_sdk`, `crm_proto_contracts`

## 36. `crm-customer-enrichment-materialization-composition` — LATER / B-LATER + D? adapter

### [`src/candidate_evidence.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-materialization-composition/src/candidate_evidence.rs) · 254 lines

- **Public declarations (2):** `new@L43`, `decode_provider_suggestion_candidate_evidence@L81`
- **Internal/trait/helpers (13):** `load@L30`, `fmt@L49`, `load@L60`, `validate_candidate_evidence_lineage@L108`, `evidence_unavailable@L132`, `evidence_invalid@L142`, `evidence_conflict@L152`, `artifact@L167`, `command@L186`, `exact_finalized_candidate_evidence_decodes@L199`, `request_or_receipt_lineage_mismatch_fails_closed@L211`, `raw_or_wrongly_classified_artifact_is_rejected@L226`, `candidate_count_is_bounded_before_planner_execution@L239`
- **Public types:** `ProviderSuggestionCandidateEvidenceRequest@L21`, `ProviderSuggestionCandidateEvidenceSourcePort@L29`, `GovernedFileProviderSuggestionCandidateEvidenceSource@L38`
- **Detected tests (4):** `exact_finalized_candidate_evidence_decodes@L199`, `request_or_receipt_lineage_mismatch_fails_closed@L211`, `raw_or_wrongly_classified_artifact_is_rejected@L226`, `candidate_count_is_bounded_before_planner_execution@L239`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_SUGGESTION_EVIDENCE_UNAVAILABLE`, `CUSTOMER_ENRICHMENT_SUGGESTION_EVIDENCE_INVALID`, `CUSTOMER_ENRICHMENT_SUGGESTION_EVIDENCE_CONFLICT`
- **Rust crate references:** `crm_core_files`, `crm_customer_enrichment_capability_adapter`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-materialization-composition/src/lib.rs) · 257 lines

- **Public declarations (2):** `new@L48`, `execute@L52`
- **Internal/trait/helpers (9):** `receipt_from_snapshot@L157`, `record_ref@L195`, `receipt_not_found@L209`, `profile_not_found@L216`, `mapping_not_found@L223`, `dependency_not_found@L230`, `dependency_conflict@L234`, `store_read_failed@L238`, `invalid_receipt_snapshot@L248`
- **Public types:** `PostgresCustomerEnrichmentSuggestionMaterializationWorker@L43`
- **Re-exports:** `pub use candidate_evidence::*;`, `pub use process::*;`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_MATERIALIZATION_REQUEST_RECEIPT_CONFLICT`, `CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_RECEIPT_NOT_FOUND`, `CUSTOMER_ENRICHMENT_PROVIDER_PROFILE_NOT_FOUND`, `CUSTOMER_ENRICHMENT_MAPPING_NOT_FOUND`, `CUSTOMER_ENRICHMENT_MATERIALIZATION_STORE_UNAVAILABLE`, `CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_RECEIPT_PERSISTED_STATE_INVALID`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_materialization_adapter`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/process.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-materialization-composition/src/process.rs) · 522 lines

- **Public declarations (3):** `new@L102`, `with_page_size@L111`, `run_cycle@L132`
- **Internal/trait/helpers (21):** `execute@L48`, `execute@L57`, `fmt@L86`, `process_delivery@L218`, `run_tenant_cycle@L325`, `decode_response_recorded_event@L344`, `materialization_context@L371`, `materialization_request_identity@L408`, `hash_frame@L417`, `hex@L422`, `module_id@L432`, `event_type@L436`, `configured@L440`, `configuration_identifier@L444`, `configuration_invalid@L448`, `response_event_invalid@L458`, `canonical_choice_pending@L467`, `canonical_choice_mismatch@L476`, `success_evidence_missing@L485`, `exact_response_event_coordinate_is_internal_and_versioned@L499`, `worker_identity_and_projection_are_stable@L511`
- **Public types:** `SuggestionMaterializationExecutorPort@L47`, `MaterializationProcessCycle@L68`, `CustomerEnrichmentMaterializationProcessWorker@L77`
- **Detected tests (2):** `exact_response_event_coordinate_is_internal_and_versioned@L499`, `worker_identity_and_projection_are_stable@L511`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_MATERIALIZATION_PROCESS_CONFIGURATION_INVALID`, `CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_EVENT_INVALID`, `CUSTOMER_ENRICHMENT_PROVIDER_CANONICAL_CHOICE_PENDING`, `CUSTOMER_ENRICHMENT_PROVIDER_CANONICAL_CHOICE_MISMATCH`, `CUSTOMER_ENRICHMENT_SUCCESS_EVIDENCE_MISSING`
- **Rust crate references:** `crm_application_composition`, `crm_capability_ingress`, `crm_capability_runtime`, `crm_core_data`, `crm_core_events`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_materialization_adapter`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/stale_guard.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-materialization-composition/src/stale_guard.rs) · 164 lines

- **Public declarations (1):** `new@L27`
- **Internal/trait/helpers (2):** `check@L94`, `store_unavailable@L155`
- **Public types:** `PostgresSuggestionMaterializationGuard@L22`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_APPLICATION_IN_PROGRESS`, `CUSTOMER_ENRICHMENT_MATERIALIZATION_GUARD_STORE_UNAVAILABLE`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_module_sdk`, `crm_proto_contracts`

### [`tests/postgres_materialization_event_process.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-materialization-composition/tests/postgres_materialization_event_process.rs) · 1179 lines

- **Internal/trait/helpers (29):** `response_event_waits_for_finalized_evidence_then_materializes_once@L79`, `apply_recorded_provider_outcome@L259`, `materialization_process@L305`, `fixture@L336`, `seed_dependencies@L415`, `upload_candidate_evidence@L498`, `prior_suggestion@L547`, `accepted_review@L609`, `seed_suggestion@L624`, `seed_review@L649`, `application_request@L676`, `terminal_outcome_request@L708`, `candidate@L743`, `seed_record@L778`, `receipt_persisted_payload@L814`, `receipt_record_ref@L829`, `receipt_to_wire@L837`, `seed_context@L862`, `artifact_context@L872`, `execution_context@L882`, `evidence_counts@L908`, `request_version@L948`, `application_attempt_count@L959`, `application_attempt_version@L969`, `suggestion_count@L980`, `lineage_mismatched_evidence_fails_checkpoint_before_materialization@L991`, `load@L1087`, `execute@L1112`, `upload_lineage_mismatched_candidate_evidence@L1131`
- **Detected tests (2):** `response_event_waits_for_finalized_evidence_then_materializes_once@L79`, `lineage_mismatched_evidence_fails_checkpoint_before_materialization@L991`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`, `CUSTOMER_ENRICHMENT_PROVIDER_CANONICAL_CHOICE_PENDING`, `CUSTOMER_ENRICHMENT_SUGGESTION_EVIDENCE_UNAVAILABLE`, `CUSTOMER_ENRICHMENT_APPLICATION_IN_PROGRESS`, `CUSTOMER_ENRICHMENT_SUGGESTION_EVIDENCE_CONFLICT`, `PROJECTION_CHECKPOINT_FAILED`, `TEST_EVIDENCE_SOURCE_MUST_NOT_RUN`, `TEST_MATERIALIZATION_EXECUTOR_MUST_NOT_RUN`
- **Rust crate references:** `crm_capability_ingress`, `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_core_events`, `crm_core_files`, `crm_customer_enrichment`, `crm_customer_enrichment_application_adapter`, `crm_customer_enrichment_application_composition`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_materialization_composition`, `crm_customer_enrichment_review_adapter`, `crm_module_sdk`, `crm_proto_contracts`

### [`tests/postgres_materialization_malformed_process.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-materialization-composition/tests/postgres_materialization_malformed_process.rs) · 7 lines


### [`tests/postgres_materialization_process.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-materialization-composition/tests/postgres_materialization_process.rs) · 536 lines

- **Internal/trait/helpers (13):** `postgres_materialization_reads_exact_dependencies_and_replays_without_duplicates@L44`, `fixture@L157`, `seed_dependencies@L236`, `seed_record@L335`, `materialization_request@L372`, `candidate@L407`, `receipt_persisted_payload@L436`, `receipt_record_ref@L451`, `receipt_to_wire@L459`, `seed_context@L484`, `read_context@L494`, `execution_context@L504`, `scalar@L530`
- **Detected tests (1):** `postgres_materialization_reads_exact_dependencies_and_replays_without_duplicates@L44`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`
- **Rust crate references:** `crm_capability_ingress`, `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_materialization_adapter`, `crm_customer_enrichment_materialization_composition`, `crm_module_sdk`, `crm_proto_contracts`

### [`tests/support/materialization_malformed/fixture_domain.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-materialization-composition/tests/support/materialization_malformed/fixture_domain.rs) · 242 lines

- **Internal/trait/helpers (4):** `fixture@L17`, `seed_dependencies@L99`, `apply_recorded_provider_outcome@L182`, `materialization_process@L228`

### [`tests/support/materialization_malformed/fixture_store.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-materialization-composition/tests/support/materialization_malformed/fixture_store.rs) · 249 lines

- **Internal/trait/helpers (12):** `seed_record@L10`, `receipt_persisted_payload@L49`, `receipt_record_ref@L64`, `receipt_to_wire@L72`, `seed_context@L99`, `artifact_context@L109`, `execution_context@L119`, `evidence_counts@L145`, `request_version@L185`, `suggestion_count@L196`, `load@L212`, `execute@L234`
- **Error/guard literals:** `TEST_EVIDENCE_SOURCE_MUST_NOT_RUN`, `TEST_MATERIALIZATION_EXECUTOR_MUST_NOT_RUN`
- **Rust crate references:** `crm_module_sdk`

### [`tests/support/materialization_malformed/imports.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-materialization-composition/tests/support/materialization_malformed/imports.rs) · 58 lines

- **Rust crate references:** `crm_capability_ingress`, `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_core_events`, `crm_core_files`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_materialization_composition`, `crm_module_sdk`, `crm_proto_contracts`

### [`tests/support/materialization_malformed/test.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-materialization-composition/tests/support/materialization_malformed/test.rs) · 129 lines

- **Internal/trait/helpers (2):** `malformed_finalized_evidence_fails_checkpoint_before_materialization@L2`, `upload_malformed_candidate_evidence@L91`
- **Detected tests (1):** `malformed_finalized_evidence_fails_checkpoint_before_materialization@L2`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`, `CUSTOMER_ENRICHMENT_SUGGESTION_EVIDENCE_INVALID`, `PROJECTION_CHECKPOINT_FAILED`

## 37. `crm-customer-enrichment-privacy-scope-adapter` — END / B-END + D? adapter

### [`src/contract.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-privacy-scope-adapter/src/contract.rs) · 104 lines

- **Public declarations (8):** `customer_enrichment_privacy_scope_definition@L43`, `validate_definition@L59`, `module_id@L69`, `capability_id@L73`, `capability_version@L77`, `schema_id@L81`, `schema_version@L85`, `output_descriptor_hash@L89`
- **Internal/trait/helpers (1):** `payload_contract@L93`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_PRIVACY_SCOPE_DEFINITION_MISMATCH`
- **Rust crate references:** `crm_capability_runtime`, `crm_customer_enrichment_capability_adapter`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/digest.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-privacy-scope-adapter/src/digest.rs) · 44 lines

- **Public declarations (2):** `encode_hex@L3`, `decode_hex@L13`
- **Internal/trait/helpers (2):** `nibble@L30`, `cursor_invalid@L38`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_PRIVACY_SCOPE_CURSOR_INVALID`
- **Rust crate references:** `crm_module_sdk`

### [`src/errors.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-privacy-scope-adapter/src/errors.rs) · 196 lines

- **Public declarations (13):** `configured@L6`, `map_request_contract_error@L18`, `map_common_lineage_error@L37`, `map_canonical_party_claim_error@L85`, `invalid_contract@L111`, `invalid_contract_with_reference@L115`, `lineage_invalid@L123`, `stored_state_invalid@L137`, `relationship_state_invalid@L147`, `association_state_invalid@L157`, `limit_exceeded@L167`, `topology_invalid@L177`, `database_unavailable@L187`
- **Internal/trait/helpers (1):** `lineage_contract_invalid@L67`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_PRIVACY_SCOPE_CONFIGURATION_INVALID`, `CUSTOMER_ENRICHMENT_PRIVACY_SCOPE_REQUEST_BINDING_MISMATCH`, `CUSTOMER_ENRICHMENT_PRIVACY_SCOPE_INPUT_CONTRACT_MISMATCH`, `CUSTOMER_ENRICHMENT_PRIVACY_SCOPE_INPUT_HASH_MISMATCH`, `TENANT_MISMATCH`, `CUSTOMER_ENRICHMENT_PRIVACY_SCOPE_CASE_ID_INVALID`, `PARTY_INVALID`, `CUSTOMER_ENRICHMENT_PRIVACY_SCOPE_PARTY_INVALID`, `GENERATION_INVALID`, `REGISTRY_INVALID`, `CUSTOMER_ENRICHMENT_PRIVACY_SCOPE_REGISTRY_UNAVAILABLE`, `REGISTRY_MISMATCH`, `PURPOSE_INVALID`, `REQUEST_TIME_INVALID`, `PAGE_SIZE_INVALID`, `CUSTOMER_ENRICHMENT_PRIVACY_SCOPE_TENANT_MISMATCH`, `CUSTOMER_ENRICHMENT_PRIVACY_SCOPE_GENERATION_INVALID`, `CUSTOMER_ENRICHMENT_PRIVACY_SCOPE_REGISTRY_INVALID`, `CUSTOMER_ENRICHMENT_PRIVACY_SCOPE_REGISTRY_MISMATCH`, `CUSTOMER_ENRICHMENT_PRIVACY_SCOPE_PURPOSE_INVALID`, `CUSTOMER_ENRICHMENT_PRIVACY_SCOPE_REQUEST_TIME_INVALID`, `CUSTOMER_ENRICHMENT_PRIVACY_SCOPE_PAGE_SIZE_INVALID`, `CUSTOMER_ENRICHMENT_PRIVACY_SCOPE_LINEAGE_INVALID`, `CUSTOMER_ENRICHMENT_PRIVACY_SCOPE_STORED_STATE_INVALID`, `CUSTOMER_ENRICHMENT_PRIVACY_SCOPE_RELATIONSHIP_STATE_INVALID`, `CUSTOMER_ENRICHMENT_PRIVACY_SCOPE_ASSOCIATION_STATE_INVALID`, `CUSTOMER_ENRICHMENT_PRIVACY_SCOPE_LIMIT_EXCEEDED`, `CUSTOMER_ENRICHMENT_PRIVACY_SCOPE_TOPOLOGY_INVALID`, `CUSTOMER_ENRICHMENT_PRIVACY_SCOPE_UNAVAILABLE`
- **Rust crate references:** `crm_customer_privacy_owner_scope_support`, `crm_module_sdk`

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-privacy-scope-adapter/src/lib.rs) · 42 lines

- **Re-exports:** `pub use contract::{`, `pub use owner_action::{`, `pub use postgres::CustomerEnrichmentPrivacyScopeQueryAdapter;`

### [`src/owner_action.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-privacy-scope-adapter/src/owner_action.rs) · 252 lines

- **Public declarations (2):** `customer_enrichment_privacy_action_definition@L35`, `customer_enrichment_privacy_action_planner@L39`
- **Internal/trait/helpers (15):** `owner_module_id@L45`, `capability_id@L49`, `supports_resource_type@L53`, `anonymize@L66`, `deletion_tombstone@L75`, `validate_resource@L85`, `validate_response_receipt@L111`, `validate_response_conflict@L133`, `validate_provider_usage@L161`, `response_receipt_contract@L183`, `provider_usage_contract@L194`, `unsupported@L205`, `stored_state_invalid@L209`, `publishes_the_frozen_owner_action_coordinate@L224`, `supports_exactly_the_seven_discovered_resource_families@L236`
- **Public types:** `CustomerEnrichmentPrivacyActionPlanner@L29`, `CustomerEnrichmentPrivacyActionPolicy@L33`
- **Detected tests (2):** `publishes_the_frozen_owner_action_coordinate@L224`, `supports_exactly_the_seven_discovered_resource_families@L236`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_PRIVACY_STORED_STATE_INVALID`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_customer_enrichment`, `crm_customer_enrichment_application_adapter`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_provider_process_composition`, `crm_customer_enrichment_review_adapter`, `crm_customer_privacy_owner_scope_support`, `crm_module_sdk`

### [`src/postgres.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-privacy-scope-adapter/src/postgres.rs) · 1570 lines

- **Public declarations (1):** `new@L90`
- **Internal/trait/helpers (37):** `fmt@L81`, `execute_query@L94`, `execute@L132`, `new@L238`, `resolves_to_subject@L245`, `new@L310`, `charge@L314`, `read_customer_enrichment_page@L328`, `validate_relationships@L505`, `validate_associations@L560`, `strict_definitions@L835`, `strict_requests@L877`, `strict_receipts@L894`, `strict_conflicts@L933`, `strict_suggestions@L959`, `strict_reviews@L984`, `strict_applications@L1007`, `strict_usage_entries@L1037`, `append_request_resources@L1076`, `append_receipt_resources@L1092`, `append_conflict_resources@L1108`, `append_suggestion_resources@L1124`, `append_review_resources@L1140`, `append_application_resources@L1168`, `append_usage_resources@L1198`, `suggestion_request_id@L1214`, `suggestion_receipt_id@L1225`, `resource_after_cursor@L1236`, `provider_response_receipt_persisted_contract@L1250`, `provider_usage_entry_persisted_contract@L1261`, `versioned@L1272`, `map_owner_error@L1280`, `load_request_relationship_rows@L1299`, `load_record_rows@L1405`, `decode_stored_row@L1467`, `strict_snapshot@L1508`, `positive_version@L1567`
- **Public types:** `CustomerEnrichmentPrivacyScopeQueryAdapter@L76`
- **Error/guard literals:** `IDENTITY_RESOLUTION_CANONICAL_PARTY_MISMATCH`, `IDENTITY_RESOLUTION_TOPOLOGY_GENERATION_STALE`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment`, `crm_customer_enrichment_application_adapter`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_provider_process_composition`, `crm_customer_enrichment_review_adapter`, `crm_customer_privacy_owner_scope_support`, `crm_identity_resolution`, `crm_identity_resolution_topology_composition`, `crm_module_sdk`, `crm_query_runtime`, `crm_proto_contracts`

### [`src/request.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-privacy-scope-adapter/src/request.rs) · 253 lines

- **Public declarations (4):** `token@L33`, `validate_request_contract@L75`, `validate_wire_request@L80`, `encode_cursor@L138`
- **Internal/trait/helpers (4):** `parse@L45`, `decode_cursor@L166`, `cursor_digest@L215`, `cursor_invalid@L247`
- **Public types:** `ResourceFamily@L22`, `CursorState@L60`, `ValidatedRequest@L66`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_PRIVACY_SCOPE_REQUEST_INVALID`, `CUSTOMER_ENRICHMENT_PRIVACY_SCOPE_LINEAGE_INVALID`, `CUSTOMER_ENRICHMENT_PRIVACY_SCOPE_CURSOR_INVALID`
- **Rust crate references:** `crm_customer_privacy_owner_scope_support`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`

### [`src/response.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-privacy-scope-adapter/src/response.rs) · 172 lines

- **Public declarations (2):** `build_response@L54`, `typed_output@L127`
- **Internal/trait/helpers (3):** `resource_type@L31`, `retention_policy_id@L43`, `page_digest@L143`
- **Public types:** `VerifiedCustomerEnrichmentResource@L24`
- **Rust crate references:** `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_customer_privacy_owner_scope_support`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/tests.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-privacy-scope-adapter/src/tests.rs) · 77 lines

- **Internal/trait/helpers (3):** `definition_is_exact_contract_only_query_coordinate@L17`, `frozen_bounds_match_the_entry_packet@L29`, `seven_family_order_and_tokens_are_stable@L53`
- **Detected tests (3):** `definition_is_exact_contract_only_query_coordinate@L17`, `frozen_bounds_match_the_entry_packet@L29`, `seven_family_order_and_tokens_are_stable@L53`
- **Rust crate references:** `crm_customer_enrichment_capability_adapter`

### [`tests/postgres_scope.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-privacy-scope-adapter/tests/postgres_scope.rs) · 1219 lines

- **Internal/trait/helpers (23):** `enrichment_scope_is_relationship_rooted_strict_minimized_and_side_effect_free@L95`, `definitions@L408`, `insert_definitions@L438`, `insert_complete_graph@L468`, `provider_response_receipt_contract@L676`, `provider_usage_contract@L687`, `insert_record@L698`, `insert_request_relationship@L743`, `data_class_name@L787`, `create_party@L795`, `merge_party@L825`, `capability_request@L874`, `scope_request@L920`, `current_generation@L976`, `write_surface_counts@L1005`, `prove_relationship_and_record_primary_key_scans@L1046`, `set_record_data_class@L1101`, `delete_record@L1117`, `decode@L1135`, `expected_two_graphs@L1140`, `graph_values@L1174`, `assert_response_omits_private_enrichment_values@L1186`, `contains@L1214`
- **Detected tests (1):** `enrichment_scope_is_relationship_rooted_strict_minimized_and_side_effect_free@L95`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`, `CUSTOMER_ENRICHMENT_PRIVACY_SCOPE_LINEAGE_INVALID`, `CUSTOMER_ENRICHMENT_PRIVACY_SCOPE_CURSOR_INVALID`, `CUSTOMER_ENRICHMENT_PRIVACY_SCOPE_STORED_STATE_INVALID`, `CUSTOMER_ENRICHMENT_PRIVACY_SCOPE_ASSOCIATION_STATE_INVALID`, `PRIVACY_ERASURE_SCOPE`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment`, `crm_customer_enrichment_application_adapter`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_privacy_scope_adapter`, `crm_customer_enrichment_provider_process_composition`, `crm_customer_enrichment_review_adapter`, `crm_customer_privacy`, `crm_identity_resolution_capability_adapter`, `crm_module_sdk`, `crm_parties_capability_adapter`, `crm_proto_contracts`, `crm_query_runtime`, `crm_parties`, `crm_identity_resolution`

## 38. `crm-customer-enrichment-provider-process-composition` — LATER / B-LATER + D? adapter

### [`src/conflict_persistence.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-provider-process-composition/src/conflict_persistence.rs) · 574 lines

- **Public declarations (10):** `new@L73`, `record@L77`, `recovery_conflict_for_request@L95`, `unresolved_for_request@L150`, `provider_response_conflict_persistence_plan@L162`, `provider_response_conflict_persisted_contract@L251`, `provider_response_conflict_persisted_payload@L262`, `provider_response_conflict_record_ref@L272`, `provider_response_conflict_relationship@L282`, `provider_response_conflict_to_wire@L313`
- **Internal/trait/helpers (14):** `provider_response_conflict_relationship_payload@L298`, `validate_batch_result@L333`, `conflict_suffix@L365`, `configured@L374`, `conflict_batch_error@L378`, `conflict_state_invalid@L413`, `conflict_plan_invalid@L423`, `request_id@L440`, `receipt_id@L448`, `draft@L456`, `lineage@L467`, `plan_contains_one_exact_record_event_audit_and_idempotency_transaction@L477`, `exact_replay_has_identical_persistence_identity_and_changed_fingerprint_does_not@L535`, `persisted_payload_is_strict_confidential_conflict_state@L557`
- **Public types:** `ProviderResponseConflictPersistenceLineage@L47`, `ProviderResponseConflictPersistencePlan@L54`, `ProviderResponseConflictPersistenceResult@L62`, `PostgresProviderResponseConflictStore@L68`
- **Detected tests (3):** `plan_contains_one_exact_record_event_audit_and_idempotency_transaction@L477`, `exact_replay_has_identical_persistence_identity_and_changed_fingerprint_does_not@L535`, `persisted_payload_is_strict_confidential_conflict_state@L557`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_IDEMPOTENCY_REUSED`, `CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_IN_PROGRESS`, `CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_PERSISTENCE_CONFLICT`, `CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_STORAGE_UNAVAILABLE`, `CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_STATE_INVALID`, `CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_PLAN_INVALID`
- **Rust crate references:** `crm_capability_ingress`, `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/conflict_rejection.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-provider-process-composition/src/conflict_rejection.rs) · 368 lines

- **Public declarations (2):** `new@L48`, `execute@L52`
- **Internal/trait/helpers (10):** `rejection_context@L181`, `validate_rejection_result@L220`, `conflict_suffix@L249`, `rejection_resolution_missing@L265`, `rejection_resolution_invalid@L274`, `rejection_request_not_found@L284`, `rejection_state_invalid@L293`, `rejection_read_error@L303`, `rejection_batch_error@L325`, `configured@L357`
- **Public types:** `ProviderResponseConflictRejectionLineage@L30`, `ProviderResponseConflictRejectionResult@L37`, `PostgresProviderResponseConflictRejectExecutor@L43`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_UNRESOLVED`, `CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_REJECTION_DECISION_INVALID`, `CUSTOMER_ENRICHMENT_REQUEST_NOT_FOUND`, `CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_REJECTION_STATE_INVALID`, `CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_REJECTION_STORE_UNAVAILABLE`, `CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_REJECTION_CONFLICT`, `CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_REJECTION_IN_PROGRESS`, `CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_REJECTION_PLAN_INVALID`, `CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_REJECTION_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_capability_ingress`, `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/conflict_resolution.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-provider-process-composition/src/conflict_resolution.rs) · 447 lines

- **Public declarations (2):** `new@L76`, `execute@L83`
- **Internal/trait/helpers (13):** `fmt@L63`, `resolution_context@L218`, `allowed_policy_version@L253`, `validate_policy_evidence@L280`, `validate_resolution_result@L293`, `conflict_suffix@L326`, `conflict_not_found@L343`, `resolution_input_invalid@L352`, `resolution_policy_invalid@L362`, `resolution_state_invalid@L372`, `resolution_read_error@L382`, `resolution_batch_error@L404`, `configured@L436`
- **Public types:** `ProviderResponseConflictResolutionCommand@L37`, `ProviderResponseConflictResolutionResult@L51`, `PostgresProviderResponseConflictResolutionExecutor@L57`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_RESOLUTION_DENIED`, `CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_NOT_FOUND`, `CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_RESOLUTION_INPUT_INVALID`, `CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_RESOLUTION_POLICY_INVALID`, `CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_RESOLUTION_STATE_INVALID`, `CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_RESOLUTION_STORE_UNAVAILABLE`, `CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_RESOLUTION_CONFLICT`, `CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_RESOLUTION_IN_PROGRESS`, `CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_RESOLUTION_PLAN_INVALID`, `CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_RESOLUTION_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_capability_ingress`, `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-provider-process-composition/src/lib.rs) · 520 lines

- **Public declarations (1):** `build_provider_dispatch_work_item@L62`
- **Internal/trait/helpers (15):** `build_dispatch_capability_request@L160`, `dispatch_work_item_identity@L222`, `hash_frame@L234`, `hex@L239`, `nonnegative_u64@L249`, `configured@L253`, `dispatch_timestamp_invalid@L257`, `queue_state_unrecoverable@L261`, `request_not_actionable@L270`, `work_item_invalid@L280`, `dispatched_recovery_rebuilds_the_exact_created_work_item@L300`, `dispatched_recovery_rebuilds_the_exact_retry_work_item@L337`, `exact_profile_party_and_deadline_are_revalidated_before_recovery@L378`, `queued_and_terminal_states_are_not_recoverable@L418`, `fixture@L458`
- **Public types:** `ProviderDispatchWorkItemInput@L48`
- **Re-exports:** `pub use conflict_persistence::*;`, `pub use conflict_rejection::*;`, `pub use conflict_resolution::*;`, `pub use worker::*;`
- **Detected tests (4):** `dispatched_recovery_rebuilds_the_exact_created_work_item@L300`, `dispatched_recovery_rebuilds_the_exact_retry_work_item@L337`, `exact_profile_party_and_deadline_are_revalidated_before_recovery@L378`, `queued_and_terminal_states_are_not_recoverable@L418`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_PROVIDER_QUEUE_STATE_UNRECOVERABLE`, `CUSTOMER_ENRICHMENT_PROVIDER_REQUEST_NOT_ACTIONABLE`, `CUSTOMER_ENRICHMENT_PROVIDER_WORK_ITEM_PREPARATION_INVALID`, `CUSTOMER_ENRICHMENT_DISPATCH_TARGET_CONFLICT`, `CUSTOMER_ENRICHMENT_DISPATCH_WINDOW_CLOSED`
- **Rust crate references:** `crm_capability_ingress`, `crm_capability_runtime`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_worker_composition`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/worker.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-provider-process-composition/src/worker.rs) · 647 lines

- **Public declarations (3):** `new@L119`, `with_page_size@L128`, `run_cycle@L151`
- **Internal/trait/helpers (22):** `load@L53`, `execute@L63`, `execute@L70`, `fmt@L104`, `process_delivery@L253`, `run_tenant_cycle@L395`, `processed_delivery@L421`, `recorded_outcome@L437`, `provider_outcome_invalid@L466`, `decode_created_event@L475`, `current_time_ms@L502`, `module_id@L519`, `event_type@L523`, `worker_identifier_invalid@L527`, `worker_configuration_invalid@L531`, `created_event_invalid@L541`, `unresolved_provider_conflict@L550`, `source_snapshot_invalid@L560`, `worker_identity_and_event_coordinate_are_canonical@L578`, `unresolved_provider_conflict_is_a_retryable_checkpoint_hold@L588`, `worker_clock_requires_a_positive_representable_millisecond@L598`, `decodes_only_self_consumed_request_created_evidence@L608`
- **Public types:** `ProviderDispatchSourceSnapshot@L40`, `ProviderDispatchSourceDisposition@L47`, `ProviderDispatchSourcePort@L52`, `ProviderDispatchExecutorPort@L62`, `ProviderProcessCycle@L81`, `CustomerEnrichmentProviderProcessWorker@L93`
- **Re-exports:** `pub use crm_customer_enrichment::PROVIDER_PROCESS_PROJECTION_ID;`
- **Detected tests (4):** `worker_identity_and_event_coordinate_are_canonical@L578`, `unresolved_provider_conflict_is_a_retryable_checkpoint_hold@L588`, `worker_clock_requires_a_positive_representable_millisecond@L598`, `decodes_only_self_consumed_request_created_evidence@L608`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_PROVIDER_PROCESS_OUTCOME_INVALID`, `CUSTOMER_ENRICHMENT_PROVIDER_PROCESS_CONFIGURATION_INVALID`, `CUSTOMER_ENRICHMENT_REQUEST_CREATED_EVENT_INVALID`, `CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_UNRESOLVED`, `CUSTOMER_ENRICHMENT_PROVIDER_SOURCE_SNAPSHOT_INVALID`
- **Rust crate references:** `crm_application_composition`, `crm_core_data`, `crm_core_events`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_worker_composition`, `crm_module_sdk`, `crm_proto_contracts`

### [`tests/postgres_conflict_persistence.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-provider-process-composition/tests/postgres_conflict_persistence.rs) · 298 lines

- **Internal/trait/helpers (8):** `postgres_conflict_persistence_is_atomic_and_exactly_replayable@L33`, `draft@L139`, `lineage@L150`, `canonical_request@L159`, `seed_request@L213`, `seed_context@L257`, `receipt_id@L284`, `scalar@L292`
- **Detected tests (1):** `postgres_conflict_persistence_is_atomic_and_exactly_replayable@L33`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`
- **Rust crate references:** `crm_core_data`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_provider_process_composition`, `crm_module_sdk`, `crm_proto_contracts`

### [`tests/postgres_conflict_process_hold.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-provider-process-composition/tests/postgres_conflict_process_hold.rs) · 551 lines

- **Internal/trait/helpers (13):** `unresolved_conflict_is_persisted_once_and_holds_checkpoint_across_restart@L45`, `provider_outcome@L247`, `evaluate@L270`, `load@L293`, `execute@L316`, `fixture@L333`, `receipt_id@L410`, `seed_request@L418`, `execution_context@L461`, `evidence_counts@L494`, `conflict_count@L529`, `conflict_relationship_count@L537`, `scalar@L545`
- **Detected tests (1):** `unresolved_conflict_is_persisted_once_and_holds_checkpoint_across_restart@L45`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`, `CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_UNRESOLVED`
- **Rust crate references:** `crm_core_data`, `crm_core_events`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_provider_process_composition`, `crm_customer_enrichment_worker_composition`, `crm_module_sdk`, `crm_proto_contracts`

### [`tests/postgres_conflict_reject_process.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-provider-process-composition/tests/postgres_conflict_reject_process.rs) · 513 lines

- **Internal/trait/helpers (14):** `approved_reject_terminalizes_once_then_resumes_checkpoint_without_provider_io@L49`, `provider_outcome@L222`, `evaluate@L245`, `load@L268`, `execute@L293`, `canonical_request@L309`, `seed_request@L364`, `execution_context@L407`, `receipt_id@L430`, `evidence_counts@L448`, `request_record_version@L483`, `status_changed_events@L491`, `suggestion_records@L499`, `scalar@L507`
- **Detected tests (1):** `approved_reject_terminalizes_once_then_resumes_checkpoint_without_provider_io@L49`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`, `TEST_PROVIDER_SOURCE_MUST_NOT_RUN`, `TEST_PROVIDER_EXECUTOR_MUST_NOT_RUN`
- **Rust crate references:** `crm_core_data`, `crm_core_events`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_provider_process_composition`, `crm_customer_enrichment_worker_composition`, `crm_module_sdk`, `crm_proto_contracts`

### [`tests/postgres_conflict_resolution.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-provider-process-composition/tests/postgres_conflict_resolution.rs) · 396 lines

- **Internal/trait/helpers (13):** `postgres_conflict_resolution_is_live_authorized_immutable_and_replay_safe@L35`, `new@L190`, `calls@L197`, `evaluate@L203`, `command@L213`, `draft@L231`, `lineage@L242`, `canonical_request@L251`, `seed_request@L305`, `seed_context@L349`, `receipt_id@L374`, `record_version@L382`, `scalar@L390`
- **Detected tests (1):** `postgres_conflict_resolution_is_live_authorized_immutable_and_replay_safe@L35`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`, `CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_RESOLUTION_DENIED`, `CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_CONFLICT_ALREADY_RESOLVED`
- **Rust crate references:** `crm_core_data`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_provider_process_composition`, `crm_module_sdk`, `crm_proto_contracts`

## 39. `crm-customer-enrichment-provider-registry` — LATER / B-LATER + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-provider-registry/src/lib.rs) · 1379 lines

- **Public declarations (21):** `enabled@L41`, `enabled_arc@L51`, `disabled@L61`, `coordinate@L68`, `try_new@L96`, `len@L112`, `is_empty@L116`, `resolve_exact@L121`, `try_new@L164`, `expose_to_transport@L177`, `try_new@L226`, `try_new@L310`, `try_new@L394`, `provider_request@L480`, `credential@L484`, `credential_aliases@L488`, `try_new@L531`, `class@L544`, `safe_provider_code@L548`, `new@L571`, `new@L657`
- **Internal/trait/helpers (65):** `fmt@L74`, `fmt@L134`, `dispatch_exact@L143`, `fmt@L183`, `fmt@L200`, `resolve@L212`, `fmt@L247`, `resolve@L261`, `acquire@L277`, `from_request@L287`, `fmt@L330`, `acquire@L340`, `before_attempt@L374`, `record_success@L375`, `record_failure@L376`, `fmt@L414`, `before_attempt@L424`, `record_success@L445`, `record_failure@L454`, `fmt@L494`, `dispatch@L555`, `fmt@L587`, `dispatch@L599`, `fmt@L671`, `dispatch@L681`, `deterministic_response_digest@L741`, `validate_provider_request@L766`, `validate_transport_response@L786`, `validate_handle_alias@L808`, `validate_safe_provider_code@L821`, `valid_now@L834`, `duplicate_registration@L842`, `adapter_unavailable@L852`, `adapter_disabled@L862`, `provider_configuration_invalid@L872`, `provider_secret_unavailable@L882`, `provider_quota_exceeded@L891`, `provider_circuit_open@L900`, `provider_state_unavailable@L909`, `provider_response_invalid@L918`, `transport_failure_error@L927`, `coordinate_reference@L961`, `block_on@L985`, `dispatch@L1003`, `new@L1025`, `calls@L1036`, `dispatch@L1042`, `coordinate@L1057`, `provider_request@L1061`, `resolver@L1128`, `adapter_with_transport@L1138`, `exact_coordinate_resolves_enabled_adapter@L1163`, `another_contract_version_does_not_fallback@L1176`, `disabled_exact_coordinate_fails_closed@L1191`, `duplicate_exact_coordinate_is_rejected@L1203`, `secret_debug_surfaces_are_redacted@L1214`, `governed_adapter_resolves_secret_and_returns_only_sanitized_evidence@L1231`, `missing_secret_fails_closed_without_secret_or_raw_error_leakage@L1255`, `quota_is_tenant_and_exact_coordinate_scoped@L1282`, `retryable_transport_failure_opens_circuit_without_second_provider_call@L1297`, `mapping_conflict_is_typed_bounded_and_does_not_open_circuit@L1318`, `transport_failure_rejects_unbounded_or_unsafe_provider_codes@L1353`, `assert_registry_port@L1370`, `assert_provider_port@L1371`, `provider_boundaries_are_thread_safe@L1374`
- **Public types:** `ProviderAdapterRegistration@L35`, `ExactProviderAdapterRegistry@L91`, `ProviderSecretMaterial@L159`, `ProviderSecretRegistration@L193`, `ProviderSecretHandleResolverPort@L211`, `StaticProviderSecretHandleResolver@L221`, `ProviderQuotaPort@L276`, `FixedWindowProviderQuota@L302`, `ProviderCircuitBreakerPort@L373`, `ConsecutiveFailureProviderCircuitBreaker@L386`, `ProviderTransportRequest@L474`, `ProviderTransportFailureClass@L516`, `ProviderTransportFailure@L525`, `ProviderTransportPort@L554`, `GovernedProviderAdapter@L563`, `DeterministicSanitizedProviderTransport@L650`
- **Detected tests (12):** `exact_coordinate_resolves_enabled_adapter@L1163`, `another_contract_version_does_not_fallback@L1176`, `disabled_exact_coordinate_fails_closed@L1191`, `duplicate_exact_coordinate_is_rejected@L1203`, `secret_debug_surfaces_are_redacted@L1214`, `governed_adapter_resolves_secret_and_returns_only_sanitized_evidence@L1231`, `missing_secret_fails_closed_without_secret_or_raw_error_leakage@L1255`, `quota_is_tenant_and_exact_coordinate_scoped@L1282`, `retryable_transport_failure_opens_circuit_without_second_provider_call@L1297`, `mapping_conflict_is_typed_bounded_and_does_not_open_circuit@L1318`, `transport_failure_rejects_unbounded_or_unsafe_provider_codes@L1353`, `provider_boundaries_are_thread_safe@L1374`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_PROVIDER_ADAPTER_DUPLICATE`, `CUSTOMER_ENRICHMENT_PROVIDER_ADAPTER_UNAVAILABLE`, `CUSTOMER_ENRICHMENT_PROVIDER_ADAPTER_DISABLED`, `CUSTOMER_ENRICHMENT_PROVIDER_CONFIGURATION_INVALID`, `CUSTOMER_ENRICHMENT_PROVIDER_SECRET_UNAVAILABLE`, `CUSTOMER_ENRICHMENT_PROVIDER_QUOTA_EXCEEDED`, `CUSTOMER_ENRICHMENT_PROVIDER_CIRCUIT_OPEN`, `CUSTOMER_ENRICHMENT_PROVIDER_STATE_UNAVAILABLE`, `CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_INVALID`, `CUSTOMER_ENRICHMENT_PROVIDER_RETRYABLE_FAILURE`, `CUSTOMER_ENRICHMENT_PROVIDER_TERMINAL_FAILURE`, `CUSTOMER_ENRICHMENT_PROVIDER_MAPPING_CONFLICT`, `TEST_NOOP_ADAPTER`
- **Rust crate references:** `crm_customer_enrichment`, `crm_module_sdk`

## 40. `crm-customer-enrichment-query-adapter` — LATER / B-LATER + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-query-adapter/src/lib.rs) · 601 lines

- **Public declarations (6):** `new@L57`, `query_capability_definitions@L296`, `query_capability_definition@L304`, `provider_profile_query_capability_definition@L308`, `mapping_query_capability_definition@L317`, `enrichment_request_query_capability_definition@L326`
- **Internal/trait/helpers (27):** `execute_get_provider_profile@L61`, `execute_get_mapping@L98`, `execute_get_enrichment_request@L137`, `get_visible_snapshot@L187`, `get_snapshot@L208`, `fmt@L228`, `validate@L238`, `execute@L274`, `query_definition@L335`, `decode_input@L360`, `provider_profile_record_id@L392`, `mapping_record_id@L409`, `enrichment_request_record_id@L424`, `redact_enrichment_request@L441`, `ensure_definition@L489`, `module_id@L500`, `provider_profile_record_type@L504`, `mapping_record_type@L508`, `enrichment_request_record_type@L512`, `configured@L516`, `unsupported_query@L520`, `configuration_error@L529`, `provider_profile_not_found@L538`, `mapping_not_found@L547`, `enrichment_request_not_found@L556`, `query_catalog_is_exact@L570`, `request_get_contract_is_personal@L589`
- **Public types:** `CustomerEnrichmentQueryAdapter@L51`
- **Detected tests (2):** `query_catalog_is_exact@L570`, `request_get_contract_is_personal@L589`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_QUERY_CONTRACT_MISMATCH`, `CUSTOMER_ENRICHMENT_QUERY_PROTOBUF_INVALID`, `CUSTOMER_ENRICHMENT_QUERY_UNSUPPORTED`, `CUSTOMER_ENRICHMENT_QUERY_CONFIGURATION_INVALID`, `CUSTOMER_ENRICHMENT_PROVIDER_PROFILE_NOT_FOUND`, `CUSTOMER_ENRICHMENT_MAPPING_NOT_FOUND`, `CUSTOMER_ENRICHMENT_REQUEST_NOT_FOUND`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`

## 41. `crm-customer-enrichment-registry-http-transport` — LATER / B-LATER + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-registry-http-transport/src/lib.rs) · 845 lines

- **Public declarations (3):** `try_new@L44`, `endpoint@L80`, `try_new@L105`
- **Internal/trait/helpers (26):** `fmt@L86`, `fmt@L125`, `dispatch@L136`, `serialize_request@L208`, `exactly_one_secret@L240`, `bearer_header@L261`, `read_bounded_body@L273`, `parse_response@L290`, `effective_timeout@L361`, `map_reqwest_failure@L386`, `map_http_status@L397`, `parse_fixed_endpoint@L431`, `valid_optional_text@L451`, `valid_optional_safe_code@L460`, `failure@L471`, `configuration_invalid@L479`, `mock_provider@L524`, `spawn_provider@L569`, `provider_request@L581`, `adapter@L648`, `configuration_requires_an_exact_allowlisted_endpoint@L690`, `successful_http_dispatch_returns_only_sanitized_evidence@L714`, `malformed_response_is_a_mapping_conflict@L749`, `timeout_is_retryable_and_opens_the_exact_coordinate_circuit@L772`, `quota_is_enforced_before_a_second_http_attempt@L797`, `redirects_are_not_followed@L820`
- **Public types:** `RegistryHttpTransportConfig@L36`, `RegistryHttpTransport@L98`
- **Detected tests (6):** `configuration_requires_an_exact_allowlisted_endpoint@L690`, `successful_http_dispatch_returns_only_sanitized_evidence@L714`, `malformed_response_is_a_mapping_conflict@L749`, `timeout_is_retryable_and_opens_the_exact_coordinate_circuit@L772`, `quota_is_enforced_before_a_second_http_attempt@L797`, `redirects_are_not_followed@L820`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_REGISTRY_HTTP_CONFIGURATION_INVALID`, `CUSTOMER_ENRICHMENT_PROVIDER_MAPPING_CONFLICT`, `CUSTOMER_ENRICHMENT_PROVIDER_RETRYABLE_FAILURE`, `CUSTOMER_ENRICHMENT_PROVIDER_CIRCUIT_OPEN`, `CUSTOMER_ENRICHMENT_PROVIDER_QUOTA_EXCEEDED`, `CUSTOMER_ENRICHMENT_PROVIDER_TERMINAL_FAILURE`
- **Rust crate references:** `crm_customer_enrichment`, `crm_customer_enrichment_provider_registry`, `crm_module_sdk`

### [`tests/http_response_contract.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-registry-http-transport/tests/http_response_contract.rs) · 287 lines

- **Internal/trait/helpers (8):** `provider@L45`, `spawn_provider@L101`, `oversized_response_is_bounded_sanitized_and_deterministic@L118`, `wrong_schema_is_sanitized_and_deterministic@L128`, `verify_rejected_response@L137`, `assert_mapping_conflict@L174`, `adapter@L184`, `provider_request@L219`
- **Detected tests (2):** `oversized_response_is_bounded_sanitized_and_deterministic@L118`, `wrong_schema_is_sanitized_and_deterministic@L128`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_PROVIDER_MAPPING_CONFLICT`
- **Rust crate references:** `crm_customer_enrichment`, `crm_customer_enrichment_provider_registry`, `crm_customer_enrichment_registry_http_transport`, `crm_module_sdk`

## 42. `crm-customer-enrichment-request-list-query-adapter` — LATER / B-LATER + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-request-list-query-adapter/src/lib.rs) · 690 lines

- **Public declarations (2):** `new@L47`, `query_capability_definition@L281`
- **Internal/trait/helpers (30):** `execute_list@L59`, `collect_visible_requests@L102`, `has_more_visible@L182`, `fmt@L229`, `validate@L240`, `execute@L267`, `decode_input@L305`, `party_record_id@L334`, `provider_profile_record_id@L351`, `validate_status@L368`, `status_to_wire@L395`, `matches_filters@L418`, `resolve_page_size@L432`, `cursor_binding@L450`, `decode_after@L480`, `encode_next@L501`, `list_response@L522`, `redact_enrichment_request@L537`, `ensure_definition@L585`, `module_id@L596`, `request_record_type@L600`, `party_record_type@L604`, `configured@L608`, `enforce_scan_limit@L612`, `cursor_invalid@L625`, `cursor_error@L634`, `unsupported_query@L638`, `configuration_error@L647`, `definition_is_one_personal_low_risk_query@L662`, `page_size_and_status_are_bounded@L681`
- **Public types:** `CustomerEnrichmentRequestListQueryAdapter@L40`
- **Detected tests (2):** `definition_is_one_personal_low_risk_query@L662`, `page_size_and_status_are_bounded@L681`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_REQUEST_LIST_CONTRACT_MISMATCH`, `CUSTOMER_ENRICHMENT_REQUEST_LIST_PROTOBUF_INVALID`, `CUSTOMER_ENRICHMENT_REQUEST_LIST_SCAN_LIMIT_EXCEEDED`, `CUSTOMER_ENRICHMENT_REQUEST_LIST_CURSOR_INVALID`, `CUSTOMER_ENRICHMENT_REQUEST_LIST_UNSUPPORTED`, `CUSTOMER_ENRICHMENT_REQUEST_LIST_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`

## 43. `crm-customer-enrichment-review-adapter` — LATER / B-LATER + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-review-adapter/src/lib.rs) · 708 lines

- **Public declarations (15):** `review_capability_definitions@L51`, `accept_suggestion_capability_definition@L58`, `reject_suggestion_capability_definition@L66`, `new@L106`, `suggestion_persisted_contract@L273`, `review_decision_persisted_contract@L284`, `suggestion_from_snapshot@L295`, `review_decision_from_snapshot@L315`, `suggestion_persisted_payload@L339`, `review_decision_persisted_payload@L347`, `suggestion_record_ref@L357`, `review_decision_record_ref@L365`, `suggestion_to_wire@L373`, `suggestion_to_wire_with_supersession@L381`, `review_decision_to_wire@L456`
- **Internal/trait/helpers (19):** `review_definition@L74`, `review_contract@L94`, `decision@L116`, `target@L185`, `plan@L197`, `ensure_definition@L490`, `ensure_suggestion_ref@L506`, `ensure_expected_binding@L530`, `request_started_at_unix_ms@L558`, `positive_u64@L570`, `non_negative_u64@L581`, `target_field_to_wire@L586`, `suggestion_status_to_wire@L592`, `checked_i64@L609`, `configured@L666`, `unsupported_capability@L670`, `review_conflict@L679`, `review_state_invalid@L689`, `review_plan_invalid@L699`
- **Public types:** `CustomerEnrichmentSuggestionReviewPlanner@L100`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_REVIEW_CAPABILITY_UNSUPPORTED`, `CUSTOMER_ENRICHMENT_REVIEW_CONFLICT`, `CUSTOMER_ENRICHMENT_REVIEW_PERSISTED_STATE_INVALID`, `CUSTOMER_ENRICHMENT_REVIEW_PLAN_INVALID`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_module_sdk`, `crm_proto_contracts`

### [`tests/suggestion_review_atomic_plan.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-review-adapter/tests/suggestion_review_atomic_plan.rs) · 371 lines

- **Internal/trait/helpers (14):** `accepted_suggestion_is_one_immutable_atomic_batch@L26`, `rejected_suggestion_is_one_immutable_atomic_batch_without_approval@L78`, `acceptance_requires_policy_approval_evidence@L100`, `stale_party_version_is_rejected_before_locking@L120`, `changed_value_digest_is_rejected_before_locking@L140`, `expired_suggestion_is_rejected_before_locking@L154`, `strict_snapshot_round_trip_preserves_deterministic_suggestion_identity@L168`, `suggestion@L182`, `profile@L246`, `mapping@L265`, `digest@L278`, `accept_request@L282`, `reject_request@L309`, `request@L332`
- **Detected tests (7):** `accepted_suggestion_is_one_immutable_atomic_batch@L26`, `rejected_suggestion_is_one_immutable_atomic_batch_without_approval@L78`, `acceptance_requires_policy_approval_evidence@L100`, `stale_party_version_is_rejected_before_locking@L120`, `changed_value_digest_is_rejected_before_locking@L140`, `expired_suggestion_is_rejected_before_locking@L154`, `strict_snapshot_round_trip_preserves_deterministic_suggestion_identity@L168`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_APPROVAL_REQUIRED`, `CUSTOMER_ENRICHMENT_REVIEW_CONFLICT`, `CUSTOMER_ENRICHMENT_SUGGESTION_EXPIRED`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_review_adapter`, `crm_module_sdk`, `crm_proto_contracts`

## 44. `crm-customer-enrichment-review-composition` — LATER / B-LATER + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-review-composition/src/lib.rs) · 413 lines

- **Public declarations (3):** `new@L51`, `execute@L55`, `suggestion_review_policy_request@L105`
- **Internal/trait/helpers (15):** `fmt@L41`, `review_command@L157`, `required_suggestion_id@L191`, `resolve_policy@L208`, `validate_policy_evidence@L250`, `validate_safe_reason@L267`, `suggestion_not_found@L293`, `review_input_invalid@L302`, `review_input_conflict@L312`, `review_store_unavailable@L322`, `review_state_invalid@L332`, `review_policy_invalid@L342`, `allowed_policy_preserves_acceptance_approval_requirement@L357`, `rejection_never_requires_acceptance_approval@L372`, `denied_and_mismatched_policy_fail_closed@L387`
- **Public types:** `PostgresCustomerEnrichmentSuggestionReviewExecutor@L35`
- **Detected tests (3):** `allowed_policy_preserves_acceptance_approval_requirement@L357`, `rejection_never_requires_acceptance_approval@L372`, `denied_and_mismatched_policy_fail_closed@L387`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_SUGGESTION_REVIEW_POLICY_DENIED`, `CUSTOMER_ENRICHMENT_SUGGESTION_NOT_FOUND`, `CUSTOMER_ENRICHMENT_SUGGESTION_REVIEW_INPUT_INVALID`, `CUSTOMER_ENRICHMENT_SUGGESTION_REVIEW_CONFLICT`, `CUSTOMER_ENRICHMENT_SUGGESTION_REVIEW_STORE_UNAVAILABLE`, `CUSTOMER_ENRICHMENT_SUGGESTION_REVIEW_STATE_INVALID`, `CUSTOMER_ENRICHMENT_SUGGESTION_REVIEW_POLICY_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_review_adapter`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/stale_guard.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-review-composition/src/stale_guard.rs) · 122 lines

- **Public declarations (1):** `new@L16`
- **Internal/trait/helpers (3):** `check@L22`, `store_unavailable@L103`, `state_invalid@L113`
- **Public types:** `PostgresSuggestionMutationGuard@L11`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_SUGGESTION_SUPERSEDED`, `CUSTOMER_ENRICHMENT_STALE_GUARD_STORE_UNAVAILABLE`, `CUSTOMER_ENRICHMENT_STALE_GUARD_STATE_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_module_sdk`

### [`tests/postgres_review_process.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-review-composition/tests/postgres_review_process.rs) · 520 lines

- **Internal/trait/helpers (8):** `evaluate@L39`, `authorize_visibility@L72`, `postgres_review_and_permission_aware_queries_are_replay_safe@L108`, `query_request@L435`, `query_request_at@L444`, `suggestion_fields@L471`, `review_fields@L496`, `scalar@L514`
- **Detected tests (1):** `postgres_review_and_permission_aware_queries_are_replay_safe@L108`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`, `CUSTOMER_ENRICHMENT_SUGGESTION_SUPERSEDED`, `CUSTOMER_ENRICHMENT_SUGGESTION_NOT_FOUND`
- **Rust crate references:** `crm_capability_ingress`, `crm_core_data`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_review_composition`, `crm_customer_enrichment_suggestion_query_adapter`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`

### [`tests/support/mod.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-review-composition/tests/support/mod.rs) · 265 lines

- **Public declarations (5):** `suggestion@L30`, `refreshed_suggestion@L40`, `seed_suggestion@L149`, `seed_suggestion_with_suffix@L156`, `accept_request@L207`
- **Internal/trait/helpers (2):** `suggestion_at@L50`, `context@L240`
- **Rust crate references:** `crm_capability_ingress`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_review_adapter`, `crm_module_sdk`, `crm_proto_contracts`

## 45. `crm-customer-enrichment-suggestion-query-adapter` — LATER / B-LATER + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-suggestion-query-adapter/src/lib.rs) · 701 lines

- **Public declarations (20):** `new@L64`, `new_get_only@L76`, `cursor_codec@L87`, `ensure_party_visible@L153`, `load_visible_latest_reviews@L175`, `load_visible_suggestions_for_party@L240`, `query_capability_definitions@L370`, `get_suggestion_capability_definition@L377`, `list_suggestions_by_party_capability_definition@L385`, `decode_input@L421`, `request_started_at_unix_ms@L467`, `module_id@L478`, `suggestion_record_type@L482`, `party_record_type@L490`, `redact_suggestion@L494`, `enforce_scan_limit@L595`, `configured@L608`, `suggestion_not_found@L614`, `query_state_invalid@L632`, `query_configuration_invalid@L642`
- **Internal/trait/helpers (13):** `execute_get@L93`, `fmt@L300`, `validate@L311`, `execute@L332`, `ordering_key@L365`, `query_definition@L393`, `suggestion_record_id@L452`, `review_record_type@L486`, `redact_review_decision@L551`, `ensure_definition@L584`, `unsupported_query@L623`, `definitions_are_two_personal_low_risk_queries@L657`, `suggestion_redaction_is_field_specific@L675`
- **Public types:** `CustomerEnrichmentSuggestionQueryAdapter@L57`, `VisibleSuggestion@L350`, `VisibleReview@L357`
- **Detected tests (2):** `definitions_are_two_personal_low_risk_queries@L657`, `suggestion_redaction_is_field_specific@L675`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_SUGGESTION_QUERY_CONTRACT_MISMATCH`, `CUSTOMER_ENRICHMENT_SUGGESTION_QUERY_PROTOBUF_INVALID`, `CUSTOMER_ENRICHMENT_SUGGESTION_QUERY_SCAN_LIMIT_EXCEEDED`, `CUSTOMER_ENRICHMENT_SUGGESTION_NOT_FOUND`, `CUSTOMER_ENRICHMENT_SUGGESTION_QUERY_UNSUPPORTED`, `CUSTOMER_ENRICHMENT_SUGGESTION_QUERY_STATE_INVALID`, `CUSTOMER_ENRICHMENT_SUGGESTION_QUERY_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_review_adapter`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`

### [`src/list/cursor.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-suggestion-query-adapter/src/list/cursor.rs) · 171 lines

- **Public declarations (7):** `party_id@L12`, `profile_id@L27`, `status@L44`, `page_size@L56`, `binding@L74`, `decode_after@L103`, `encode_next@L123`
- **Internal/trait/helpers (3):** `cursor_invalid@L144`, `cursor_error@L153`, `page_and_status_bounds_are_strict@L162`
- **Detected tests (1):** `page_and_status_bounds_are_strict@L162`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_SUGGESTION_LIST_CURSOR_INVALID`
- **Rust crate references:** `crm_core_data`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`

### [`src/list/mod.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-suggestion-query-adapter/src/list/mod.rs) · 139 lines

- **Public declarations (3):** `validate@L13`, `execute@L27`, `matches@L75`
- **Internal/trait/helpers (2):** `response@L93`, `exact_filters_are_required@L111`
- **Detected tests (1):** `exact_filters_are_required@L111`
- **Rust crate references:** `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`

### [`src/list/scan.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-suggestion-query-adapter/src/list/scan.rs) · 135 lines

- **Public declarations (1):** `collect@L14`
- **Internal/trait/helpers (1):** `has_more@L88`
- **Rust crate references:** `crm_core_data`, `crm_customer_enrichment_review_adapter`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`

## 46. `crm-customer-enrichment-visibility` — LATER / B-LATER + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-visibility/src/lib.rs) · 261 lines

- **Public declarations (1):** `query_visibility_resources@L163`
- **Internal/trait/helpers (6):** `fields@L192`, `declaration_covers_exact_six_query_coordinates_once@L197`, `definition_queries_share_exact_provider_profile_visibility@L211`, `request_queries_share_party_gate_and_request_fields@L225`, `suggestion_get_and_list_share_exact_lifecycle_visibility@L240`, `unknown_query_contributes_no_visibility@L257`
- **Public types:** `CustomerEnrichmentVisibilityResource@L155`
- **Detected tests (5):** `declaration_covers_exact_six_query_coordinates_once@L197`, `definition_queries_share_exact_provider_profile_visibility@L211`, `request_queries_share_party_gate_and_request_fields@L225`, `suggestion_get_and_list_share_exact_lifecycle_visibility@L240`, `unknown_query_contributes_no_visibility@L257`
- **Rust crate references:** `crm_customer_enrichment`, `crm_customer_enrichment_query_adapter`, `crm_customer_enrichment_request_list_query_adapter`, `crm_customer_enrichment_suggestion_query_adapter`

## 47. `crm-customer-enrichment-worker-composition` — LATER / B-LATER + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-worker-composition/src/lib.rs) · 803 lines

- **Public declarations (4):** `try_new@L108`, `postgres@L124`, `execute@L148`, `execute_reconciled@L162`
- **Internal/trait/helpers (27):** `fmt@L89`, `validate_work_item@L234`, `validate_dispatch_output@L297`, `validate_sanitized_response@L350`, `build_response_request@L384`, `validate_response_output@L454`, `decode_execution_output@L498`, `validate_output_payload@L512`, `response_identity@L532`, `response_semantic_input_hash@L544`, `provider_response_class_tag@L591`, `hash_optional_i64@L600`, `hash_optional_text@L610`, `response_received_usage@L620`, `classify_response_reconciliation@L638`, `provider_response_class_to_wire@L664`, `validate_internal_key@L677`, `hash_frame@L690`, `hex@L695`, `configured@L705`, `worker_input_invalid@L709`, `dispatch_output_invalid@L719`, `provider_response_conflict_draft@L729`, `is_response_reconciliation_conflict@L764`, `conflicting_provider_replay@L771`, `response_output_invalid@L781`, `worker_output_invalid@L791`
- **Public types:** `ProviderDispatchWorkItem@L49`, `ProviderResponseReconciliation@L56`, `ProviderDispatchWorkerResult@L64`, `ProviderDispatchExecution@L73`, `CustomerEnrichmentProviderWorker@L80`
- **Error/guard literals:** `CUSTOMER_ENRICHMENT_PROVIDER_REPLAY_KEY_MISMATCH`, `CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_INVALID`, `CUSTOMER_ENRICHMENT_PROVIDER_WORK_ITEM_INVALID`, `CUSTOMER_ENRICHMENT_DISPATCH_OUTPUT_INVALID`, `DATA_CONFLICT`, `CAPABILITY_IDEMPOTENCY_KEY_REUSED`, `CUSTOMER_ENRICHMENT_CONFLICTING_PROVIDER_REPLAY`, `CUSTOMER_ENRICHMENT_RESPONSE_OUTPUT_INVALID`, `CUSTOMER_ENRICHMENT_WORKER_OUTPUT_INVALID`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/tests.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-worker-composition/src/tests.rs) · 505 lines

- **Internal/trait/helpers (14):** `execute@L33`, `dispatch_exact@L76`, `worker_orders_dispatch_commit_provider_and_response_commit@L87`, `provider_is_not_called_when_dispatch_commit_fails@L111`, `mismatched_provider_replay_key_stops_before_response_commit@L132`, `response_semantic_hash_ignores_only_volatile_transport_metadata@L152`, `repeated_work_item_builds_identical_response_commit_identity@L188`, `fixture@L216`, `provider_profile@L292`, `dispatch_capability_request@L311`, `worker@L354`, `dispatch_output@L381`, `response_output@L396`, `enrichment_request_wire@L463`
- **Detected tests (5):** `worker_orders_dispatch_commit_provider_and_response_commit@L87`, `provider_is_not_called_when_dispatch_commit_fails@L111`, `mismatched_provider_replay_key_stops_before_response_commit@L132`, `response_semantic_hash_ignores_only_volatile_transport_metadata@L152`, `repeated_work_item_builds_identical_response_commit_identity@L188`
- **Error/guard literals:** `TEST_DISPATCH_COMMIT_FAILED`, `CUSTOMER_ENRICHMENT_PROVIDER_REPLAY_KEY_MISMATCH`
- **Rust crate references:** `crm_capability_ingress`, `crm_customer_enrichment`, `crm_module_sdk`, `crm_proto_contracts`

### [`tests/postgres_worker_process.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-enrichment-worker-composition/tests/postgres_worker_process.rs) · 553 lines

- **Internal/trait/helpers (10):** `registry_provider@L61`, `spawn_registry_provider@L106`, `postgres_worker_commits_and_replays_without_duplicates@L126`, `fixture@L347`, `seed_request@L426`, `dispatch_capability_request@L471`, `seed_context@L501`, `read_context@L511`, `worker_context@L521`, `scalar@L547`
- **Detected tests (1):** `postgres_worker_commits_and_replays_without_duplicates@L126`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`, `CUSTOMER_ENRICHMENT_CONFLICTING_PROVIDER_REPLAY`
- **Rust crate references:** `crm_capability_ingress`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_enrichment`, `crm_customer_enrichment_capability_adapter`, `crm_customer_enrichment_provider_registry`, `crm_customer_enrichment_registry_http_transport`, `crm_customer_enrichment_worker_composition`, `crm_module_sdk`, `crm_proto_contracts`

## 48. `crm-customer-privacy-application` — END / B-END + D? adapter

### [`src/access_export.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-application/src/access_export.rs) · 245 lines

- **Public declarations (2):** `new@L122`, `request@L134`
- **Internal/trait/helpers (7):** `prepare@L74`, `complete@L79`, `request@L88`, `fmt@L111`, `validate_invocation@L197`, `access_export_disabled@L227`, `configuration_invalid@L236`
- **Public types:** `AccessExportInvocation@L16`, `AccessExportPreparation@L33`, `PrivacyExportTargetRequest@L44`, `PrivacyExportTargetResult@L62`, `AccessExportPersistencePort@L73`, `PrivacyExportTargetPort@L87`, `AccessExportResult@L95`, `PrivacyAccessExportService@L104`
- **Error/guard literals:** `CUSTOMER_PRIVACY_ACCESS_EXPORT_NOT_TRUSTED`, `CUSTOMER_PRIVACY_ACCESS_EXPORT_DISABLED`, `CUSTOMER_PRIVACY_ACCESS_EXPORT_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_application_composition`, `crm_customer_privacy`, `crm_module_sdk`

### [`src/approval.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-application/src/approval.rs) · 67 lines

- **Public declarations (1):** `approval_capability_definition@L12`
- **Internal/trait/helpers (2):** `configured@L36`, `approval_definition_is_exact_high_risk_idempotent_mutation@L53`
- **Detected tests (1):** `approval_definition_is_exact_high_risk_idempotent_mutation@L53`
- **Error/guard literals:** `CUSTOMER_PRIVACY_CASE_APPROVAL_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_customer_privacy`, `crm_module_sdk`

### [`src/discovery.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-application/src/discovery.rs) · 1397 lines

- **Public declarations (10):** `label@L79`, `exact_canonical@L176`, `len@L214`, `is_empty@L218`, `new@L242`, `discover@L254`, `new@L606`, `read@L616`, `discovery_attempt_digest@L1119`, `discovery_page_key_digest@L1141`
- **Internal/trait/helpers (41):** `begin_attempt@L107`, `load_owner_pages@L113`, `accept_owner_page@L120`, `advance_checkpoint@L127`, `finalize_snapshot@L136`, `load_snapshot@L142`, `record_audit@L148`, `fmt@L161`, `get@L208`, `fmt@L231`, `discover_all_owners@L368`, `discover_owner@L396`, `audit@L545`, `authorize@L591`, `validate_endpoint@L686`, `owner_query_request@L714`, `validate_response_page@L784`, `build_owner_contribution@L854`, `proto_lineage@L951`, `encode_owner_request@L966`, `decode_owner_response@L1026`, `scope_resource@L1081`, `append_frame@L1154`, `ensure_active@L1159`, `exact_digest@L1176`, `decode_error@L1189`, `map_owner_error@L1193`, `incompatible_owner@L1216`, `replay_conflict@L1225`, `corrupt_evidence@L1234`, `configuration_error@L1243`, `scope_error@L1252`, `discovery_error@L1256`, `module_id@L1271`, `record_id@L1275`, `retention@L1279`, `lineage@L1288`, `canonical_registry_and_frozen_bounds_are_exact@L1304`, `attempt_identity_is_deterministic_and_purpose_sensitive@L1322`, `page_key_binds_attempt_owner_page_and_request_cursor@L1334`, `terminal_page_requires_empty_cursor_and_safe_projection_only@L1357`
- **Public types:** `DiscoveryInvocation@L27`, `DiscoveryAttempt@L39`, `DiscoveryPageReceipt@L45`, `PersistedDiscoveryPage@L61`, `DiscoveryAuditEvent@L68`, `DiscoveryAuditRecord@L93`, `DiscoveryPersistencePort@L106`, `OwnerContributionEndpoint@L155`, `OwnerContributionEndpoints@L171`, `ScopeDiscoveryService@L224`, `SnapshotReadContext@L574`, `SnapshotVisibilityDecision@L584`, `DiscoverySnapshotVisibilityPort@L590`, `DiscoverySnapshotReader@L600`
- **Detected tests (4):** `canonical_registry_and_frozen_bounds_are_exact@L1304`, `attempt_identity_is_deterministic_and_purpose_sensitive@L1322`, `page_key_binds_attempt_owner_page_and_request_cursor@L1334`, `terminal_page_requires_empty_cursor_and_safe_projection_only@L1357`
- **Error/guard literals:** `CUSTOMER_PRIVACY_DISCOVERY_TRUST_REQUIRED`, `CUSTOMER_PRIVACY_DISCOVERY_TIME_INVALID`, `CUSTOMER_PRIVACY_DISCOVERY_REGISTRY_DRIFT`, `CUSTOMER_PRIVACY_DISCOVERY_SNAPSHOT_READ_DENIED`, `CUSTOMER_PRIVACY_DISCOVERY_SNAPSHOT_NOT_FOUND`, `CUSTOMER_PRIVACY_DISCOVERY_PAGE_SIZE_INVALID`, `CUSTOMER_PRIVACY_DISCOVERY_CURSOR_TOO_LARGE`, `CUSTOMER_PRIVACY_DISCOVERY_OWNER_INCOMPLETE`, `CUSTOMER_PRIVACY_DISCOVERY_RESOURCE_CONFLICT`, `CUSTOMER_PRIVACY_DISCOVERY_OWNER_DISABLED`, `CUSTOMER_PRIVACY_DISCOVERY_OWNER_UNAVAILABLE`, `CUSTOMER_PRIVACY_DISCOVERY_OWNER_FAILED`, `CUSTOMER_PRIVACY_DISCOVERY_OWNER_INCOMPATIBLE`, `CUSTOMER_PRIVACY_DISCOVERY_PAGE_REPLAY_CONFLICT`, `CUSTOMER_PRIVACY_DISCOVERY_EVIDENCE_CORRUPT`, `CUSTOMER_PRIVACY_DISCOVERY_CONFIGURATION_INVALID`, `ERASURE`, `ACCESS`
- **Rust crate references:** `crm_application_composition`, `crm_capability_runtime`, `crm_customer_privacy`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`

### [`src/execution.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-application/src/execution.rs) · 1010 lines

- **Public declarations (3):** `exact_canonical@L105`, `new@L220`, `execute_next@L232`
- **Internal/trait/helpers (41):** `apply@L77`, `fmt@L90`, `get@L126`, `load_ready@L162`, `prepare_next@L171`, `record_outcome@L176`, `advance_checkpoint@L183`, `fmt@L209`, `run_tenant_cycle@L312`, `validate_work_batch@L346`, `owner_request@L391`, `validate_invocation@L415`, `execution_disabled@L439`, `execution_not_trusted@L448`, `execution_invalid_argument@L457`, `worker_batch_invalid@L467`, `execution_configuration_invalid@L477`, `block_on@L502`, `is_active@L519`, `apply@L536`, `prepare_next@L558`, `record_outcome@L568`, `advance_checkpoint@L584`, `load_ready@L603`, `prepare_next@L615`, `record_outcome@L628`, `advance_checkpoint@L637`, `exact_endpoints@L645`, `attempt@L655`, `invocation@L731`, `harness@L760`, `worker_harness@L802`, `exact_owner_registry_rejects_missing_or_duplicate_owners@L831`, `execution_is_activation_and_trusted_internal_gated@L864`, `durable_prepare_precedes_owner_and_outcome_checkpoint_writes@L884`, `retry_generation_keeps_permanent_owner_idempotency_identity@L909`, `coordinator_only_retention_never_invokes_owner_capability@L920`, `completed_execution_is_terminal_and_has_no_owner_or_write_replay@L937`, `inactive_worker_cycle_does_not_discover_or_execute_work@L978`, `active_worker_cycle_loads_bounded_work_and_delegates_to_replay_safe_execution@L987`, `invalid_worker_batch_fails_before_any_execution@L996`
- **Public types:** `OwnerExecutionInvocation@L34`, `OwnerActionRequest@L51`, `OwnerActionResult@L71`, `OwnerPrivacyActionPort@L76`, `OwnerActionEndpoint@L84`, `OwnerActionEndpoints@L100`, `ExecutionPreparation@L143`, `CheckpointAdvance@L155`, `OwnerExecutionPersistencePort@L161`, `OwnerExecutionResult@L190`, `PrivacyOwnerExecutionService@L202`
- **Detected tests (9):** `exact_owner_registry_rejects_missing_or_duplicate_owners@L831`, `execution_is_activation_and_trusted_internal_gated@L864`, `durable_prepare_precedes_owner_and_outcome_checkpoint_writes@L884`, `retry_generation_keeps_permanent_owner_idempotency_identity@L909`, `coordinator_only_retention_never_invokes_owner_capability@L920`, `completed_execution_is_terminal_and_has_no_owner_or_write_replay@L937`, `inactive_worker_cycle_does_not_discover_or_execute_work@L978`, `active_worker_cycle_loads_bounded_work_and_delegates_to_replay_safe_execution@L987`, `invalid_worker_batch_fails_before_any_execution@L996`
- **Error/guard literals:** `CUSTOMER_PRIVACY_OWNER_EXECUTION_DISABLED`, `CUSTOMER_PRIVACY_OWNER_EXECUTION_NOT_TRUSTED`, `CUSTOMER_PRIVACY_OWNER_EXECUTION_INVALID_ARGUMENT`, `CUSTOMER_PRIVACY_OWNER_EXECUTION_WORK_BATCH_INVALID`, `CUSTOMER_PRIVACY_OWNER_EXECUTION_CONFIGURATION_INVALID`, `ERASURE_DISCOVERY`
- **Rust crate references:** `crm_application_composition`, `crm_customer_privacy`, `crm_module_sdk`

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-application/src/lib.rs) · 148 lines

- **Public declarations (6):** `mutation_capability_definitions@L51`, `mutation_capability_definitions_with_restrictions@L61`, `mutation_capability_definitions_with_restrictions_and_legal_holds@L69`, `mutation_capability_definitions_with_complete_control_lifecycle@L77`, `query_capability_definitions@L86`, `query_capability_definitions_with_complete_control_lifecycle@L93`
- **Internal/trait/helpers (1):** `inventories_preserve_legacy_and_complete_frozen_control_lifecycle@L106`
- **Re-exports:** `pub use access_export::*;`, `pub use approval::*;`, `pub use discovery::*;`, `pub use execution::*;`, `pub use planning::*;`, `pub use reads::*;`, `pub use retention::*;`, `pub use crm_customer_privacy_capability_adapter::{`
- **Detected tests (1):** `inventories_preserve_legacy_and_complete_frozen_control_lifecycle@L106`
- **Rust crate references:** `crm_customer_privacy`, `crm_capability_runtime`, `crm_customer_privacy_cancel_capability_adapter`, `crm_customer_privacy_capability_adapter`, `crm_customer_privacy_query_adapter`, `crm_customer_privacy_subject_capability_adapter`, `crm_customer_privacy_submit_capability_adapter`, `crm_module_sdk`

### [`src/planning.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-application/src/planning.rs) · 341 lines

- **Public declarations (2):** `new@L72`, `build@L82`
- **Internal/trait/helpers (12):** `load_source@L43`, `finalize_plan@L48`, `fmt@L62`, `validate_invocation@L135`, `validate_source@L158`, `validate_existing_plan@L217`, `domain_error@L260`, `configuration_error@L269`, `planning_error@L278`, `invocation@L298`, `planning_phase_and_trust_boundary_are_exact@L320`, `planned_time_must_round_trip_through_postgresql_exactly@L331`
- **Public types:** `PlanningInvocation@L15`, `PlanningSource@L29`, `PlanningCommit@L36`, `PlanningPersistencePort@L42`, `PrivacyPlanningService@L56`
- **Detected tests (2):** `planning_phase_and_trust_boundary_are_exact@L320`, `planned_time_must_round_trip_through_postgresql_exactly@L331`
- **Error/guard literals:** `CUSTOMER_PRIVACY_PLANNING_DISABLED`, `CUSTOMER_PRIVACY_PLANNING_CASE_STATE_INVALID`, `CUSTOMER_PRIVACY_PLANNING_TRUST_REQUIRED`, `CUSTOMER_PRIVACY_PLANNING_TIME_INVALID`, `CUSTOMER_PRIVACY_PLANNING_SUBJECT_UNVERIFIED`, `CUSTOMER_PRIVACY_PLANNING_SOURCE_MISMATCH`, `CUSTOMER_PRIVACY_PLANNING_EVIDENCE_INVALID`, `CUSTOMER_PRIVACY_PLANNING_REPLAY_CONFLICT`, `CUSTOMER_PRIVACY_PLANNING_DOMAIN_INVALID`, `CUSTOMER_PRIVACY_PLANNING_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_application_composition`, `crm_customer_privacy`, `crm_module_sdk`

### [`src/reads.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-application/src/reads.rs) · 1373 lines

- **Public declarations (6):** `plan_read_visibility_resources@L167`, `new@L226`, `plan_read_query_capability_definitions@L585`, `owner_outcome_page_digest@L704`, `owner_outcome_page_digest_for_items@L725`, `owner_outcome_terminal_digest@L763`
- **Internal/trait/helpers (43):** `from_request@L94`, `load_plan_source@L138`, `load_owner_outcomes@L144`, `append_read_audit@L154`, `fmt@L214`, `ensure_active@L240`, `execute_plan_get@L252`, `execute_owner_outcomes@L305`, `authorized_source@L413`, `audit_concealed@L500`, `validate@L536`, `execute@L565`, `query_definition@L600`, `validate_source@L629`, `owner_outcome_cursor_binding@L774`, `decode_outcome_cursor@L799`, `encode_outcome_cursor@L831`, `validate_outcome_page@L851`, `owner_outcome_to_wire@L894`, `authorization_digest@L928`, `append_digest_field@L957`, `outcome_page_size@L962`, `validate_cursor_size@L978`, `cursor_error@L990`, `owner_module_filter@L998`, `decode_input@L1009`, `privacy_case_id@L1040`, `record_ref@L1055`, `ensure_definition@L1062`, `nanos_to_millis@L1073`, `unsupported_query@L1082`, `read_not_found@L1090`, `evidence_invalid@L1099`, `configuration_error@L1107`, `read_error@L1115`, `read_coordinates_and_visibility_are_exact@L1134`, `empty_terminal_page_digests_are_stable_and_filter_sensitive@L1156`, `outcome_bounds_and_cursor_size_are_frozen@L1174`, `outcome_binding@L1195`, `outcome_token@L1218`, `owner_outcome_cursor_round_trip_is_keyset_exact@L1239`, `outcome_cursor_is_bound_to_tenant_case_plan_filter_actor_and_page_size@L1258`, `malformed_tampered_and_out_of_bounds_outcome_cursors_fail_closed@L1324`
- **Public types:** `PrivacyPlanReplayLink@L63`, `PrivacyPlanReadSource@L74`, `PrivacyReadContext@L82`, `PrivacyReadAuditRecord@L109`, `PrivacyOwnerOutcomePosition@L125`, `PrivacyOwnerOutcomePage@L132`, `PrivacyReadPersistencePort@L137`, `CustomerPrivacyReadVisibilityResource@L161`, `CustomerPrivacyPlanReadAdapter@L206`, `PrivacyOwnerOutcomePageDigestContext@L696`
- **Detected tests (6):** `read_coordinates_and_visibility_are_exact@L1134`, `empty_terminal_page_digests_are_stable_and_filter_sensitive@L1156`, `outcome_bounds_and_cursor_size_are_frozen@L1174`, `owner_outcome_cursor_round_trip_is_keyset_exact@L1239`, `outcome_cursor_is_bound_to_tenant_case_plan_filter_actor_and_page_size@L1258`, `malformed_tampered_and_out_of_bounds_outcome_cursors_fail_closed@L1324`
- **Error/guard literals:** `CUSTOMER_PRIVACY_READ_DISABLED`, `QUERY_CURSOR_TOO_LARGE`, `QUERY_CURSOR_INVALID`, `CUSTOMER_PRIVACY_READ_CONTRACT_MISMATCH`, `CUSTOMER_PRIVACY_READ_PROTOBUF_INVALID`, `CUSTOMER_PRIVACY_READ_UNSUPPORTED`, `CUSTOMER_PRIVACY_CASE_NOT_FOUND`, `CUSTOMER_PRIVACY_READ_EVIDENCE_INVALID`, `CUSTOMER_PRIVACY_READ_CONFIGURATION_INVALID`, `QUERY_PAGE_SIZE_EXCEEDS_LIMIT`, `QUERY_PAGE_SIZE_INVALID`
- **Rust crate references:** `crm_application_composition`, `crm_capability_runtime`, `crm_customer_privacy`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`

### [`src/retention.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-application/src/retention.rs) · 252 lines

- **Public declarations (2):** `new@L60`, `evaluate@L70`
- **Internal/trait/helpers (9):** `evaluate_and_persist@L37`, `fmt@L50`, `validate_invocation@L102`, `validate_commit@L138`, `retention_error@L158`, `invocation@L186`, `retention_phase_coordinate_and_trust_boundary_are_exact@L208`, `only_registered_step_six_triggers_are_accepted@L223`, `evaluation_time_must_round_trip_through_postgresql_exactly@L242`
- **Public types:** `RetentionEvaluationInvocation@L15`, `RetentionEvaluationCommit@L31`, `RetentionEvaluationPersistencePort@L36`, `PrivacyRetentionEvaluationService@L44`
- **Detected tests (3):** `retention_phase_coordinate_and_trust_boundary_are_exact@L208`, `only_registered_step_six_triggers_are_accepted@L223`, `evaluation_time_must_round_trip_through_postgresql_exactly@L242`
- **Error/guard literals:** `CONFIGURATION_INVALID`, `DISABLED`, `TRUST_REQUIRED`, `TRIGGER_INVALID`, `TIME_INVALID`, `COMMIT_MISMATCH`, `CUSTOMER_PRIVACY_RETENTION_CONFIGURATION_INVALID`, `CUSTOMER_PRIVACY_RETENTION_DISABLED`, `CUSTOMER_PRIVACY_RETENTION_TRUST_REQUIRED`, `CUSTOMER_PRIVACY_RETENTION_TRIGGER_INVALID`, `CUSTOMER_PRIVACY_RETENTION_TIME_INVALID`, `CUSTOMER_PRIVACY_RETENTION_COMMIT_MISMATCH`, `CUSTOMER_PRIVACY_RETENTION_FAILED`
- **Rust crate references:** `crm_application_composition`, `crm_customer_privacy`, `crm_module_sdk`

## 49. `crm-customer-privacy-cancel-capability-adapter` — END / B-END + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-cancel-capability-adapter/src/lib.rs) · 692 lines

- **Public declarations (4):** `capability_definition@L92`, `capability_definitions@L116`, `privacy_case_ref_from_request@L120`, `cancellation_subject_lock_ids@L129`
- **Internal/trait/helpers (25):** `target@L42`, `plan@L54`, `cancel_command@L143`, `build_plan@L154`, `case_to_wire@L224`, `subject_binding_to_wire@L281`, `rescope_to_wire@L301`, `status_to_wire@L320`, `resume_stage_to_wire@L347`, `kind_to_wire@L356`, `verification_method_to_wire@L365`, `ensure_exact_coordinate@L382`, `positive_version@L400`, `case_ref@L415`, `nanos_to_millis@L429`, `domain_error@L439`, `case_not_found@L454`, `case_state_invalid@L463`, `required@L473`, `invalid_plan@L477`, `configured@L487`, `request@L508`, `snapshot@L554`, `draft_and_verified_cases_cancel_with_exact_evidence@L590`, `stale_terminal_cross_tenant_and_malformed_state_fail_closed@L629`
- **Public types:** `CustomerPrivacyCaseCancelCapabilityPlanner@L39`
- **Detected tests (2):** `draft_and_verified_cases_cancel_with_exact_evidence@L590`, `stale_terminal_cross_tenant_and_malformed_state_fail_closed@L629`
- **Error/guard literals:** `CUSTOMER_PRIVACY_CASE_NOT_FOUND`, `CUSTOMER_PRIVACY_CASE_INVALID`, `CUSTOMER_PRIVACY_CASE_CANCEL_PLAN_INVALID`, `CUSTOMER_PRIVACY_CASE_CANCEL_CONFIGURATION_INVALID`, `CUSTOMER_PRIVACY_VERSION_CONFLICT`, `CUSTOMER_PRIVACY_INVALID_TRANSITION`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_privacy`, `crm_customer_privacy_persistence_adapter`, `crm_module_sdk`, `crm_proto_contracts`

### [`tests/planner.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-cancel-capability-adapter/tests/planner.rs) · 476 lines

- **Internal/trait/helpers (11):** `request@L29`, `new_case@L74`, `snapshot@L86`, `verify_subject@L94`, `target_resolution_and_required_request_fields_are_exact@L108`, `draft_submitted_and_subject_verified_cases_cancel_by_one_exact_version@L143`, `cancellation_preserves_complete_immutable_lineage@L246`, `pending_rescope_is_preserved_and_produces_sorted_deduplicated_subject_locks@L315`, `every_terminal_state_rejects_cancellation@L364`, `completed_case@L406`, `stale_cross_tenant_and_malformed_snapshots_fail_closed@L431`
- **Detected tests (6):** `target_resolution_and_required_request_fields_are_exact@L108`, `draft_submitted_and_subject_verified_cases_cancel_by_one_exact_version@L143`, `cancellation_preserves_complete_immutable_lineage@L246`, `pending_rescope_is_preserved_and_produces_sorted_deduplicated_subject_locks@L315`, `every_terminal_state_rejects_cancellation@L364`, `stale_cross_tenant_and_malformed_snapshots_fail_closed@L431`
- **Error/guard literals:** `CUSTOMER_PRIVACY_CASE_NOT_FOUND`, `CUSTOMER_PRIVACY_INVALID_TRANSITION`, `CUSTOMER_PRIVACY_VERSION_CONFLICT`, `CUSTOMER_PRIVACY_CASE_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_privacy`, `crm_customer_privacy_cancel_capability_adapter`, `crm_customer_privacy_persistence_adapter`, `crm_module_sdk`, `crm_proto_contracts`

## 50. `crm-customer-privacy-capability-adapter` — END / B-END + D? adapter

### [`src/bin/customer_privacy_approval_fixture.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-capability-adapter/src/bin/customer_privacy_approval_fixture.rs) · 276 lines

- **Internal/trait/helpers (6):** `main@L46`, `build_fixture@L92`, `append_fixture_sql@L179`, `append_record_sql@L246`, `sql_literal@L263`, `hex@L267`
- **Error/guard literals:** `ERASURE_REQUEST`
- **Rust crate references:** `crm_customer_privacy`, `crm_module_sdk`

### [`src/control_release.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-capability-adapter/src/control_release.rs) · 674 lines

- **Public declarations (4):** `release_processing_restriction_capability_definition@L169`, `release_customer_data_legal_hold_capability_definition@L178`, `processing_restriction_ref_from_release_request@L223`, `legal_hold_ref_from_release_request@L231`
- **Internal/trait/helpers (27):** `target@L46`, `plan@L62`, `target@L110`, `plan@L126`, `release_control_capability_definitions@L188`, `mutation_definition@L195`, `build_restriction_release_plan@L237`, `build_legal_hold_release_plan@L303`, `processing_restriction_to_wire@L369`, `customer_data_legal_hold_to_wire@L419`, `release_processing_restriction_command@L456`, `release_legal_hold_command@L471`, `processing_restriction_ref@L486`, `legal_hold_ref@L497`, `legal_hold_scope_to_wire@L506`, `data_class_to_wire@L521`, `ensure_exact_coordinate@L535`, `positive_version@L554`, `nanos_to_millis@L566`, `required@L573`, `configured@L577`, `domain_error@L589`, `control_not_found@L608`, `control_state_invalid@L617`, `plan_invalid@L627`, `release_definitions_are_exact_high_risk_idempotent_mutations@L642`, `expected_versions_are_strictly_positive@L668`
- **Public types:** `CustomerPrivacyRestrictionReleaseCapabilityPlanner@L43`, `CustomerPrivacyLegalHoldReleaseCapabilityPlanner@L107`
- **Detected tests (2):** `release_definitions_are_exact_high_risk_idempotent_mutations@L642`, `expected_versions_are_strictly_positive@L668`
- **Error/guard literals:** `CUSTOMER_PRIVACY_CONTROL_RELEASE_CONFIGURATION_INVALID`, `CUSTOMER_PRIVACY_CONTROL_NOT_FOUND`, `CUSTOMER_PRIVACY_CONTROL_STATE_INVALID`, `CUSTOMER_PRIVACY_CONTROL_RELEASE_PLAN_INVALID`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_privacy`, `crm_customer_privacy_persistence_adapter`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/legal_hold.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-capability-adapter/src/legal_hold.rs) · 548 lines

- **Public declarations (4):** `place_customer_data_legal_hold_capability_definition@L87`, `deterministic_customer_data_legal_hold_id@L114`, `customer_data_legal_hold_canonical_party_id_from_request@L127`, `customer_data_legal_hold_from_place_request@L137`
- **Internal/trait/helpers (24):** `target@L35`, `plan@L48`, `legal_hold_from_command@L158`, `plan_legal_hold_place@L190`, `legal_hold_to_wire@L247`, `decode_place_request@L286`, `ensure_definition@L297`, `canonical_party_id_from_wire@L316`, `legal_hold_scope_from_wire@L327`, `legal_hold_scope_to_wire@L362`, `data_class_from_wire@L377`, `data_class_to_wire@L395`, `policy_version@L409`, `millis_to_nanos@L424`, `nanos_to_millis@L436`, `update_length_framed@L443`, `hex@L448`, `configured@L458`, `configuration_error@L462`, `domain_error@L472`, `plan_invalid@L491`, `definition_is_exact_high_risk_personal_mutation@L506`, `deterministic_identity_is_tenant_and_idempotency_bound@L525`, `nanosecond_values_are_truncated_to_wire_milliseconds@L536`
- **Public types:** `CustomerPrivacyLegalHoldPlaceCapabilityPlanner@L32`
- **Detected tests (3):** `definition_is_exact_high_risk_personal_mutation@L506`, `deterministic_identity_is_tenant_and_idempotency_bound@L525`, `nanosecond_values_are_truncated_to_wire_milliseconds@L536`
- **Error/guard literals:** `CUSTOMER_PRIVACY_LEGAL_HOLD_CONFIGURATION_INVALID`, `CUSTOMER_PRIVACY_LEGAL_HOLD_PLAN_INVALID`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_privacy`, `crm_customer_privacy_persistence_adapter`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-capability-adapter/src/lib.rs) · 761 lines

- **Public declarations (8):** `capability_definition@L138`, `capability_definitions@L162`, `deterministic_privacy_case_id@L168`, `privacy_case_from_create_request@L181`, `previous_case_id_from_request@L212`, `validate_previous_case_snapshot@L226`, `privacy_case_ref_from_id@L412`, `previous_case_not_found@L460`
- **Internal/trait/helpers (28):** `target@L53`, `plan@L66`, `target@L95`, `plan@L114`, `decode_create_request@L260`, `plan_case_create@L266`, `privacy_case_to_wire@L319`, `ensure_definition@L352`, `privacy_case_kind_from_wire@L370`, `privacy_case_kind_to_wire@L383`, `policy_version@L392`, `privacy_case_id_from_wire@L404`, `nanos_to_millis@L420`, `update_length_framed@L430`, `hex@L435`, `domain_error@L445`, `plan_invalid@L469`, `configured@L479`, `configuration_error@L483`, `request@L505`, `request_with@L520`, `predecessor_snapshot@L568`, `deterministic_identity_is_tenant_and_idempotency_bound@L589`, `root_create_is_one_confidential_atomic_draft_plan@L606`, `executable_lineage_target_remains_the_new_absent_successor@L656`, `reference_planner_requires_exact_terminal_predecessor@L673`, `nonterminal_cross_tenant_and_malformed_predecessors_fail_closed@L705`, `invalid_enum_policy_and_timestamp_produce_no_plan@L732`
- **Public types:** `CustomerPrivacyCaseCreateCapabilityPlanner@L50`, `CustomerPrivacyCasePreviousReferencePlanner@L92`
- **Detected tests (6):** `deterministic_identity_is_tenant_and_idempotency_bound@L589`, `root_create_is_one_confidential_atomic_draft_plan@L606`, `executable_lineage_target_remains_the_new_absent_successor@L656`, `reference_planner_requires_exact_terminal_predecessor@L673`, `nonterminal_cross_tenant_and_malformed_predecessors_fail_closed@L705`, `invalid_enum_policy_and_timestamp_produce_no_plan@L732`
- **Error/guard literals:** `CUSTOMER_PRIVACY_PREVIOUS_CASE_INVALID`, `CUSTOMER_PRIVACY_PREVIOUS_CASE_NOT_TERMINAL`, `CUSTOMER_PRIVACY_PREVIOUS_CASE_NOT_FOUND`, `CUSTOMER_PRIVACY_CASE_CREATE_PLAN_INVALID`, `CUSTOMER_PRIVACY_CASE_CREATE_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_privacy`, `crm_customer_privacy_persistence_adapter`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/restriction.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-capability-adapter/src/restriction.rs) · 482 lines

- **Public declarations (4):** `place_processing_restriction_capability_definition@L85`, `deterministic_processing_restriction_id@L112`, `processing_restriction_canonical_party_id_from_request@L125`, `processing_restriction_from_place_request@L135`
- **Internal/trait/helpers (22):** `target@L35`, `plan@L48`, `processing_restriction_from_command@L156`, `plan_restriction_place@L182`, `processing_restriction_to_wire@L242`, `decode_place_request@L281`, `ensure_definition@L292`, `canonical_party_id_from_wire@L311`, `restriction_scope_from_wire@L322`, `restriction_scope_to_wire@L338`, `policy_version@L348`, `millis_to_nanos@L363`, `nanos_to_millis@L375`, `update_length_framed@L382`, `hex@L387`, `configured@L397`, `configuration_error@L401`, `domain_error@L411`, `plan_invalid@L430`, `definition_is_exact_high_risk_personal_mutation@L445`, `deterministic_identity_is_tenant_and_idempotency_bound@L464`, `nanosecond_request_timestamp_is_truncated_to_wire_milliseconds@L474`
- **Public types:** `CustomerPrivacyRestrictionPlaceCapabilityPlanner@L32`
- **Detected tests (3):** `definition_is_exact_high_risk_personal_mutation@L445`, `deterministic_identity_is_tenant_and_idempotency_bound@L464`, `nanosecond_request_timestamp_is_truncated_to_wire_milliseconds@L474`
- **Error/guard literals:** `CUSTOMER_PRIVACY_RESTRICTION_CONFIGURATION_INVALID`, `CUSTOMER_PRIVACY_RESTRICTION_PLAN_INVALID`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_privacy`, `crm_customer_privacy_persistence_adapter`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/root.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-capability-adapter/src/root.rs) · 69 lines

- **Public declarations (1):** `capability_definitions@L36`
- **Internal/trait/helpers (2):** `implemented_capability_definitions@L41`, `implemented_inventory_contains_case_create_and_complete_control_mutations@L56`
- **Re-exports:** `pub use case_create::{`, `pub use control_release::*;`, `pub use legal_hold::*;`, `pub use restriction::*;`
- **Detected tests (1):** `implemented_inventory_contains_case_create_and_complete_control_mutations@L56`
- **Rust crate references:** `crm_capability_runtime`, `crm_module_sdk`

### [`tests/legal_hold_public_contract.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-capability-adapter/tests/legal_hold_public_contract.rs) · 69 lines

- **Internal/trait/helpers (2):** `legal_hold_place_definition_is_exact_and_fail_closed@L8`, `legal_hold_identity_is_stable_tenant_and_idempotency_bound@L54`
- **Detected tests (2):** `legal_hold_place_definition_is_exact_and_fail_closed@L8`, `legal_hold_identity_is_stable_tenant_and_idempotency_bound@L54`
- **Rust crate references:** `crm_customer_privacy_capability_adapter`, `crm_module_sdk`

### [`tests/restriction_public_contract.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-capability-adapter/tests/restriction_public_contract.rs) · 55 lines

- **Internal/trait/helpers (2):** `restriction_place_definition_is_exact_and_fail_closed@L8`, `restriction_identity_is_stable_and_tenant_bound@L40`
- **Detected tests (2):** `restriction_place_definition_is_exact_and_fail_closed@L8`, `restriction_identity_is_stable_and_tenant_bound@L40`
- **Rust crate references:** `crm_customer_privacy_capability_adapter`, `crm_module_sdk`

## 51. `crm-customer-privacy-capability-composition` — END / B-END + D? adapter

### [`src/approval.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-capability-composition/src/approval.rs) · 1102 lines

- **Public declarations (4):** `approval_capability_definition@L98`, `privacy_case_ref_from_request@L122`, `expected_version_from_request@L131`, `postgres_case_approval_executor@L283`
- **Internal/trait/helpers (42):** `target@L47`, `plan@L59`, `approval_command@L135`, `build_plan@L148`, `check@L228`, `validate_exact_lineage@L293`, `load_case_in_transaction@L349`, `load_snapshot_in_transaction@L359`, `load_plan_in_transaction@L369`, `select_record_row@L378`, `decode_case_row@L419`, `decode_snapshot_row@L426`, `decode_plan_row@L449`, `decode_record_snapshot@L472`, `validate_immutable_contract@L537`, `verify_planning_link_in_transaction@L561`, `privacy_case_to_wire@L627`, `subject_binding_to_wire@L684`, `rescope_to_wire@L704`, `status_to_wire@L723`, `resume_stage_to_wire@L750`, `kind_to_wire@L759`, `verification_method_to_wire@L768`, `ensure_exact_coordinate@L785`, `positive_version@L803`, `case_ref@L818`, `nanos_to_millis@L832`, `domain_error@L842`, `database_error@L857`, `case_not_found@L867`, `version_conflict@L876`, `approval_conflict@L888`, `approval_evidence_invalid@L898`, `approval_guard_unsupported@L908`, `case_state_invalid@L917`, `required@L927`, `invalid_plan@L931`, `configured@L941`, `request@L962`, `awaiting_approval_snapshot@L1002`, `awaiting_approval_case_advances_with_exact_append_only_evidence@L1045`, `stale_and_ineligible_cases_fail_closed@L1069`
- **Public types:** `CustomerPrivacyCaseApprovalCapabilityPlanner@L44`, `PostgresCustomerPrivacyApprovalGuard@L225`
- **Detected tests (2):** `awaiting_approval_case_advances_with_exact_append_only_evidence@L1045`, `stale_and_ineligible_cases_fail_closed@L1069`
- **Error/guard literals:** `CUSTOMER_PRIVACY_APPROVAL_STORE_UNAVAILABLE`, `CUSTOMER_PRIVACY_CASE_NOT_FOUND`, `CUSTOMER_PRIVACY_VERSION_CONFLICT`, `CUSTOMER_PRIVACY_APPROVAL_CONFLICT`, `CUSTOMER_PRIVACY_APPROVAL_EVIDENCE_INVALID`, `CUSTOMER_PRIVACY_APPROVAL_GUARD_UNSUPPORTED`, `CUSTOMER_PRIVACY_CASE_INVALID`, `CUSTOMER_PRIVACY_CASE_APPROVAL_PLAN_INVALID`, `CUSTOMER_PRIVACY_CASE_APPROVAL_CONFIGURATION_INVALID`, `CUSTOMER_PRIVACY_INVALID_TRANSITION`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_privacy`, `crm_customer_privacy_persistence_adapter`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/control_release.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-capability-composition/src/control_release.rs) · 212 lines

- **Public declarations (2):** `postgres_restriction_release_executor@L93`, `postgres_legal_hold_release_executor@L103`
- **Internal/trait/helpers (10):** `check@L24`, `check@L61`, `load_control_payload@L113`, `ensure_release_coordinate@L144`, `control_not_found@L162`, `control_state_unavailable@L171`, `control_store_unavailable@L181`, `subject_lock_unavailable@L191`, `release_guards_are_shareable_transaction_guards@L206`, `require_guard@L207`
- **Detected tests (2):** `release_guards_are_shareable_transaction_guards@L206`, `require_guard@L207`
- **Error/guard literals:** `CUSTOMER_PRIVACY_CONTROL_RELEASE_GUARD_INVALID`, `CUSTOMER_PRIVACY_CONTROL_NOT_FOUND`, `CUSTOMER_PRIVACY_CONTROL_STATE_UNAVAILABLE`, `CUSTOMER_PRIVACY_CONTROL_STORE_UNAVAILABLE`, `CUSTOMER_PRIVACY_CONTROL_SUBJECT_LOCK_UNAVAILABLE`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_privacy`, `crm_customer_privacy_capability_adapter`, `crm_module_sdk`, `crm_party_reference_composition`

### [`src/legacy.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-capability-composition/src/legacy.rs) · 508 lines

- **Public declarations (4):** `postgres_case_create_executor@L181`, `postgres_case_submit_executor@L191`, `postgres_case_subject_verify_executor@L200`, `postgres_case_cancel_executor@L210`
- **Internal/trait/helpers (18):** `check@L46`, `check@L71`, `check@L144`, `load_cancellation_snapshot@L220`, `select_case_row@L249`, `required_party_reference@L306`, `map_subject_proof_error@L317`, `map_subject_lock_error@L354`, `map_cancellation_lock_error@L364`, `subject_guard_unsupported@L374`, `cancellation_guard_unsupported@L383`, `cancellation_subject_changed@L392`, `subject_proof_invalid@L401`, `decode_snapshot@L411`, `cancellation_case_not_found@L470`, `cancellation_state_invalid@L479`, `reference_store_unavailable@L489`, `reference_state_invalid@L499`
- **Public types:** `PostgresCustomerPrivacyPreviousCaseGuard@L43`, `PostgresCustomerPrivacySubjectVerificationGuard@L68`, `PostgresCustomerPrivacyCancellationGuard@L141`
- **Error/guard literals:** `PARTY_REFERENCE_UNAVAILABLE`, `CUSTOMER_PRIVACY_SUBJECT_REFERENCE_UNAVAILABLE`, `IDENTITY_RESOLUTION_TOPOLOGY_GENERATION_STALE`, `CUSTOMER_PRIVACY_SUBJECT_GENERATION_STALE`, `IDENTITY_RESOLUTION_CANONICAL_PARTY_MISMATCH`, `CUSTOMER_PRIVACY_SUBJECT_CANONICAL_REFERENCE_INVALID`, `IDENTITY_RESOLUTION_CANONICAL_REDIRECT_INVALID`, `CUSTOMER_PRIVACY_SUBJECT_TOPOLOGY_INVALID`, `CUSTOMER_PRIVACY_SUBJECT_PROOF_UNAVAILABLE`, `CUSTOMER_PRIVACY_SUBJECT_LOCK_UNAVAILABLE`, `CUSTOMER_PRIVACY_CANCELLATION_SUBJECT_LOCK_UNAVAILABLE`, `CUSTOMER_PRIVACY_SUBJECT_GUARD_UNSUPPORTED`, `CUSTOMER_PRIVACY_CANCELLATION_GUARD_UNSUPPORTED`, `CUSTOMER_PRIVACY_CANCELLATION_SUBJECT_CHANGED`, `CUSTOMER_PRIVACY_SUBJECT_PROOF_INVALID`, `CUSTOMER_PRIVACY_CASE_NOT_FOUND`, `CUSTOMER_PRIVACY_CASE_INVALID`, `CUSTOMER_PRIVACY_CASE_STORE_UNAVAILABLE`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_privacy`, `crm_customer_privacy_cancel_capability_adapter`, `crm_customer_privacy_capability_adapter`, `crm_customer_privacy_persistence_adapter`, `crm_customer_privacy_subject_capability_adapter`, `crm_customer_privacy_submit_capability_adapter`, `crm_identity_resolution`, `crm_identity_resolution_topology_composition`, `crm_module_sdk`, `crm_party_reference_composition`, `crm_proto_contracts`

### [`src/legal_hold.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-capability-composition/src/legal_hold.rs) · 108 lines

- **Public declarations (1):** `postgres_legal_hold_place_executor@L52`
- **Internal/trait/helpers (6):** `check@L25`, `ensure_place_coordinate@L62`, `canonical_party_unavailable@L78`, `subject_lock_unavailable@L88`, `placement_guard_is_shareable_transaction_guard@L103`, `require_guard@L104`
- **Public types:** `PostgresCustomerPrivacyLegalHoldPlacementGuard@L22`
- **Detected tests (2):** `placement_guard_is_shareable_transaction_guard@L103`, `require_guard@L104`
- **Error/guard literals:** `CUSTOMER_PRIVACY_LEGAL_HOLD_PLACEMENT_GUARD_INVALID`, `CUSTOMER_PRIVACY_LEGAL_HOLD_CANONICAL_PARTY_UNAVAILABLE`, `CUSTOMER_PRIVACY_LEGAL_HOLD_SUBJECT_LOCK_UNAVAILABLE`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_privacy`, `crm_customer_privacy_capability_adapter`, `crm_identity_resolution_topology_composition`, `crm_module_sdk`, `crm_party_reference_composition`

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-capability-composition/src/lib.rs) · 14 lines

- **Re-exports:** `pub use approval::*;`, `pub use legacy::*;`

### [`src/restriction.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-capability-composition/src/restriction.rs) · 109 lines

- **Public declarations (1):** `postgres_restriction_place_executor@L53`
- **Internal/trait/helpers (6):** `check@L26`, `ensure_place_coordinate@L63`, `canonical_party_unavailable@L79`, `subject_lock_unavailable@L89`, `placement_guard_is_shareable_transaction_guard@L104`, `require_guard@L105`
- **Public types:** `PostgresCustomerPrivacyRestrictionPlacementGuard@L23`
- **Detected tests (2):** `placement_guard_is_shareable_transaction_guard@L104`, `require_guard@L105`
- **Error/guard literals:** `CUSTOMER_PRIVACY_RESTRICTION_PLACEMENT_GUARD_INVALID`, `CUSTOMER_PRIVACY_RESTRICTION_CANONICAL_PARTY_UNAVAILABLE`, `CUSTOMER_PRIVACY_RESTRICTION_SUBJECT_LOCK_UNAVAILABLE`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_privacy`, `crm_customer_privacy_capability_adapter`, `crm_identity_resolution_topology_composition`, `crm_module_sdk`, `crm_party_reference_composition`

### [`src/root.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-capability-composition/src/root.rs) · 13 lines

- **Re-exports:** `pub use base::*;`, `pub use control_release::*;`, `pub use legal_hold::*;`, `pub use restriction::*;`

### [`tests/postgres_case_create.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-capability-composition/tests/postgres_case_create.rs) · 637 lines

- **Internal/trait/helpers (11):** `postgres_case_create_is_atomic_replay_safe_and_tenant_isolated@L45`, `request@L338`, `decode_output@L379`, `assert_record_metadata@L386`, `assert_committed_evidence@L428`, `assert_no_evidence@L480`, `record_count@L507`, `evidence_count@L519`, `make_predecessor_terminal@L533`, `governed_replace_payload@L556`, `bind_write_context@L615`
- **Detected tests (1):** `postgres_case_create_is_atomic_replay_safe_and_tenant_isolated@L45`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`, `CAPABILITY_IDEMPOTENCY_KEY_REUSED`, `CUSTOMER_PRIVACY_PREVIOUS_CASE_NOT_TERMINAL`, `CUSTOMER_PRIVACY_PREVIOUS_CASE_NOT_FOUND`, `CUSTOMER_PRIVACY_PREVIOUS_CASE_INVALID`, `SELECT`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_privacy`, `crm_customer_privacy_capability_adapter`, `crm_customer_privacy_capability_composition`, `crm_customer_privacy_persistence_adapter`, `crm_module_sdk`, `crm_proto_contracts`

### [`tests/postgres_case_subject_verify.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-capability-composition/tests/postgres_case_subject_verify.rs) · 741 lines

- **Internal/trait/helpers (15):** `subject_verify_is_atomic_replay_safe_and_authoritative@L51`, `failure_scenarios@L317`, `create_party@L372`, `create_case@L403`, `submitted_case@L432`, `verify_request@L468`, `request@L505`, `decode_verify@L541`, `assert_verified_record@L548`, `assert_failure@L571`, `assert_success_evidence@L632`, `record_version@L666`, `count_for_transaction@L678`, `corrupt_payload@L692`, `bind_context@L717`
- **Detected tests (1):** `subject_verify_is_atomic_replay_safe_and_authoritative@L51`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`, `CAPABILITY_IDEMPOTENCY_KEY_REUSED`, `CUSTOMER_PRIVACY_VERSION_CONFLICT`, `CUSTOMER_PRIVACY_INVALID_TRANSITION`, `CUSTOMER_PRIVACY_CASE_INVALID`, `CUSTOMER_PRIVACY_SUBJECT_LOCK_UNAVAILABLE`, `CUSTOMER_PRIVACY_SUBJECT_REFERENCE_UNAVAILABLE`, `CUSTOMER_PRIVACY_SUBJECT_CANONICAL_REFERENCE_INVALID`, `CUSTOMER_PRIVACY_SUBJECT_GENERATION_STALE`, `SELECT`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_privacy`, `crm_customer_privacy_capability_adapter`, `crm_customer_privacy_capability_composition`, `crm_customer_privacy_persistence_adapter`, `crm_customer_privacy_subject_capability_adapter`, `crm_customer_privacy_submit_capability_adapter`, `crm_module_sdk`, `crm_parties_capability_adapter`, `crm_proto_contracts`

### [`tests/postgres_case_submit.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-capability-composition/tests/postgres_case_submit.rs) · 586 lines

- **Internal/trait/helpers (13):** `postgres_case_submit_is_atomic_replay_safe_and_fail_closed@L33`, `create_draft@L268`, `create_request@L295`, `submit_request@L321`, `capability_request@L350`, `decode_submit@L391`, `assert_submitted_record@L398`, `assert_submit_evidence@L420`, `assert_no_submit_evidence@L472`, `assert_record_version@L500`, `count_transaction_rows@L513`, `corrupt_record_payload@L527`, `bind_write_context@L564`
- **Detected tests (1):** `postgres_case_submit_is_atomic_replay_safe_and_fail_closed@L33`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`, `CAPABILITY_IDEMPOTENCY_KEY_REUSED`, `CUSTOMER_PRIVACY_VERSION_CONFLICT`, `CUSTOMER_PRIVACY_INVALID_TRANSITION`, `CUSTOMER_PRIVACY_CASE_INVALID`, `SELECT`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_privacy`, `crm_customer_privacy_capability_adapter`, `crm_customer_privacy_capability_composition`, `crm_customer_privacy_persistence_adapter`, `crm_customer_privacy_submit_capability_adapter`, `crm_module_sdk`, `crm_proto_contracts`

## 52. `crm-customer-privacy-owner-scope-support` — END / B-END + D? adapter

### [`src/digest.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-owner-scope-support/src/digest.rs) · 25 lines

- **Public declarations (2):** `framed_digest@L8`, `append_frame@L21`

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-owner-scope-support/src/lib.rs) · 32 lines

- **Re-exports:** `pub use crm_customer_privacy::PrivacyOwnerActionCommand;`, `pub use digest::{append_frame, framed_digest};`, `pub use owner_action::{`, `pub use postgres::{CanonicalPartyClaimError, prove_canonical_party_claim};`, `pub use request::{`
- **Rust crate references:** `crm_customer_privacy`

### [`src/owner_action.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-owner-scope-support/src/owner_action.rs) · 372 lines

- **Public declarations (4):** `new@L52`, `owner_action_definition@L112`, `owner_action_input_payload@L150`, `unsupported_owner_action@L170`
- **Internal/trait/helpers (20):** `owner_module_id@L29`, `capability_id@L30`, `supports_resource_type@L31`, `anonymize@L33`, `deletion_tombstone@L39`, `target@L61`, `plan@L79`, `validate_request@L180`, `validate_locked_snapshot@L217`, `validate_owner_payload@L232`, `owner_event@L245`, `owner_audit@L287`, `deterministic_id@L299`, `append_frame@L312`, `hex@L317`, `action_configuration_invalid@L326`, `action_invalid@L336`, `action_stale@L346`, `action_unsupported@L355`, `crypto_shred_unavailable@L364`
- **Public types:** `OwnerPrivacyActionPolicy@L28`, `OwnerPrivacyActionPlanner@L47`
- **Error/guard literals:** `PRIVACY_OWNER_ACTION_CONFIGURATION_INVALID`, `PRIVACY_OWNER_ACTION_COMMAND_REJECTED`, `PRIVACY_OWNER_ACTION_STALE`, `PRIVACY_OWNER_ACTION_UNSUPPORTED`, `PRIVACY_OWNER_CRYPTO_SHRED_UNAVAILABLE`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_privacy`, `crm_module_sdk`

### [`src/postgres.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-owner-scope-support/src/postgres.rs) · 93 lines

- **Public declarations (1):** `prove_canonical_party_claim@L19`
- **Public types:** `CanonicalPartyClaimError@L11`
- **Rust crate references:** `crm_core_data`, `crm_identity_resolution_capability_adapter`, `crm_module_sdk`, `crm_parties`, `crm_parties_capability_adapter`

### [`src/request.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-owner-scope-support/src/request.rs) · 145 lines

- **Public declarations (2):** `validate_query_request_contract@L42`, `validate_common_lineage@L72`
- **Internal/trait/helpers (1):** `valid_purpose_code@L138`
- **Public types:** `QueryRequestContractError@L11`, `ValidatedCommonLineage@L20`, `CommonLineageError@L28`
- **Rust crate references:** `crm_capability_runtime`, `crm_customer_privacy`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`

### [`src/tests.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-owner-scope-support/src/tests.rs) · 262 lines

- **Internal/trait/helpers (12):** `context@L23`, `definition@L37`, `query_request@L61`, `valid_query_request@L82`, `lineage@L88`, `framed_digest_preserves_field_boundaries_and_domain@L105`, `query_request_contract_accepts_exact_binding_contract_and_hash@L115`, `query_request_contract_rejects_every_integrity_boundary@L120`, `common_lineage_defaults_page_size_and_preserves_identity@L167`, `common_lineage_rejects_identity_and_registry_substitution@L176`, `common_lineage_rejects_invalid_purpose_time_and_page_size@L233`, `validated_common_lineage_keeps_the_exact_canonical_party_id@L255`
- **Detected tests (7):** `framed_digest_preserves_field_boundaries_and_domain@L105`, `query_request_contract_accepts_exact_binding_contract_and_hash@L115`, `query_request_contract_rejects_every_integrity_boundary@L120`, `common_lineage_defaults_page_size_and_preserves_identity@L167`, `common_lineage_rejects_identity_and_registry_substitution@L176`, `common_lineage_rejects_invalid_purpose_time_and_page_size@L233`, `validated_common_lineage_keeps_the_exact_canonical_party_id@L255`
- **Error/guard literals:** `PRIVACY_ERASURE_SCOPE`
- **Rust crate references:** `crm_capability_runtime`, `crm_customer_privacy`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`

### [`tests/digest_compatibility.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-owner-scope-support/tests/digest_compatibility.rs) · 101 lines

- **Internal/trait/helpers (2):** `parties_digests_match_the_accepted_pre_extraction_protocol@L9`, `consents_digests_match_the_accepted_pre_extraction_protocol@L51`
- **Detected tests (2):** `parties_digests_match_the_accepted_pre_extraction_protocol@L9`, `consents_digests_match_the_accepted_pre_extraction_protocol@L51`
- **Rust crate references:** `crm_customer_privacy_owner_scope_support`

## 53. `crm-customer-privacy-persistence-adapter` — END / B-END + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-persistence-adapter/src/lib.rs) · 349 lines

- **Public declarations (12):** `privacy_case_record_ref@L25`, `processing_restriction_record_ref@L33`, `legal_hold_record_ref@L43`, `privacy_case_persisted_contract@L51`, `processing_restriction_persisted_contract@L62`, `legal_hold_persisted_contract@L73`, `privacy_case_persisted_payload@L84`, `processing_restriction_persisted_payload@L92`, `legal_hold_persisted_payload@L102`, `privacy_case_from_snapshot@L112`, `processing_restriction_from_snapshot@L129`, `legal_hold_from_snapshot@L148`
- **Internal/trait/helpers (13):** `ensure_record_type@L167`, `ensure_snapshot_identity_and_version@L180`, `adapter_error@L204`, `record_id@L220`, `tenant_id@L224`, `actor_id@L228`, `policy_version@L232`, `privacy_case@L236`, `restriction@L248`, `legal_hold@L263`, `privacy_case_payload_and_snapshot_round_trip_are_exact@L280`, `restriction_and_legal_hold_snapshots_round_trip@L296`, `rehydration_rejects_record_identity_version_and_contract_drift@L318`
- **Re-exports:** `pub use retention::*;`
- **Detected tests (3):** `privacy_case_payload_and_snapshot_round_trip_are_exact@L280`, `restriction_and_legal_hold_snapshots_round_trip@L296`, `rehydration_rejects_record_identity_version_and_contract_drift@L318`
- **Error/guard literals:** `CUSTOMER_PRIVACY_PERSISTENCE_ADAPTER_INVALID`, `LITIGATION_HOLD`
- **Rust crate references:** `crm_capability_plan_support`, `crm_customer_privacy`, `crm_module_sdk`

### [`src/retention.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-persistence-adapter/src/retention.rs) · 100 lines

- **Public declarations (4):** `retention_decision_record_ref@L11`, `retention_decision_persisted_contract@L21`, `retention_decision_persisted_payload@L32`, `retention_decision_from_snapshot@L42`
- **Internal/trait/helpers (2):** `retention_adapter_error@L66`, `retention_record_contract_is_personal_and_immutable_v1@L81`
- **Detected tests (1):** `retention_record_contract_is_personal_and_immutable_v1@L81`
- **Error/guard literals:** `CUSTOMER_PRIVACY_RETENTION_PERSISTENCE_INVALID`
- **Rust crate references:** `crm_capability_plan_support`, `crm_customer_privacy`, `crm_module_sdk`

## 54. `crm-customer-privacy-postgres` — END / B-END + D? adapter

### [`src/access_export.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-postgres/src/access_export.rs) · 1023 lines

- **Public declarations (2):** `new@L49`, `store@L53`
- **Internal/trait/helpers (28):** `prepare@L59`, `complete@L130`, `apply_target_result@L209`, `load_locked_source@L229`, `validate_case_plan_lineage@L278`, `validate_complete_checkpoint@L306`, `bind_invocation@L359`, `bind_business_transaction@L383`, `lock_customer_subject@L395`, `load_case@L409`, `load_plan@L438`, `load_reference@L472`, `insert_reference@L518`, `update_reference@L561`, `insert_access_export_transaction_evidence@L608`, `access_export_transaction_id@L784`, `access_export_request_hash@L805`, `access_export_transaction_audit_hash@L829`, `decode_record_snapshot@L858`, `validate_contract@L916`, `digest_column@L939`, `append_digest_field@L946`, `checked_i64@L951`, `hex@L955`, `database_error@L985`, `case_not_found@L995`, `access_export_conflict@L1004`, `access_export_evidence_invalid@L1014`
- **Public types:** `PostgresAccessExportPersistence@L44`
- **Error/guard literals:** `CUSTOMER_PRIVACY_ACCESS_EXPORT_STORE_UNAVAILABLE`, `CUSTOMER_PRIVACY_CASE_NOT_FOUND`, `CUSTOMER_PRIVACY_ACCESS_EXPORT_CONFLICT`, `CUSTOMER_PRIVACY_ACCESS_EXPORT_EVIDENCE_INVALID`
- **Rust crate references:** `crm_core_data`, `crm_customer_privacy`, `crm_customer_privacy_application`, `crm_customer_privacy_persistence_adapter`, `crm_module_sdk`

### [`src/discovery.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-postgres/src/discovery.rs) · 797 lines

- **Public declarations (2):** `new@L31`, `store@L35`
- **Internal/trait/helpers (20):** `begin_attempt@L41`, `load_owner_pages@L138`, `accept_owner_page@L169`, `advance_checkpoint@L251`, `finalize_snapshot@L340`, `load_snapshot@L454`, `record_audit@L469`, `bind_invocation@L520`, `bind_tenant@L547`, `load_snapshot_in_transaction@L559`, `decode_page@L627`, `digest_column@L673`, `audit_digest@L680`, `frame@L715`, `hex@L720`, `database_error@L730`, `evidence_conflict@L740`, `identifier_error@L750`, `audit_digest_excludes_resource_payload_and_is_deterministic@L761`, `attempt_digest_matches_application_identity@L778`
- **Public types:** `PostgresDiscoveryPersistence@L26`
- **Detected tests (2):** `audit_digest_excludes_resource_payload_and_is_deterministic@L761`, `attempt_digest_matches_application_identity@L778`
- **Error/guard literals:** `CUSTOMER_PRIVACY_DISCOVERY_STORAGE_UNAVAILABLE`, `CUSTOMER_PRIVACY_DISCOVERY_PERSISTED_EVIDENCE_INVALID`, `ERASURE`
- **Rust crate references:** `crm_core_data`, `crm_customer_privacy`, `crm_customer_privacy_application`, `crm_module_sdk`

### [`src/execution.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-postgres/src/execution.rs) · 1878 lines

- **Public declarations (3):** `new@L46`, `store@L50`, `outcome_from_row@L1012`
- **Internal/trait/helpers (53):** `prepare_next@L56`, `record_outcome@L178`, `advance_checkpoint@L231`, `complete@L282`, `bind_invocation@L289`, `lock_customer_subject@L315`, `load_execution_source@L329`, `validate_execution_source@L381`, `initialize_checkpoint@L435`, `normalize_checkpoint@L507`, `validate_checkpoint@L595`, `insert_checkpoint@L633`, `update_checkpoint@L684`, `load_checkpoint@L720`, `checkpoint_from_row@L745`, `insert_attempt@L768`, `insert_outcome@L828`, `load_latest_attempt_with_outcome@L878`, `load_attempt_by_id@L931`, `load_outcome_by_id@L958`, `attempt_from_row@L985`, `validate_execution_envelope@L1040`, `validate_attempt_invocation@L1079`, `validate_outcome_attempt@L1095`, `append_execution_audit@L1118`, `execution_audit_digest@L1183`, `load_case@L1217`, `load_plan@L1247`, `load_decision@L1286`, `update_case_record@L1330`, `decode_record_snapshot@L1397`, `validate_record_contract@L1455`, `data_class_label@L1478`, `positive_u64@L1492`, `optional_positive_u64@L1500`, `bounded_u32@L1515`, `positive_i64@L1530`, `digest@L1538`, `identifier@L1545`, `checked_i64@L1549`, `append_digest_field@L1553`, `bind_business_transaction@L1558`, `insert_case_transition_evidence@L1570`, `case_transition_request_hash@L1734`, `case_transition_audit_hash@L1763`, `case_transition_transaction_id@L1792`, `transaction_id@L1813`, `hex@L1826`, `case_not_found@L1836`, `domain_error@L1845`, `execution_conflict@L1849`, `evidence_invalid@L1859`, `database_error@L1869`
- **Public types:** `PostgresOwnerExecutionPersistence@L41`
- **Error/guard literals:** `CUSTOMER_PRIVACY_CASE_NOT_FOUND`, `CUSTOMER_PRIVACY_OWNER_EXECUTION_CONFLICT`, `CUSTOMER_PRIVACY_OWNER_EXECUTION_EVIDENCE_INVALID`, `CUSTOMER_PRIVACY_OWNER_EXECUTION_STORE_UNAVAILABLE`
- **Rust crate references:** `crm_core_data`, `crm_customer_privacy`, `crm_customer_privacy_application`, `crm_customer_privacy_persistence_adapter`, `crm_module_sdk`

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-postgres/src/lib.rs) · 44 lines

- **Re-exports:** `pub use access_export::*;`, `pub use discovery::*;`, `pub use execution::*;`, `pub use planning::*;`, `pub use reads::*;`, `pub use restrictions::*;`, `pub use retention::*;`, `pub use crm_customer_privacy_capability_composition::{`, `pub use crm_customer_privacy_persistence_adapter::{`
- **Rust crate references:** `crm_customer_privacy_capability_composition`, `crm_customer_privacy_persistence_adapter`

### [`src/planning.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-postgres/src/planning.rs) · 875 lines

- **Public declarations (2):** `new@L35`, `store@L39`
- **Internal/trait/helpers (28):** `load_source@L45`, `finalize_plan@L95`, `load_case_in_transaction@L270`, `load_snapshot_in_transaction@L288`, `load_plan_in_transaction@L306`, `select_record_row@L324`, `decode_case_row@L365`, `decode_snapshot_row@L373`, `decode_plan_row@L396`, `decode_record_snapshot@L419`, `validate_contract@L480`, `insert_plan_record_in_transaction@L503`, `update_case_record_in_transaction@L540`, `insert_link_in_transaction@L596`, `verify_link_in_transaction@L634`, `insert_audit_in_transaction@L691`, `bind_invocation@L729`, `transaction_id@L753`, `planning_audit_digest@L758`, `digest_column@L783`, `hex@L790`, `database_error@L800`, `case_not_found@L810`, `planning_conflict@L819`, `planning_evidence_invalid@L829`, `planning_state_invalid@L839`, `domain_error@L843`, `planning_transaction_identity_is_deterministic@L852`
- **Public types:** `PostgresPlanningPersistence@L30`
- **Detected tests (1):** `planning_transaction_identity_is_deterministic@L852`
- **Error/guard literals:** `CUSTOMER_PRIVACY_PLANNING_STORE_UNAVAILABLE`, `CUSTOMER_PRIVACY_CASE_NOT_FOUND`, `CUSTOMER_PRIVACY_PLANNING_CONFLICT`, `CUSTOMER_PRIVACY_PLANNING_EVIDENCE_INVALID`
- **Rust crate references:** `crm_core_data`, `crm_customer_privacy`, `crm_customer_privacy_application`, `crm_customer_privacy_persistence_adapter`, `crm_module_sdk`

### [`src/reads.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-postgres/src/reads.rs) · 606 lines

- **Public declarations (2):** `new@L35`, `store@L39`
- **Internal/trait/helpers (22):** `load_plan_source@L45`, `load_owner_outcomes@L87`, `append_read_audit@L159`, `load_case@L218`, `load_snapshot@L232`, `load_plan@L248`, `select_record@L259`, `load_replay_link@L285`, `decode_snapshot_row@L329`, `decode_plan_row@L353`, `decode_record_snapshot@L376`, `validate_contract@L423`, `bind_context@L445`, `read_transaction_id@L471`, `read_audit_digest@L476`, `append_field@L525`, `positive_u64@L530`, `digest_column@L535`, `hex@L542`, `database_error@L552`, `evidence_invalid@L562`, `read_transaction_and_audit_identity_are_deterministic@L577`
- **Public types:** `PostgresPrivacyReadPersistence@L30`
- **Detected tests (1):** `read_transaction_and_audit_identity_are_deterministic@L577`
- **Error/guard literals:** `CUSTOMER_PRIVACY_READ_STORE_UNAVAILABLE`, `CUSTOMER_PRIVACY_READ_EVIDENCE_INVALID`
- **Rust crate references:** `crm_core_data`, `crm_customer_privacy`, `crm_customer_privacy_application`, `crm_customer_privacy_persistence_adapter`, `crm_module_sdk`

### [`src/ready.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-postgres/src/ready.rs) · 332 lines

- **Internal/trait/helpers (13):** `from@L114`, `load_ready@L120`, `prepare_next@L172`, `record_outcome@L179`, `advance_checkpoint@L188`, `ready_invocation@L196`, `parse@L250`, `parse@L256`, `identifier@L271`, `ready_work_invalid@L279`, `ready_evidence_invalid@L289`, `database_error@L299`, `ready_work_sql_is_tenant_bound_bounded_and_lineage_joined@L314`
- **Detected tests (1):** `ready_work_sql_is_tenant_bound_bounded_and_lineage_joined@L314`
- **Error/guard literals:** `CUSTOMER_PRIVACY_OWNER_EXECUTION_READY_WORK_INVALID`, `CUSTOMER_PRIVACY_OWNER_EXECUTION_READY_EVIDENCE_INVALID`, `CUSTOMER_PRIVACY_OWNER_EXECUTION_STORE_UNAVAILABLE`
- **Rust crate references:** `crm_core_data`, `crm_customer_privacy`, `crm_customer_privacy_application`, `crm_module_sdk`

### [`src/restrictions.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-postgres/src/restrictions.rs) · 287 lines

- **Internal/trait/helpers (12):** `lock_and_enforce@L50`, `lock_customer_subject@L103`, `scope_blocks@L117`, `decode_restriction_snapshot@L134`, `restriction_active@L195`, `subject_lock_unavailable@L204`, `restriction_store_unavailable@L214`, `strict_rehydration_failed@L224`, `restriction_row_invalid@L231`, `restriction_state_invalid@L235`, `configuration_error@L245`, `restriction_matrix_is_deny_only@L260`
- **Public types:** `PostgresCustomerPrivacySubjectPolicy@L47`
- **Detected tests (1):** `restriction_matrix_is_deny_only@L260`
- **Error/guard literals:** `CUSTOMER_PRIVACY_RESTRICTION_ACTIVE`, `CUSTOMER_PRIVACY_SUBJECT_LOCK_UNAVAILABLE`, `CUSTOMER_PRIVACY_RESTRICTION_DECISION_UNAVAILABLE`, `CUSTOMER_PRIVACY_RESTRICTION_STATE_INVALID`, `CUSTOMER_PRIVACY_RESTRICTION_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_core_data`, `crm_customer_privacy`, `crm_customer_privacy_persistence_adapter`, `crm_module_sdk`

### [`src/retention.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-postgres/src/retention.rs) · 957 lines

- **Public declarations (2):** `new@L36`, `store@L40`
- **Internal/trait/helpers (27):** `evaluate_and_persist@L46`, `bind_invocation@L151`, `lock_customer_subject@L177`, `load_case@L191`, `load_plan@L239`, `load_legal_holds@L307`, `require_registered_initiating_capability@L370`, `insert_transaction_root@L407`, `delete_unused_transaction_root@L436`, `insert_transaction_evidence@L456`, `retention_request_hash@L596`, `retention_audit_hash@L621`, `append_hash_field@L650`, `insert_decision@L658`, `load_decision@L695`, `decode_snapshot@L743`, `validate_case@L807`, `validate_plan@L826`, `transaction_id@L846`, `data_class_label@L860`, `hex@L874`, `database_error@L884`, `case_not_found@L894`, `retention_conflict@L903`, `retention_evidence_invalid@L913`, `domain_error@L923`, `transaction_identity_is_deterministic_and_coordinate_bound@L932`
- **Public types:** `PostgresRetentionEvaluationPersistence@L31`
- **Detected tests (1):** `transaction_identity_is_deterministic_and_coordinate_bound@L932`
- **Error/guard literals:** `CUSTOMER_PRIVACY_RETENTION_STORE_UNAVAILABLE`, `CUSTOMER_PRIVACY_CASE_NOT_FOUND`, `CUSTOMER_PRIVACY_RETENTION_CONFLICT`, `CUSTOMER_PRIVACY_RETENTION_EVIDENCE_INVALID`
- **Rust crate references:** `crm_core_data`, `crm_customer_privacy`, `crm_customer_privacy_application`, `crm_customer_privacy_persistence_adapter`, `crm_module_sdk`

### [`tests/hold_retention_contract.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-postgres/tests/hold_retention_contract.rs) · 10 lines

- **Internal/trait/helpers (2):** `postgres_retention_evaluation_is_a_shareable_internal_port@L5`, `require_port@L6`
- **Detected tests (2):** `postgres_retention_evaluation_is_a_shareable_internal_port@L5`, `require_port@L6`
- **Rust crate references:** `crm_customer_privacy_application`, `crm_customer_privacy_postgres`

### [`tests/restriction_policy_contract.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-postgres/tests/restriction_policy_contract.rs) · 10 lines

- **Internal/trait/helpers (2):** `postgres_restriction_policy_is_a_shareable_final_decision_port@L5`, `require_policy@L6`
- **Detected tests (2):** `postgres_restriction_policy_is_a_shareable_final_decision_port@L5`, `require_policy@L6`
- **Rust crate references:** `crm_core_data`, `crm_customer_privacy_postgres`

## 55. `crm-customer-privacy-production` — END / B-END + D? adapter

### [`src/access_export.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-production/src/access_export.rs) · 28 lines

- **Public declarations (1):** `build_internal_access_export@L16`
- **Re-exports:** `pub use crm_customer_privacy::encode_access_export_manifest;`, `pub use crm_customer_privacy_application::{`, `pub use crm_customer_privacy_postgres::PostgresAccessExportPersistence;`
- **Rust crate references:** `crm_customer_privacy`, `crm_customer_privacy_application`, `crm_customer_privacy_postgres`

### [`src/legal_hold.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-production/src/legal_hold.rs) · 245 lines

- **Public declarations (3):** `build_production_with_holds@L58`, `build_contribution_with_holds@L79`, `build_contribution_with_complete_control_lifecycle@L113`
- **Internal/trait/helpers (6):** `build_internal_retention@L68`, `add_release_mutation@L181`, `cursor@L196`, `composition_error@L200`, `exact_step_six_inventory_is_seven@L215`, `complete_control_lifecycle_inventory_is_nine_mutations_and_seven_queries@L230`
- **Public types:** `CustomerPrivacyHoldRetentionProduction@L53`
- **Re-exports:** `pub use crm_customer_privacy::{`, `pub use crm_customer_privacy_query_adapter::control_query_capability_definitions;`, `pub use {`
- **Detected tests (2):** `exact_step_six_inventory_is_seven@L215`, `complete_control_lifecycle_inventory_is_nine_mutations_and_seven_queries@L230`
- **Error/guard literals:** `CUSTOMER_PRIVACY_HOLD_RETENTION_PRODUCTION_COMPOSITION_INVALID`
- **Rust crate references:** `crm_application_composition`, `crm_capability_runtime`, `crm_customer_privacy`, `crm_customer_privacy_application`, `crm_customer_privacy_postgres`, `crm_customer_privacy_query_adapter`, `crm_module_sdk`, `crm_query_runtime`

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-production/src/lib.rs) · 384 lines

- **Public declarations (4):** `build_production@L102`, `build_internal_discovery@L116`, `build_internal_planning@L192`, `build_contribution@L203`
- **Internal/trait/helpers (7):** `authorize@L284`, `make@L336`, `make@L342`, `id@L357`, `cursor@L361`, `composition_error@L365`, `configuration_error@L375`
- **Public types:** `CustomerPrivacyProductionDependencies@L88`, `CustomerPrivacyProduction@L95`
- **Re-exports:** `pub use crm_customer_privacy::{`, `pub use crm_customer_privacy_application::{`, `pub use crm_customer_privacy_postgres::{`, `pub use crm_customer_privacy_query_adapter::{`
- **Error/guard literals:** `CUSTOMER_PRIVACY_PRODUCTION_COMPOSITION_INVALID`, `CUSTOMER_PRIVACY_PRODUCTION_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_application_composition`, `crm_capability_runtime`, `crm_consents_privacy_scope_adapter`, `crm_contact_points_privacy_scope_adapter`, `crm_core_data`, `crm_customer_accounts_privacy_scope_adapter`, `crm_customer_data_operations_privacy_scope_adapter`, `crm_customer_enrichment_privacy_scope_adapter`, `crm_customer_privacy`, `crm_customer_privacy_application`, `crm_customer_privacy_postgres`, `crm_customer_privacy_query_adapter`, `crm_data_quality_privacy_scope_adapter`, `crm_identity_resolution_privacy_scope_adapter`, `crm_module_sdk`, `crm_parties_privacy_scope_adapter`, `crm_party_relationships_privacy_scope_adapter`, `crm_query_runtime`

### [`src/owner_execution.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-production/src/owner_execution.rs) · 519 lines

- **Public declarations (2):** `build_internal_owner_execution@L76`, `build_canonical_internal_owner_execution@L95`
- **Internal/trait/helpers (19):** `postgres_owner_endpoint@L149`, `fmt@L174`, `apply@L184`, `load_durable_attempt@L208`, `attempt_from_row@L260`, `validate_endpoint_coordinate@L295`, `validate_request_attempt_binding@L310`, `capability_request@L335`, `owner_failure_result@L384`, `canonical_failure_code@L395`, `derived_request_id@L408`, `derived_causation_id@L412`, `derived_business_transaction_id@L417`, `durable_attempt_not_found@L424`, `durable_attempt_invalid@L433`, `database_error@L443`, `configuration_error@L453`, `exact_owner_action_definitions_remain_frozen@L468`, `failure_codes_are_safe_and_bounded@L507`
- **Re-exports:** `pub use crm_customer_privacy::{`, `pub use crm_customer_privacy_application::{`, `pub use crm_customer_privacy_postgres::{`
- **Detected tests (2):** `exact_owner_action_definitions_remain_frozen@L468`, `failure_codes_are_safe_and_bounded@L507`
- **Error/guard literals:** `PRIVACY_OWNER_ACTION_FAILED`, `CUSTOMER_PRIVACY_OWNER_ATTEMPT_NOT_FOUND`, `CUSTOMER_PRIVACY_OWNER_ATTEMPT_INVALID`, `CUSTOMER_PRIVACY_OWNER_ACTION_DATABASE_ERROR`, `CUSTOMER_PRIVACY_OWNER_ACTION_CONFIGURATION_INVALID`, `PRIVACY_OWNER_ACTION_UNSUPPORTED`
- **Rust crate references:** `crm_capability_runtime`, `crm_consents_privacy_scope_adapter`, `crm_contact_points_privacy_scope_adapter`, `crm_core_data`, `crm_customer_accounts_privacy_scope_adapter`, `crm_customer_data_operations_privacy_scope_adapter`, `crm_customer_enrichment_privacy_scope_adapter`, `crm_customer_privacy`, `crm_customer_privacy_application`, `crm_customer_privacy_postgres`, `crm_data_quality_privacy_scope_adapter`, `crm_identity_resolution_privacy_scope_adapter`, `crm_module_sdk`, `crm_parties_privacy_scope_adapter`, `crm_party_relationships_privacy_scope_adapter`

### [`src/restriction.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-production/src/restriction.rs) · 89 lines

- **Public declarations (2):** `build_production_with_restrictions@L17`, `build_contribution_with_restrictions@L31`
- **Internal/trait/helpers (2):** `composition_error@L65`, `exact_step_four_inventory_is_six@L80`
- **Re-exports:** `pub use crm_customer_privacy_application::mutation_capability_definitions_with_restrictions;`
- **Detected tests (1):** `exact_step_four_inventory_is_six@L80`
- **Error/guard literals:** `CUSTOMER_PRIVACY_RESTRICTION_PRODUCTION_COMPOSITION_INVALID`
- **Rust crate references:** `crm_application_composition`, `crm_capability_runtime`, `crm_customer_privacy_application`, `crm_customer_privacy_postgres`, `crm_module_sdk`

### [`src/root.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-production/src/root.rs) · 16 lines

- **Re-exports:** `pub use access_export::*;`, `pub use legacy::*;`, `pub use legal_hold::*;`, `pub use owner_execution::*;`, `pub use {crm_customer_privacy_query_adapter::control_query_visibility_resources, restriction::*};`
- **Rust crate references:** `crm_customer_privacy_query_adapter`

### [`src/worker.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-production/src/worker.rs) · 183 lines

- **Internal/trait/helpers (8):** `fmt@L21`, `from@L33`, `try_from@L46`, `run_tenant_cycle@L58`, `authorize_visibility@L81`, `validate_work_batch@L95`, `work_batch_invalid@L139`, `production_batch_rejects_duplicate_identity_before_execution@L157`
- **Detected tests (1):** `production_batch_rejects_duplicate_identity_before_execution@L157`
- **Error/guard literals:** `CUSTOMER_PRIVACY_OWNER_EXECUTION_WORK_BATCH_INVALID`
- **Rust crate references:** `crm_application_composition`, `crm_core_data`, `crm_module_sdk`, `crm_query_runtime`

## 56. `crm-customer-privacy-query-adapter` — END / B-END + D? adapter

### [`src/controls.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-query-adapter/src/controls.rs) · 1051 lines

- **Public declarations (3):** `control_query_visibility_resources@L84`, `new_with_cursor@L122`, `control_query_capability_definitions@L277`
- **Internal/trait/helpers (45):** `cursor_codec@L134`, `get_restriction@L140`, `get_legal_hold@L180`, `fmt@L218`, `validate@L229`, `execute@L257`, `query_definition@L297`, `list_legal_holds@L334`, `collect_holds@L357`, `has_more@L416`, `list_parameters@L458`, `cursor_binding@L480`, `decode_after@L504`, `encode_next@L524`, `list_response@L545`, `require_subject_visible@L560`, `rehydrate_restriction@L580`, `rehydrate_hold@L596`, `status_matches@L612`, `processing_restriction_to_wire@L622`, `customer_data_legal_hold_to_wire@L666`, `legal_hold_scope_to_wire@L700`, `data_class_to_wire@L715`, `redact_restriction@L729`, `redact_legal_hold@L765`, `decode_input@L804`, `processing_restriction_ref@L835`, `legal_hold_ref@L851`, `party_id@L865`, `legal_hold_status_filter@L880`, `page_size@L897`, `ensure_definition@L915`, `module_id@L926`, `record_type@L930`, `configured@L934`, `nanos_to_millis@L938`, `enforce_scan_limit@L945`, `control_not_found@L958`, `control_state_invalid@L967`, `unsupported_query@L977`, `configuration_invalid@L986`, `cursor_invalid@L996`, `cursor_error@L1005`, `control_query_catalog_and_visibility_are_exact@L1014`, `legal_hold_list_bounds_and_filter_are_strict@L1033`
- **Public types:** `CustomerPrivacyControlQueryAdapter@L115`
- **Detected tests (2):** `control_query_catalog_and_visibility_are_exact@L1014`, `legal_hold_list_bounds_and_filter_are_strict@L1033`
- **Error/guard literals:** `CUSTOMER_PRIVACY_CONTROL_QUERY_CONTRACT_MISMATCH`, `CUSTOMER_PRIVACY_CONTROL_QUERY_PROTOBUF_INVALID`, `CUSTOMER_PRIVACY_LEGAL_HOLD_LIST_SCAN_LIMIT_EXCEEDED`, `CUSTOMER_PRIVACY_CONTROL_NOT_FOUND`, `CUSTOMER_PRIVACY_CONTROL_STATE_INVALID`, `CUSTOMER_PRIVACY_CONTROL_QUERY_UNSUPPORTED`, `CUSTOMER_PRIVACY_CONTROL_QUERY_CONFIGURATION_INVALID`, `CUSTOMER_PRIVACY_LEGAL_HOLD_LIST_CURSOR_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_privacy`, `crm_customer_privacy_persistence_adapter`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-query-adapter/src/lib.rs) · 673 lines

- **Public declarations (16):** `query_visibility_resources@L66`, `new@L96`, `new_with_cursor@L104`, `cursor_codec@L116`, `query_capability_definition@L231`, `get_privacy_case_capability_definition@L235`, `list_privacy_cases_capability_definition@L243`, `query_capability_definitions@L279`, `privacy_case_to_wire@L286`, `redact_privacy_case@L382`, `decode_input@L427`, `module_id@L489`, `privacy_case_record_type@L493`, `configured@L497`, `case_state_invalid@L512`, `query_configuration_invalid@L531`
- **Internal/trait/helpers (18):** `execute_get_case@L122`, `fmt@L182`, `validate@L193`, `execute@L214`, `query_definition@L251`, `subject_binding_to_wire@L343`, `rescope_to_wire@L363`, `privacy_case_ref@L458`, `ensure_definition@L478`, `case_not_found@L503`, `unsupported_query@L522`, `nanos_to_millis@L541`, `kind_to_wire@L548`, `verification_method_to_wire@L557`, `status_to_wire@L574`, `resume_stage_to_wire@L601`, `query_catalog_and_visibility_are_exact@L615`, `redaction_preserves_identity_and_removes_only_hidden_fields@L645`
- **Public types:** `CustomerPrivacyVisibilityResource@L60`, `CustomerPrivacyQueryAdapter@L89`
- **Re-exports:** `pub use controls::*;`
- **Detected tests (2):** `query_catalog_and_visibility_are_exact@L615`, `redaction_preserves_identity_and_removes_only_hidden_fields@L645`
- **Error/guard literals:** `CUSTOMER_PRIVACY_QUERY_CONTRACT_MISMATCH`, `CUSTOMER_PRIVACY_QUERY_PROTOBUF_INVALID`, `CUSTOMER_PRIVACY_CASE_NOT_FOUND`, `CUSTOMER_PRIVACY_CASE_STATE_INVALID`, `CUSTOMER_PRIVACY_QUERY_UNSUPPORTED`, `CUSTOMER_PRIVACY_QUERY_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_privacy`, `crm_customer_privacy_persistence_adapter`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`

### [`src/list.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-query-adapter/src/list.rs) · 398 lines

- **Public declarations (2):** `validate@L30`, `execute@L37`
- **Internal/trait/helpers (17):** `collect@L60`, `has_more@L115`, `rehydrate_case@L156`, `matches_filters@L172`, `parameters@L189`, `party_id@L211`, `kind@L226`, `status@L238`, `page_size@L250`, `binding@L268`, `decode_after@L297`, `encode_next@L317`, `response@L338`, `enforce_scan_limit@L353`, `cursor_invalid@L366`, `cursor_error@L375`, `list_bounds_and_filters_are_strict@L384`
- **Detected tests (1):** `list_bounds_and_filters_are_strict@L384`
- **Error/guard literals:** `CUSTOMER_PRIVACY_CASE_LIST_SCAN_LIMIT_EXCEEDED`, `CUSTOMER_PRIVACY_CASE_LIST_CURSOR_INVALID`
- **Rust crate references:** `crm_core_data`, `crm_customer_privacy`, `crm_customer_privacy_persistence_adapter`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`

## 57. `crm-customer-privacy-subject-capability-adapter` — END / B-END + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-subject-capability-adapter/src/lib.rs) · 480 lines

- **Public declarations (2):** `capability_definition@L116`, `capability_definitions@L142`
- **Internal/trait/helpers (19):** `target@L46`, `plan@L63`, `verify_command@L146`, `build_plan@L166`, `case_to_wire@L254`, `ensure_exact_coordinate@L306`, `positive_version@L325`, `case_ref@L340`, `party_id@L354`, `verification_method@L364`, `method_to_wire@L385`, `kind_to_wire@L402`, `nanos_to_millis@L411`, `domain_error@L421`, `case_not_found@L436`, `case_state_invalid@L445`, `required@L455`, `invalid_plan@L459`, `configured@L469`
- **Public types:** `CustomerPrivacyCaseSubjectVerifyCapabilityPlanner@L43`
- **Error/guard literals:** `CUSTOMER_PRIVACY_CASE_NOT_FOUND`, `CUSTOMER_PRIVACY_CASE_INVALID`, `CUSTOMER_PRIVACY_CASE_SUBJECT_VERIFY_PLAN_INVALID`, `CUSTOMER_PRIVACY_CASE_SUBJECT_VERIFY_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_privacy`, `crm_customer_privacy_persistence_adapter`, `crm_module_sdk`, `crm_proto_contracts`

### [`tests/planner.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-subject-capability-adapter/tests/planner.rs) · 425 lines

- **Internal/trait/helpers (8):** `base_spec@L41`, `request@L55`, `snapshot@L109`, `successful_transition_binds_subject_and_emits_exact_evidence@L146`, `planning_is_stable_for_exact_replay_input@L246`, `missing_references_and_invalid_scalars_fail_closed@L266`, `missing_cross_tenant_malformed_and_wrong_lifecycle_snapshots_are_bounded@L335`, `stale_version_and_non_monotonic_time_are_rejected@L398`
- **Detected tests (5):** `successful_transition_binds_subject_and_emits_exact_evidence@L146`, `planning_is_stable_for_exact_replay_input@L246`, `missing_references_and_invalid_scalars_fail_closed@L266`, `missing_cross_tenant_malformed_and_wrong_lifecycle_snapshots_are_bounded@L335`, `stale_version_and_non_monotonic_time_are_rejected@L398`
- **Error/guard literals:** `SDK_INVALID_ARGUMENT`, `CUSTOMER_PRIVACY_CASE_NOT_FOUND`, `CUSTOMER_PRIVACY_CASE_INVALID`, `CUSTOMER_PRIVACY_INVALID_TRANSITION`, `CUSTOMER_PRIVACY_VERSION_CONFLICT`, `CUSTOMER_PRIVACY_INVALID_ARGUMENT`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_privacy`, `crm_customer_privacy_persistence_adapter`, `crm_customer_privacy_subject_capability_adapter`, `crm_module_sdk`, `crm_proto_contracts`

## 58. `crm-customer-privacy-submit-capability-adapter` — END / B-END + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-customer-privacy-submit-capability-adapter/src/lib.rs) · 629 lines

- **Public declarations (2):** `capability_definition@L99`, `capability_definitions@L123`
- **Internal/trait/helpers (21):** `target@L44`, `plan@L61`, `submit_command@L127`, `build_plan@L138`, `case_to_wire@L221`, `ensure_exact_coordinate@L256`, `positive_version@L274`, `case_ref@L289`, `kind_to_wire@L303`, `nanos_to_millis@L312`, `domain_error@L322`, `case_not_found@L337`, `case_state_invalid@L346`, `required@L356`, `invalid_plan@L360`, `configured@L370`, `request@L392`, `snapshot@L438`, `target_and_plan_are_exact_versioned_update@L459`, `missing_tenant_stale_and_wrong_state_fail_closed@L517`, `malformed_state_and_invalid_request_are_bounded@L584`
- **Public types:** `CustomerPrivacyCaseSubmitCapabilityPlanner@L41`
- **Detected tests (3):** `target_and_plan_are_exact_versioned_update@L459`, `missing_tenant_stale_and_wrong_state_fail_closed@L517`, `malformed_state_and_invalid_request_are_bounded@L584`
- **Error/guard literals:** `CUSTOMER_PRIVACY_CASE_NOT_FOUND`, `CUSTOMER_PRIVACY_CASE_INVALID`, `CUSTOMER_PRIVACY_CASE_SUBMIT_PLAN_INVALID`, `CUSTOMER_PRIVACY_CASE_SUBMIT_CONFIGURATION_INVALID`, `CUSTOMER_PRIVACY_VERSION_CONFLICT`, `CUSTOMER_PRIVACY_INVALID_TRANSITION`, `SDK_INVALID_ARGUMENT`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_privacy`, `crm_customer_privacy_persistence_adapter`, `crm_module_sdk`, `crm_proto_contracts`

## 59. `crm-data-quality-capability-adapter` — NEXT/LATER / B-CONDITIONAL + D? adapter

### [`src/completeness_profile_planner.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-capability-adapter/src/completeness_profile_planner.rs) · 403 lines

- **Public declarations (8):** `new@L38`, `completeness_profile_reference_scope_from_request@L123`, `party_completeness_profile_from_definition@L154`, `party_completeness_profile_to_wire@L190`, `party_completeness_profile_persisted_contract@L215`, `party_completeness_profile_persisted_payload@L226`, `party_completeness_profile_from_snapshot@L236`, `completeness_profile_rule_set_version_id_from_snapshot@L255`
- **Internal/trait/helpers (11):** `target@L44`, `plan@L59`, `completeness_profile_record_ref@L269`, `ensure_completeness_semantic_version@L279`, `decode_request@L291`, `ensure_definition@L300`, `reference_unavailable@L313`, `invalid_plan@L322`, `rule_set@L338`, `definition_binds_exact_rule_set_and_canonicalizes_components@L355`, `definition_rejects_different_or_unknown_rule_set_binding@L381`
- **Public types:** `CompletenessProfileReferenceScope@L28`, `DataQualityCompletenessProfileCapabilityPlanner@L33`
- **Detected tests (2):** `definition_binds_exact_rule_set_and_canonicalizes_components@L355`, `definition_rejects_different_or_unknown_rule_set_binding@L381`
- **Error/guard literals:** `DATA_QUALITY_COMPLETENESS_RULE_SET_UNAVAILABLE`, `DATA_QUALITY_COMPLETENESS_PROFILE_CAPABILITY_PLAN_INVALID`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_data_quality`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/evaluation_job_planner.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-capability-adapter/src/evaluation_job_planner.rs) · 316 lines

- **Public declarations (6):** `new@L39`, `evaluation_reference_scope_from_request@L145`, `party_evaluation_job_to_wire@L186`, `party_evaluation_job_persisted_contract@L235`, `party_evaluation_job_persisted_payload@L246`, `party_evaluation_job_from_snapshot@L256`
- **Internal/trait/helpers (8):** `job_from_request@L49`, `target@L72`, `plan@L85`, `job_record_ref@L276`, `record_id@L284`, `missing@L288`, `ensure_definition@L292`, `invalid_plan@L307`
- **Public types:** `EvaluationReferenceScope@L25`, `DataQualityEvaluationJobCapabilityPlanner@L33`
- **Error/guard literals:** `DATA_QUALITY_EVALUATION_CAPABILITY_PLAN_INVALID`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_data_quality`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/evaluation_materialization_planner.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-capability-adapter/src/evaluation_materialization_planner.rs) · 516 lines

- **Public declarations (13):** `new@L57`, `party_rule_outcome_persisted_contract@L332`, `party_rule_outcome_persisted_payload@L343`, `party_rule_outcome_record_ref@L353`, `party_completeness_result_persisted_contract@L363`, `party_completeness_result_persisted_payload@L374`, `party_completeness_result_record_ref@L384`, `party_finding_persisted_contract@L394`, `party_finding_persisted_payload@L405`, `party_finding_record_ref@L415`, `party_finding_observation_persisted_contract@L425`, `party_finding_observation_persisted_payload@L436`, `party_finding_observation_record_ref@L446`
- **Internal/trait/helpers (7):** `target@L89`, `plan@L102`, `decode_request@L456`, `job_record_ref@L467`, `missing@L488`, `ensure_definition@L492`, `invalid_plan@L507`
- **Public types:** `ExistingPartyFinding@L37`, `ExistingPartyFindingObservation@L43`, `DataQualityEvaluationMaterializationPlanner@L48`
- **Error/guard literals:** `DATA_QUALITY_EVALUATION_MATERIALIZATION_VERSION_CONFLICT`, `DATA_QUALITY_EVALUATION_MATERIALIZATION_PLAN_INVALID`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_data_quality`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/evaluation_stage_planner.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-capability-adapter/src/evaluation_stage_planner.rs) · 237 lines

- **Public declarations (3):** `party_evaluation_input_persisted_contract@L147`, `party_evaluation_input_persisted_payload@L158`, `party_evaluation_input_record_ref@L168`
- **Internal/trait/helpers (8):** `target@L27`, `plan@L40`, `decode_request@L178`, `job_record_ref@L189`, `record_id@L205`, `missing@L209`, `ensure_definition@L213`, `invalid_plan@L228`
- **Public types:** `DataQualityEvaluationStagePlanner@L24`
- **Error/guard literals:** `DATA_QUALITY_EVALUATION_STAGE_VERSION_CONFLICT`, `DATA_QUALITY_EVALUATION_STAGE_PLAN_INVALID`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_data_quality`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/finding_stewardship_planner.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-capability-adapter/src/finding_stewardship_planner.rs) · 351 lines

- **Public declarations (1):** `party_finding_from_snapshot@L200`
- **Internal/trait/helpers (14):** `target@L24`, `plan@L36`, `apply_command@L162`, `finding_record_ref@L215`, `expected_version@L244`, `decode@L268`, `required_finding_id@L275`, `required_observation_id@L281`, `ensure_definition@L298`, `missing@L316`, `finding_unavailable@L320`, `version_conflict@L329`, `unsupported@L338`, `invalid_plan@L342`
- **Public types:** `DataQualityFindingStewardshipPlanner@L21`
- **Error/guard literals:** `DATA_QUALITY_FINDING_NOT_FOUND`, `DATA_QUALITY_FINDING_VERSION_CONFLICT`, `DATA_QUALITY_FINDING_STEWARDSHIP_PLAN_INVALID`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_data_quality`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/finding_wire.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-capability-adapter/src/finding_wire.rs) · 72 lines

- **Public declarations (1):** `party_finding_to_wire@L6`
- **Internal/trait/helpers (2):** `status_to_wire@L55`, `severity_to_wire@L64`
- **Rust crate references:** `crm_data_quality`, `crm_proto_contracts`

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-capability-adapter/src/lib.rs) · 370 lines

- **Public declarations (12):** `party_rule_set_from_snapshot@L157`, `party_completeness_profile_from_immutable_snapshot@L164`, `capability_definitions@L172`, `capability_definition@L184`, `rule_set_capability_definition@L188`, `completeness_profile_capability_definition@L197`, `evaluation_request_capability_definition@L206`, `finding_assign_capability_definition@L215`, `finding_acknowledge_capability_definition@L224`, `finding_waive_capability_definition@L233`, `evaluation_stage_capability_definition@L242`, `evaluation_materialization_capability_definition@L251`
- **Internal/trait/helpers (6):** `mutation_definition@L261`, `ensure_immutable_version@L286`, `configured@L302`, `configuration_error@L306`, `mutation_catalog_has_exact_coordinates_and_data_classification@L320`, `internal_evaluation_definitions_are_personal_and_not_publicly_catalogued@L357`
- **Re-exports:** `pub use completeness_profile_planner::{`, `pub use evaluation_job_planner::{`, `pub use evaluation_materialization_planner::{`, `pub use evaluation_stage_planner::{`, `pub use finding_stewardship_planner::{`, `pub use finding_wire::party_finding_to_wire;`, `pub use remediation_planner::{`, `pub use remediation_registration::*;`, `pub use rule_set_planner::{`
- **Detected tests (2):** `mutation_catalog_has_exact_coordinates_and_data_classification@L320`, `internal_evaluation_definitions_are_personal_and_not_publicly_catalogued@L357`
- **Error/guard literals:** `DATA_QUALITY_PERSISTED_STATE_INVALID`, `DATA_QUALITY_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_data_quality`, `crm_module_sdk`

### [`src/remediation_planner.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-capability-adapter/src/remediation_planner.rs) · 262 lines

- **Public declarations (5):** `new@L30`, `remediation_attempt_persisted_contract@L139`, `remediation_attempt_persisted_payload@L150`, `remediation_attempt_record_ref@L160`, `remediation_attempt_to_wire@L170`
- **Internal/trait/helpers (5):** `target@L59`, `plan@L71`, `verify_command@L214`, `ensure_definition@L237`, `invalid_plan@L253`
- **Public types:** `DataQualityRemediationCompletionPlanner@L24`
- **Error/guard literals:** `DATA_QUALITY_REMEDIATION_COMPLETION_PLAN_INVALID`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_data_quality`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/remediation_registration.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-capability-adapter/src/remediation_registration.rs) · 49 lines

- **Public declarations (1):** `remediation_capability_definition@L22`
- **Re-exports:** `pub use crm_data_quality::{`
- **Rust crate references:** `crm_capability_runtime`, `crm_data_quality`, `crm_module_sdk`

### [`src/rule_set_planner.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-capability-adapter/src/rule_set_planner.rs) · 397 lines

- **Public declarations (5):** `party_rule_set_from_definition@L104`, `party_rule_set_to_wire@L134`, `party_rule_set_persisted_contract@L150`, `party_rule_set_persisted_payload@L161`, `party_rule_set_from_snapshot@L171`
- **Internal/trait/helpers (14):** `target@L27`, `plan@L41`, `party_quality_rule_from_wire@L189`, `party_quality_rule_to_wire@L230`, `rule_set_record_ref@L261`, `decode_request@L271`, `ensure_definition@L280`, `invalid_plan@L293`, `definition@L306`, `minimum_rule@L313`, `placeholder_rule@L330`, `wire_definition_is_canonicalized_before_publication_identity@L350`, `wire_definition_rejects_unspecified_semantics_and_evaluator@L370`, `persisted_payload_round_trip_revalidates_content_addressed_identity@L382`
- **Public types:** `DataQualityRuleSetCapabilityPlanner@L24`
- **Detected tests (3):** `wire_definition_is_canonicalized_before_publication_identity@L350`, `wire_definition_rejects_unspecified_semantics_and_evaluator@L370`, `persisted_payload_round_trip_revalidates_content_addressed_identity@L382`
- **Error/guard literals:** `DATA_QUALITY_RULE_SET_CAPABILITY_PLAN_INVALID`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_data_quality`, `crm_module_sdk`, `crm_proto_contracts`

### [`tests/public_surface.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-capability-adapter/tests/public_surface.rs) · 37 lines

- **Internal/trait/helpers (1):** `rule_set_publication_remains_exact_confidential_idempotent_surface@L8`
- **Detected tests (1):** `rule_set_publication_remains_exact_confidential_idempotent_surface@L8`
- **Rust crate references:** `crm_capability_runtime`, `crm_data_quality_capability_adapter`, `crm_module_sdk`

## 60. `crm-data-quality-privacy-scope-adapter` — END / B-END + D? adapter

### [`src/contract.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-privacy-scope-adapter/src/contract.rs) · 99 lines

- **Public declarations (8):** `data_quality_privacy_scope_definition@L38`, `validate_definition@L54`, `module_id@L64`, `capability_id@L68`, `capability_version@L72`, `schema_id@L76`, `schema_version@L80`, `output_descriptor_hash@L84`
- **Internal/trait/helpers (1):** `payload_contract@L88`
- **Error/guard literals:** `DATA_QUALITY_PRIVACY_SCOPE_DEFINITION_MISMATCH`
- **Rust crate references:** `crm_capability_runtime`, `crm_data_quality_capability_adapter`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/digest.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-privacy-scope-adapter/src/digest.rs) · 44 lines

- **Public declarations (2):** `encode_hex@L3`, `decode_hex@L13`
- **Internal/trait/helpers (2):** `nibble@L30`, `cursor_invalid@L38`
- **Error/guard literals:** `DATA_QUALITY_PRIVACY_SCOPE_CURSOR_INVALID`
- **Rust crate references:** `crm_module_sdk`

### [`src/errors.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-privacy-scope-adapter/src/errors.rs) · 183 lines

- **Public declarations (12):** `configured@L6`, `map_request_contract_error@L18`, `map_common_lineage_error@L37`, `map_canonical_party_claim_error@L82`, `invalid_contract@L108`, `invalid_contract_with_reference@L112`, `lineage_invalid@L120`, `stored_state_invalid@L134`, `association_state_invalid@L144`, `scan_limit_exceeded@L154`, `canonical_resolution_unavailable@L164`, `database_unavailable@L174`
- **Internal/trait/helpers (1):** `lineage_contract_invalid@L67`
- **Error/guard literals:** `DATA_QUALITY_PRIVACY_SCOPE_CONFIGURATION_INVALID`, `DATA_QUALITY_PRIVACY_SCOPE_REQUEST_BINDING_MISMATCH`, `DATA_QUALITY_PRIVACY_SCOPE_INPUT_CONTRACT_MISMATCH`, `DATA_QUALITY_PRIVACY_SCOPE_INPUT_HASH_MISMATCH`, `TENANT_MISMATCH`, `DATA_QUALITY_PRIVACY_SCOPE_CASE_ID_INVALID`, `PARTY_INVALID`, `DATA_QUALITY_PRIVACY_SCOPE_PARTY_INVALID`, `GENERATION_INVALID`, `REGISTRY_INVALID`, `DATA_QUALITY_PRIVACY_SCOPE_REGISTRY_UNAVAILABLE`, `REGISTRY_MISMATCH`, `PURPOSE_INVALID`, `REQUEST_TIME_INVALID`, `PAGE_SIZE_INVALID`, `DATA_QUALITY_PRIVACY_SCOPE_TENANT_MISMATCH`, `DATA_QUALITY_PRIVACY_SCOPE_GENERATION_INVALID`, `DATA_QUALITY_PRIVACY_SCOPE_REGISTRY_INVALID`, `DATA_QUALITY_PRIVACY_SCOPE_REGISTRY_MISMATCH`, `DATA_QUALITY_PRIVACY_SCOPE_PURPOSE_INVALID`, `DATA_QUALITY_PRIVACY_SCOPE_REQUEST_TIME_INVALID`, `DATA_QUALITY_PRIVACY_SCOPE_PAGE_SIZE_INVALID`, `DATA_QUALITY_PRIVACY_SCOPE_LINEAGE_INVALID`, `DATA_QUALITY_PRIVACY_SCOPE_STORED_STATE_INVALID`, `DATA_QUALITY_PRIVACY_SCOPE_ASSOCIATION_STATE_INVALID`, `DATA_QUALITY_PRIVACY_SCOPE_SCAN_LIMIT_EXCEEDED`, `DATA_QUALITY_PRIVACY_SCOPE_CANONICAL_RESOLUTION_UNAVAILABLE`, `DATA_QUALITY_PRIVACY_SCOPE_UNAVAILABLE`
- **Rust crate references:** `crm_customer_privacy_owner_scope_support`, `crm_module_sdk`

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-privacy-scope-adapter/src/lib.rs) · 39 lines

- **Re-exports:** `pub use contract::{`, `pub use owner_action::{`, `pub use postgres::DataQualityPrivacyScopeQueryAdapter;`

### [`src/owner_action.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-privacy-scope-adapter/src/owner_action.rs) · 243 lines

- **Public declarations (2):** `data_quality_privacy_action_definition@L33`, `data_quality_privacy_action_planner@L37`
- **Internal/trait/helpers (12):** `owner_module_id@L42`, `capability_id@L46`, `supports_resource_type@L50`, `anonymize@L63`, `deletion_tombstone@L72`, `validate_resource@L82`, `validate_payload@L159`, `validate_identity@L181`, `unsupported@L196`, `stored_state_invalid@L200`, `publishes_the_frozen_owner_action_coordinate@L215`, `supports_exactly_the_seven_discovered_resource_families@L227`
- **Public types:** `DataQualityPrivacyActionPlanner@L27`, `DataQualityPrivacyActionPolicy@L31`
- **Detected tests (2):** `publishes_the_frozen_owner_action_coordinate@L215`, `supports_exactly_the_seven_discovered_resource_families@L227`
- **Error/guard literals:** `DATA_QUALITY_PRIVACY_STORED_STATE_INVALID`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_customer_privacy_owner_scope_support`, `crm_data_quality`, `crm_data_quality_capability_adapter`, `crm_module_sdk`

### [`src/postgres.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-privacy-scope-adapter/src/postgres.rs) · 1249 lines

- **Public declarations (1):** `new@L70`
- **Internal/trait/helpers (25):** `fmt@L61`, `execute_query@L74`, `execute@L111`, `new@L153`, `resolves_to_subject@L160`, `new@L226`, `charge@L230`, `read_data_quality_page@L244`, `append_relevant@L519`, `validate_associations@L546`, `strict_definitions@L883`, `strict_job@L930`, `strict_input@L941`, `strict_outcome@L958`, `strict_finding@L974`, `strict_observation@L990`, `strict_completeness_result@L1009`, `strict_remediation_attempt@L1028`, `versioned@L1047`, `map_owner_error@L1055`, `resource_after_cursor@L1059`, `load_record_rows@L1084`, `decode_stored_row@L1146`, `strict_snapshot@L1187`, `positive_version@L1246`
- **Public types:** `DataQualityPrivacyScopeQueryAdapter@L56`
- **Error/guard literals:** `IDENTITY_RESOLUTION_CANONICAL_PARTY_MISMATCH`, `IDENTITY_RESOLUTION_TOPOLOGY_GENERATION_STALE`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_privacy_owner_scope_support`, `crm_data_quality`, `crm_data_quality_capability_adapter`, `crm_identity_resolution`, `crm_identity_resolution_topology_composition`, `crm_module_sdk`, `crm_query_runtime`

### [`src/request.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-privacy-scope-adapter/src/request.rs) · 257 lines

- **Public declarations (4):** `token@L33`, `validate_request_contract@L75`, `validate_wire_request@L80`, `encode_cursor@L141`
- **Internal/trait/helpers (4):** `parse@L45`, `decode_cursor@L170`, `cursor_digest@L219`, `cursor_invalid@L251`
- **Public types:** `ResourceFamily@L22`, `CursorState@L60`, `ValidatedRequest@L66`
- **Error/guard literals:** `DATA_QUALITY_PRIVACY_SCOPE_REQUEST_INVALID`, `DATA_QUALITY_PRIVACY_SCOPE_LINEAGE_INVALID`, `DATA_QUALITY_PRIVACY_SCOPE_CURSOR_INVALID`
- **Rust crate references:** `crm_customer_privacy_owner_scope_support`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`

### [`src/response.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-privacy-scope-adapter/src/response.rs) · 173 lines

- **Public declarations (2):** `build_response@L60`, `typed_output@L131`
- **Internal/trait/helpers (3):** `resource_type@L33`, `retention_policy_id@L45`, `page_digest@L147`
- **Public types:** `VerifiedDataQualityResource@L26`
- **Rust crate references:** `crm_customer_privacy_owner_scope_support`, `crm_data_quality`, `crm_data_quality_capability_adapter`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/tests.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-privacy-scope-adapter/src/tests.rs) · 70 lines

- **Internal/trait/helpers (3):** `definition_is_exact_contract_only_query_coordinate@L15`, `frozen_bounds_match_the_entry_packet@L27`, `seven_family_order_and_tokens_are_stable@L46`
- **Detected tests (3):** `definition_is_exact_contract_only_query_coordinate@L15`, `frozen_bounds_match_the_entry_packet@L27`, `seven_family_order_and_tokens_are_stable@L46`
- **Rust crate references:** `crm_data_quality_capability_adapter`

### [`tests/postgres_scope.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-privacy-scope-adapter/tests/postgres_scope.rs) · 1068 lines

- **Internal/trait/helpers (20):** `data_quality_scope_is_alias_aware_strict_complete_minimized_and_side_effect_free@L77`, `definitions@L396`, `insert_definitions@L428`, `insert_complete_graph@L458`, `insert_record@L619`, `create_party@L678`, `merge_party@L708`, `capability_request@L757`, `scope_request@L803`, `current_generation@L859`, `write_surface_counts@L888`, `count@L901`, `prove_records_primary_key_scan@L924`, `set_record_data_class@L954`, `delete_record@L970`, `decode@L988`, `expected_two_graphs@L993`, `graph_values@L1027`, `assert_response_omits_private_data_quality_values@L1039`, `contains@L1063`
- **Detected tests (1):** `data_quality_scope_is_alias_aware_strict_complete_minimized_and_side_effect_free@L77`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`, `DATA_QUALITY_PRIVACY_SCOPE_LINEAGE_INVALID`, `DATA_QUALITY_PRIVACY_SCOPE_CURSOR_INVALID`, `DATA_QUALITY_PRIVACY_SCOPE_STORED_STATE_INVALID`, `DATA_QUALITY_PRIVACY_SCOPE_ASSOCIATION_STATE_INVALID`, `PRIVACY_ERASURE_SCOPE`, `DATA_QUALITY_PARTY_DISPLAY_NAME_PLACEHOLDER`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_customer_privacy`, `crm_data_quality`, `crm_data_quality_capability_adapter`, `crm_data_quality_privacy_scope_adapter`, `crm_identity_resolution_capability_adapter`, `crm_module_sdk`, `crm_parties_capability_adapter`, `crm_proto_contracts`, `crm_query_runtime`, `crm_parties`, `crm_identity_resolution`

## 61. `crm-data-quality-query-adapter` — NEXT/LATER / B-CONDITIONAL + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-query-adapter/src/lib.rs) · 22 lines

- **Re-exports:** `pub use party_source_registry::{`

### [`src/party_source_registry.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-query-adapter/src/party_source_registry.rs) · 43 lines

- **Public declarations (2):** `register_party_quality_query_adapter@L7`, `registered_party_quality_query_adapter@L16`
- **Internal/trait/helpers (2):** `registry_unavailable@L26`, `registry_error@L35`
- **Error/guard literals:** `DATA_QUALITY_PARTY_SOURCE_UNAVAILABLE`, `DATA_QUALITY_PARTY_SOURCE_REGISTRY_INVALID`
- **Rust crate references:** `crm_module_sdk`, `crm_parties_query_adapter`

### [`src/query_catalog.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-query-adapter/src/query_catalog.rs) · 88 lines

- **Public declarations (9):** `query_capability_definitions@L31`, `query_capability_definition@L43`, `rule_set_query_capability_definition@L45`, `completeness_profile_query_capability_definition@L49`, `evaluation_job_query_capability_definition@L53`, `finding_query_capability_definition@L57`, `list_findings_by_party_query_capability_definition@L61`, `list_assigned_findings_query_capability_definition@L65`, `completeness_result_query_capability_definition@L69`
- **Internal/trait/helpers (3):** `matches@L10`, `hash@L19`, `query_definition@L73`

### [`src/query_common.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-query-adapter/src/query_common.rs) · 26 lines

- **Internal/trait/helpers (18):** `apply_definition_visibility@L1`, `validate_record_id@L4`, `ensure_definition@L7`, `module_id@L11`, `finding_record_type@L12`, `enforce_scan_limit@L13`, `rule_set_not_found@L14`, `completeness_profile_not_found@L15`, `evaluation_job_not_found@L16`, `finding_not_found@L17`, `completeness_result_not_found@L18`, `persisted_reference_missing@L19`, `persisted_observation_missing@L20`, `unsupported_query@L21`, `configuration_error@L22`, `cursor_invalid@L23`, `cursor_error@L24`, `scan_limit_error@L25`
- **Error/guard literals:** `DATA_QUALITY_PARTY_RULE_SET_NOT_FOUND`, `DATA_QUALITY_PARTY_COMPLETENESS_PROFILE_NOT_FOUND`, `DATA_QUALITY_PARTY_EVALUATION_NOT_FOUND`, `DATA_QUALITY_FINDING_NOT_FOUND`, `DATA_QUALITY_PARTY_COMPLETENESS_RESULT_NOT_FOUND`, `DATA_QUALITY_PERSISTED_STATE_INVALID`, `DATA_QUALITY_QUERY_UNSUPPORTED`, `DATA_QUALITY_QUERY_CONFIGURATION_INVALID`, `DATA_QUALITY_FINDING_QUERY_CURSOR_INVALID`, `DATA_QUALITY_FINDING_QUERY_SCAN_LIMIT_EXCEEDED`

### [`src/query_decode.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-query-adapter/src/query_decode.rs) · 65 lines

- **Internal/trait/helpers (11):** `finding_from_snapshot@L1`, `observation_from_snapshot@L9`, `completeness_result_from_snapshot@L17`, `decode_input@L25`, `required_rule_set_ref@L41`, `required_completeness_profile_ref@L44`, `required_evaluation_job_ref@L47`, `finding_record_id@L50`, `party_record_id@L54`, `completeness_result_record_id@L58`, `assigned_actor_filter@L62`
- **Error/guard literals:** `DATA_QUALITY_PERSISTED_FINDING_IDENTITY_INVALID`, `DATA_QUALITY_PERSISTED_FINDING_OBSERVATION_IDENTITY_INVALID`, `DATA_QUALITY_PERSISTED_COMPLETENESS_RESULT_IDENTITY_INVALID`

### [`src/query_definition_gets.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-query-adapter/src/query_definition_gets.rs) · 188 lines

- **Public declarations (1):** `new@L2`
- **Internal/trait/helpers (6):** `execute_get_party_rule_set@L30`, `execute_get_party_completeness_profile@L66`, `execute_get_party_evaluation_job@L120`, `data_quality_cursor_key@L155`, `configured_data_quality_cursor_key@L162`, `cursor_key_configuration_is_fail_closed_and_deterministic@L176`
- **Detected tests (1):** `cursor_key_configuration_is_fail_closed_and_deterministic@L176`
- **Error/guard literals:** `DATA_QUALITY_RULE_SET_QUERY_INPUT`, `DATA_QUALITY_COMPLETENESS_PROFILE_QUERY_INPUT`, `DATA_QUALITY_EVALUATION_QUERY_INPUT`, `CRM_CURSOR_SIGNING_KEY`

### [`src/query_dispatch.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-query-adapter/src/query_dispatch.rs) · 64 lines

- **Internal/trait/helpers (2):** `validate@L2`, `execute@L47`
- **Error/guard literals:** `DATA_QUALITY_RULE_SET_QUERY_INPUT`, `DATA_QUALITY_COMPLETENESS_PROFILE_QUERY_INPUT`, `DATA_QUALITY_EVALUATION_QUERY_INPUT`, `DATA_QUALITY_FINDING_QUERY_INPUT`, `DATA_QUALITY_COMPLETENESS_RESULT_QUERY_INPUT`

### [`src/query_filters.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-query-adapter/src/query_filters.rs) · 122 lines

- **Internal/trait/helpers (9):** `validated_status_filter@L1`, `validated_severity_filter@L22`, `status_to_wire@L37`, `severity_to_wire@L46`, `status_filter_wire@L55`, `severity_filter_wire@L59`, `finding_cursor_binding@L63`, `decode_finding_after@L80`, `encode_finding_next@L102`

### [`src/query_finding_get.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-query-adapter/src/query_finding_get.rs) · 60 lines

- **Internal/trait/helpers (1):** `execute_get_finding@L2`
- **Error/guard literals:** `DATA_QUALITY_FINDING_QUERY_INPUT`

### [`src/query_lists.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-query-adapter/src/query_lists.rs) · 40 lines

- **Internal/trait/helpers (3):** `execute_list_findings_by_party@L2`, `execute_list_assigned_findings@L16`, `execute_get_party_completeness_result@L30`
- **Error/guard literals:** `DATA_QUALITY_FINDING_QUERY_INPUT`, `DATA_QUALITY_COMPLETENESS_RESULT_QUERY_INPUT`

### [`src/query_store.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-query-adapter/src/query_store.rs) · 140 lines

- **Internal/trait/helpers (4):** `load_snapshot@L2`, `visible_or@L22`, `collect_findings@L37`, `has_more_visible_finding@L103`

### [`src/query_support.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-query-adapter/src/query_support.rs) · 110 lines

- **Internal/trait/helpers (1):** `fmt@L100`
- **Public types:** `DataQualityQueryAdapter@L92`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_data_quality`, `crm_data_quality_capability_adapter`, `crm_module_sdk`, `crm_parties_query_adapter`, `crm_proto_contracts`, `crm_query_runtime`

### [`src/query_visibility.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-query-adapter/src/query_visibility.rs) · 152 lines

- **Internal/trait/helpers (3):** `data_quality_visibility@L5`, `visible_fields@L53`, `apply_deployment_hidden_fields@L117`
- **Error/guard literals:** `CRM_QUERY_HIDDEN_FIELDS`
- **Rust crate references:** `crm_module_sdk`

### [`src/query_wire_completeness.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-query-adapter/src/query_wire_completeness.rs) · 36 lines

- **Internal/trait/helpers (1):** `completeness_result_to_wire_with_visibility@L1`

### [`src/query_wire_evaluation.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-query-adapter/src/query_wire_evaluation.rs) · 19 lines

- **Internal/trait/helpers (1):** `evaluation_job_to_wire_with_visibility@L1`

### [`src/query_wire_finding.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-query-adapter/src/query_wire_finding.rs) · 58 lines

- **Internal/trait/helpers (2):** `finding_to_wire_with_visibility@L1`, `observation_to_wire_with_visibility@L38`

### [`tests/public_surface.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-query-adapter/tests/public_surface.rs) · 51 lines

- **Internal/trait/helpers (1):** `data_quality_queries_remain_exact_classified_read_only_surfaces@L11`
- **Detected tests (1):** `data_quality_queries_remain_exact_classified_read_only_surfaces@L11`
- **Rust crate references:** `crm_capability_runtime`, `crm_data_quality_query_adapter`, `crm_module_sdk`

## 62. `crm-data-quality-source-composition` — NEXT/LATER / B-CONDITIONAL + D? adapter

### [`src/capability_execution.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-source-composition/src/capability_execution.rs) · 527 lines

- **Public declarations (1):** `new@L52`
- **Internal/trait/helpers (20):** `execute_remediation@L70`, `ensure_display_name_rule@L225`, `fmt@L262`, `execute@L279`, `bind_target_request@L352`, `authorize_target@L377`, `decode_updated_party@L398`, `required_finding_id@L416`, `required_observation_id@L428`, `fail_after_target_once@L440`, `module_id@L445`, `record_type@L449`, `rule_set_unavailable@L453`, `evaluation_definitions_unavailable@L462`, `remediation_finding_unavailable@L471`, `remediation_evidence_conflict@L480`, `remediation_rule_unavailable@L489`, `remediation_target_contract_invalid@L498`, `module_not_active@L508`, `reference_configuration_error@L518`
- **Public types:** `DataQualityCapabilityExecutor@L42`
- **Error/guard literals:** `DATA_QUALITY_REMEDIATION_OUTCOME_DEFERRED`, `DATA_QUALITY_REMEDIATION_RULE_UNSUPPORTED`, `DATA_QUALITY_REMEDIATION_TARGET_PERMISSION_DENIED`, `CRM_DATA_QUALITY_REMEDIATION_FAIL_AFTER_TARGET_ONCE`, `DATA_QUALITY_COMPLETENESS_RULE_SET_UNAVAILABLE`, `DATA_QUALITY_EVALUATION_DEFINITIONS_UNAVAILABLE`, `DATA_QUALITY_FINDING_NOT_FOUND`, `DATA_QUALITY_REMEDIATION_EVIDENCE_CONFLICT`, `DATA_QUALITY_REMEDIATION_RULE_UNAVAILABLE`, `DATA_QUALITY_REMEDIATION_TARGET_CONTRACT_INVALID`, `MODULE_NOT_ACTIVE`, `DATA_QUALITY_REFERENCE_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_application_composition`, `crm_capability_adapters`, `crm_capability_runtime`, `crm_core_data`, `crm_data_quality_capability_adapter`, `crm_data_quality_query_adapter`, `crm_module_sdk`, `crm_parties_capability_adapter`, `crm_proto_contracts`, `crm_query_runtime`

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-source-composition/src/lib.rs) · 210 lines

- **Public declarations (1):** `new@L79`
- **Internal/trait/helpers (8):** `get@L66`, `fmt@L91`, `get@L101`, `authorize_party_get@L153`, `source_not_found@L174`, `source_contract_error@L183`, `assert_send_sync@L197`, `minimized_source_contract_is_closed_and_thread_safe@L200`
- **Public types:** `PartyQualitySourceKind@L43`, `PartyQualitySourceRequest@L49`, `PartyQualitySourceSnapshot@L58`, `PartyQualitySource@L65`, `GovernedPartyQualitySource@L73`
- **Re-exports:** `pub use capability_execution::DataQualityCapabilityExecutor;`, `pub use production_contribution::*;`, `pub use registration::DataQualityAggregatePlanner;`, `pub use materialization_sink::*;`, `pub use staging_sink::*;`, `pub use worker::*;`, `pub use worker_context::{EVALUATION_WORKER_ACTOR_ID, EVALUATION_WORKER_CAPABILITY_VERSION};`
- **Detected tests (1):** `minimized_source_contract_is_closed_and_thread_safe@L200`
- **Error/guard literals:** `DATA_QUALITY_PARTY_SOURCE_PERMISSION_DENIED`, `QUERY_RESOURCE_NOT_FOUND`, `DATA_QUALITY_PARTY_SOURCE_CONTRACT_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_module_sdk`, `crm_parties_query_adapter`, `crm_proto_contracts`, `crm_query_runtime`

### [`src/materialization_sink.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-source-composition/src/materialization_sink.rs) · 306 lines

- **Public declarations (2):** `new@L43`, `materialize@L47`
- **Internal/trait/helpers (9):** `load_current_finding_evidence@L107`, `load_optional_record@L180`, `fmt@L198`, `decode_current_finding@L207`, `decode_current_observation@L229`, `bind_materialization_request@L251`, `hex@L277`, `configuration_error@L287`, `materialization_state_invalid@L297`
- **Public types:** `PostgresPartyEvaluationMaterializationSink@L37`
- **Error/guard literals:** `DATA_QUALITY_EVALUATION_MATERIALIZATION_PERMISSION_DENIED`, `DATA_QUALITY_EVALUATION_MATERIALIZATION_CONFIGURATION_INVALID`, `DATA_QUALITY_EVALUATION_MATERIALIZATION_STATE_INVALID`
- **Rust crate references:** `crm_capability_adapters`, `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_data_quality`, `crm_data_quality_capability_adapter`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/production_contribution.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-source-composition/src/production_contribution.rs) · 111 lines

- **Public declarations (3):** `mutation_capability_definitions@L34`, `query_capability_definitions@L38`, `build_contribution@L42`
- **Internal/trait/helpers (1):** `production_composition_error@L102`
- **Public types:** `DataQualityProductionDependencies@L26`
- **Error/guard literals:** `DATA_QUALITY_PRODUCTION_COMPOSITION_INVALID`
- **Rust crate references:** `crm_application_composition`, `crm_capability_runtime`, `crm_core_data`, `crm_data_quality_capability_adapter`, `crm_data_quality_query_adapter`, `crm_module_sdk`, `crm_parties_capability_adapter`, `crm_query_runtime`

### [`src/registration.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-source-composition/src/registration.rs) · 64 lines

- **Internal/trait/helpers (3):** `target@L14`, `plan@L32`, `unsupported_capability@L52`
- **Public types:** `DataQualityAggregatePlanner@L11`
- **Error/guard literals:** `DATA_QUALITY_AGGREGATE_CAPABILITY_UNSUPPORTED`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_data_quality_capability_adapter`, `crm_module_sdk`

### [`src/staging_command.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-source-composition/src/staging_command.rs) · 38 lines

- **Public declarations (1):** `stage_command@L9`
- **Error/guard literals:** `DATA_QUALITY_EVALUATION_STAGE_STATE_INVALID`
- **Rust crate references:** `crm_data_quality`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/staging_context.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-source-composition/src/staging_context.rs) · 53 lines

- **Public declarations (1):** `bind_stage_request@L8`
- **Internal/trait/helpers (2):** `hex@L34`, `configuration_error@L44`
- **Error/guard literals:** `DATA_QUALITY_EVALUATION_STAGE_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_module_sdk`

### [`src/staging_execute.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-source-composition/src/staging_execute.rs) · 65 lines

- **Public declarations (1):** `execute_stage@L8`
- **Internal/trait/helpers (1):** `batch_error_to_sdk@L46`
- **Error/guard literals:** `DATA_QUALITY_EVALUATION_STAGE_PERMISSION_DENIED`, `DATA_QUALITY_EVALUATION_STAGE_CONFLICT`, `DATA_QUALITY_EVALUATION_STAGE_FAILED`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_data_quality_capability_adapter`, `crm_module_sdk`

### [`src/staging_request.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-source-composition/src/staging_request.rs) · 40 lines

- **Public declarations (1):** `prepare_stage_request@L19`
- **Public types:** `PreparedStageRequest@L14`
- **Rust crate references:** `crm_capability_adapters`, `crm_capability_runtime`, `crm_data_quality`, `crm_data_quality_capability_adapter`, `crm_module_sdk`

### [`src/staging_sink.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-source-composition/src/staging_sink.rs) · 46 lines

- **Public declarations (2):** `new@L19`, `stage@L23`
- **Internal/trait/helpers (1):** `fmt@L38`
- **Public types:** `PostgresPartyEvaluationStageSink@L13`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_data_quality`, `crm_module_sdk`

### [`src/worker_context.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-source-composition/src/worker_context.rs) · 53 lines

- **Public declarations (2):** `evaluation_worker_context@L11`, `worker_actor_id@L40`
- **Internal/trait/helpers (1):** `config_error@L44`
- **Error/guard literals:** `DATA_QUALITY_EVALUATION_WORKER_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_data_quality_capability_adapter`, `crm_module_sdk`, `crm_data_quality`

### [`src/worker.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-data-quality-source-composition/src/worker.rs) · 354 lines

- **Public declarations (4):** `new@L53`, `new_with_materialization@L69`, `try_with_page_size@L87`, `run_tenant_cycle@L125`
- **Internal/trait/helpers (9):** `build@L98`, `load_evaluation_input@L241`, `load_owned_record@L281`, `fmt@L302`, `data_quality_module_id@L318`, `record_type@L322`, `config_error@L326`, `materialization_state_invalid@L336`, `worker_unavailable@L346`
- **Public types:** `EvaluationStageTenantCycle@L32`, `PartyEvaluationStageWorker@L41`
- **Error/guard literals:** `DATA_QUALITY_EVALUATION_WORKER_CONFIGURATION_INVALID`, `DATA_QUALITY_EVALUATION_MATERIALIZATION_STATE_INVALID`, `DATA_QUALITY_EVALUATION_WORKER_STATE_UNAVAILABLE`
- **Rust crate references:** `crm_capability_plan_support`, `crm_core_data`, `crm_data_quality`, `crm_data_quality_capability_adapter`, `crm_module_sdk`

## 63. `crm-first-party-modules` — RETIRE-REVIEW / D? + B? invariants

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-first-party-modules/src/lib.rs) · 219 lines

- **Public declarations (1):** `build_all@L110`
- **Public types:** `FirstPartyProductionDependencies@L99`
- **Re-exports:** `pub use crm_consents_capability_composition::{`, `pub use crm_contact_points_capability_composition::{`, `pub use crm_customer_360_query_adapter::MODULE_ID as CUSTOMER_360_MODULE_ID;`, `pub use crm_customer_360_query_adapter::query_capability_definitions as customer_360_query_capability_definitions;`, `pub use crm_customer_accounts_query_adapter::{`, `pub use crm_customer_data_operations_execution_composition::{`, `pub use crm_customer_enrichment_capability_composition::{`, `pub use crm_data_quality_source_composition::{`, `pub use crm_identity_resolution_capability_composition::{`, `pub use crm_party_reference_composition::{`, `pub use crm_party_relationships_capability_composition::{`, `pub use crm_sales_activities_capability_composition::{`
- **Rust crate references:** `crm_application_composition`, `crm_capability_runtime`, `crm_consents_capability_composition`, `crm_contact_points_capability_composition`, `crm_core_data`, `crm_customer_360_query_adapter`, `crm_customer_accounts_query_adapter`, `crm_customer_data_operations_execution_composition`, `crm_customer_enrichment_capability_composition`, `crm_data_quality_source_composition`, `crm_identity_resolution_capability_composition`, `crm_module_sdk`, `crm_party_reference_composition`, `crm_party_relationships_capability_composition`, `crm_query_runtime`, `crm_sales_activities_capability_composition`

## 64. `crm-global-search-composition` — LATER / B-LATER + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-global-search-composition/src/lib.rs) · 691 lines

- **Public declarations (8):** `new@L81`, `new@L569`, `for_generation@L573`, `generation_id@L591`, `projection_id@L595`, `ensure_ready@L599`, `catch_up@L607`, `reindex@L615`
- **Internal/trait/helpers (19):** `project@L116`, `party_document@L251`, `party_privacy_action_document@L303`, `validate_party_privacy_action@L325`, `require_canonical_json_field@L380`, `canonical_json_string_field@L389`, `single_title_document@L420`, `search_document@L443`, `validate_snapshot@L471`, `validate_contract@L488`, `decode@L510`, `configured_event_types@L518`, `validate_generation_id@L528`, `search_configuration_invalid@L543`, `search_event_invalid@L553`, `generation_registry_subscribes_to_owner_snapshots_and_party_privacy_actions@L632`, `production_generation_advances_to_g3_for_privacy_event_replay@L659`, `canonical_owner_action_fields_are_unique_and_unescaped@L667`, `generation_id_rejects_path_or_whitespace_characters@L686`
- **Public types:** `SearchProjectionGeneration@L74`, `GlobalSearchWorker@L564`, `Phase7SearchWorker@L625`
- **Detected tests (4):** `generation_registry_subscribes_to_owner_snapshots_and_party_privacy_actions@L632`, `production_generation_advances_to_g3_for_privacy_event_replay@L659`, `canonical_owner_action_fields_are_unique_and_unescaped@L667`, `generation_id_rejects_path_or_whitespace_characters@L686`
- **Error/guard literals:** `GLOBAL_SEARCH_CONFIGURATION_INVALID`, `GLOBAL_SEARCH_EVENT_INVALID`
- **Rust crate references:** `crm_core_data`, `crm_core_events`, `crm_module_sdk`, `crm_projection_runtime`, `crm_proto_contracts`, `crm_search_runtime`

## 65. `crm-identity-resolution-capability-adapter` — LATER / B-LATER + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-capability-adapter/src/lib.rs) · 218 lines

- **Public declarations (2):** `capability_definitions@L89`, `capability_definition@L96`
- **Internal/trait/helpers (4):** `configured@L165`, `configuration_error@L175`, `publishes_candidate_and_merge_mutation_coordinates_with_exact_governance@L184`, `rejects_unknown_identity_resolution_mutation_coordinate@L213`
- **Re-exports:** `pub use merge_planner::*;`, `pub use planner::*;`
- **Detected tests (2):** `publishes_candidate_and_merge_mutation_coordinates_with_exact_governance@L184`, `rejects_unknown_identity_resolution_mutation_coordinate@L213`
- **Error/guard literals:** `IDENTITY_RESOLUTION_CAPABILITY_UNSUPPORTED`, `IDENTITY_RESOLUTION_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_module_sdk`

### [`src/merge_planner.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-capability-adapter/src/merge_planner.rs) · 545 lines

- **Public declarations (5):** `merge_reference_scope_from_request@L106`, `merge_operation_from_snapshot@L146`, `merge_operation_to_wire@L157`, `merge_persisted_contract@L212`, `merge_persisted_payload@L223`
- **Internal/trait/helpers (14):** `target@L42`, `plan@L91`, `plan_merge@L233`, `plan_unmerge@L290`, `merge_operation_from_command@L359`, `survivorship_from_wire@L394`, `required_party_expectation@L415`, `required_party_ref@L433`, `merge_operation_id_from_ref@L442`, `ensure_definition@L452`, `mutation_plan@L467`, `invalid_plan@L495`, `unsupported_capability@L504`, `merge_persisted_contract_is_personal_json_and_stably_hashed@L518`
- **Public types:** `MergeLineageCapabilityPlanner@L26`, `MergePartyVersionExpectation@L29`, `MergeReferenceScope@L35`
- **Detected tests (1):** `merge_persisted_contract_is_personal_json_and_stably_hashed@L518`
- **Error/guard literals:** `IDENTITY_RESOLUTION_MERGE_PLAN_INVALID`, `IDENTITY_RESOLUTION_MERGE_CAPABILITY_UNSUPPORTED`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_identity_resolution`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/owner_planner.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-capability-adapter/src/owner_planner.rs) · 715 lines

- **Public declarations (5):** `evidence_reference_scope_from_request@L425`, `duplicate_candidate_case_to_wire@L473`, `persisted_contract@L596`, `persisted_payload@L607`, `duplicate_candidate_case_from_snapshot@L617`
- **Internal/trait/helpers (18):** `target@L45`, `plan@L119`, `plan_register@L136`, `plan_refresh@L196`, `plan_dismiss@L228`, `plan_confirm@L259`, `update_plan@L298`, `mutation_plan@L397`, `evidence_to_wire@L516`, `evidence_from_wire@L544`, `ensure_requested_case_matches@L636`, `case_id_from_ref@L647`, `party_reference_from_ref@L656`, `required_time@L664`, `ensure_definition@L670`, `invalid_plan@L685`, `unsupported_capability@L694`, `persisted_contract_is_exact_and_nonzero@L708`
- **Public types:** `IdentityResolutionCapabilityPlanner@L31`, `PartyVersionExpectation@L34`, `EvidenceReferenceScope@L40`
- **Detected tests (1):** `persisted_contract_is_exact_and_nonzero@L708`
- **Error/guard literals:** `IDENTITY_RESOLUTION_PERSISTED_CASE_IDENTITY_INVALID`, `IDENTITY_RESOLUTION_MUTATION_PLAN_INVALID`, `IDENTITY_RESOLUTION_CAPABILITY_UNSUPPORTED`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_identity_resolution`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/planner.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-capability-adapter/src/planner.rs) · 337 lines

- **Internal/trait/helpers (18):** `target@L68`, `plan@L82`, `add_candidate_party_links@L110`, `add_merge_party_links@L137`, `add_canonical_redirect_link@L159`, `remove_canonical_redirect_link@L175`, `party_record_ref@L190`, `candidate_party_link_payload@L199`, `merge_party_link_payload@L214`, `canonical_redirect_payload@L229`, `configured_relationship_type@L244`, `configured_record_type@L248`, `invalid_registration_scope@L252`, `invalid_merge_state@L261`, `unsupported_capability@L270`, `config_error@L279`, `candidate_merge_and_redirect_link_contracts_are_personal_json_and_distinct@L294`, `authoritative_party_access_and_redirect_coordinates_are_stable@L310`
- **Public types:** `IdentityResolutionCapabilityPlanner@L65`
- **Re-exports:** `pub use owner_planner::{`
- **Detected tests (2):** `candidate_merge_and_redirect_link_contracts_are_personal_json_and_distinct@L294`, `authoritative_party_access_and_redirect_coordinates_are_stable@L310`
- **Error/guard literals:** `IDENTITY_RESOLUTION_REGISTRATION_SCOPE_INVALID`, `IDENTITY_RESOLUTION_MERGE_STATE_INVALID`, `IDENTITY_RESOLUTION_CAPABILITY_UNSUPPORTED`, `IDENTITY_RESOLUTION_RELATIONSHIP_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_identity_resolution`, `crm_module_sdk`

## 66. `crm-identity-resolution-capability-composition` — LATER / B-LATER + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-capability-composition/src/lib.rs) · 498 lines

- **Public declarations (5):** `new@L63`, `new@L133`, `new@L182`, `validate_evidence_scope@L200`, `validate_current_candidate_evidence@L222`
- **Internal/trait/helpers (22):** `party_version@L44`, `candidate_evidence_versions@L50`, `party_version@L69`, `candidate_evidence_versions@L91`, `fmt@L124`, `validate@L139`, `fmt@L173`, `execute@L188`, `candidate_evidence_versions@L251`, `candidate_case_id_from_request@L268`, `reference_unavailable@L312`, `evidence_stale@L321`, `unsupported_capability@L330`, `config_error@L339`, `party_version@L364`, `candidate_evidence_versions@L378`, `tenant@L393`, `party@L397`, `reader@L401`, `exact_same_tenant_party_versions_are_required@L413`, `missing_party_fails_closed_and_version_drift_is_a_conflict@L434`, `terminal_decision_rechecks_current_candidate_evidence_versions@L478`
- **Public types:** `CandidateEvidenceVersions@L38`, `IdentityResolutionReferenceReader@L43`, `PostgresIdentityResolutionReferenceReader@L58`, `IdentityResolutionCapabilitySemanticValidator@L119`, `IdentityResolutionCapabilityExecutor@L168`
- **Re-exports:** `pub use production_contribution::*;`
- **Detected tests (3):** `exact_same_tenant_party_versions_are_required@L413`, `missing_party_fails_closed_and_version_drift_is_a_conflict@L434`, `terminal_decision_rechecks_current_candidate_evidence_versions@L478`
- **Error/guard literals:** `IDENTITY_RESOLUTION_REFERENCE_UNAVAILABLE`, `IDENTITY_RESOLUTION_EVIDENCE_STALE`, `IDENTITY_RESOLUTION_COMPOSITION_CAPABILITY_UNSUPPORTED`, `IDENTITY_RESOLUTION_REFERENCE_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_identity_resolution`, `crm_identity_resolution_capability_adapter`, `crm_module_sdk`, `crm_parties_capability_adapter`, `crm_proto_contracts`

### [`src/production_contribution.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-capability-composition/src/production_contribution.rs) · 177 lines

- **Public declarations (3):** `mutation_capability_definitions@L47`, `query_capability_definitions@L51`, `build_contribution@L57`
- **Internal/trait/helpers (3):** `select_definitions@L145`, `cursor@L156`, `production_composition_error@L168`
- **Public types:** `IdentityResolutionProductionDependencies@L40`
- **Error/guard literals:** `IDENTITY_RESOLUTION_CURSOR_CONFIGURATION_INVALID`, `IDENTITY_RESOLUTION_PRODUCTION_COMPOSITION_INVALID`
- **Rust crate references:** `crm_application_composition`, `crm_capability_runtime`, `crm_core_data`, `crm_identity_resolution_capability_adapter`, `crm_identity_resolution_merge_composition`, `crm_identity_resolution_merge_query_adapter`, `crm_identity_resolution_query_adapter`, `crm_module_sdk`, `crm_query_runtime`

## 67. `crm-identity-resolution-merge-composition` — LATER / B-LATER + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-merge-composition/src/lib.rs) · 499 lines

- **Public declarations (3):** `new@L65`, `new@L161`, `new@L300`
- **Internal/trait/helpers (30):** `current_party_version@L40`, `merge_operation@L46`, `immediate_redirect_target@L52`, `current_party_version@L71`, `merge_operation@L90`, `immediate_redirect_target@L109`, `fmt@L152`, `validate_merge@L165`, `validate_unmerge@L215`, `validate@L269`, `fmt@L291`, `execute@L306`, `require_exact_party_version@L318`, `resolve_canonical_root@L340`, `configured_module_id@L366`, `configured_record_type@L370`, `configured_record_id@L374`, `configured_relationship_type@L378`, `party_reference_unavailable@L382`, `stale_party_version@L391`, `invalid_provenance_party@L400`, `invalid_provenance_version@L409`, `source_not_canonical@L418`, `survivor_not_canonical@L427`, `merge_operation_unavailable@L436`, `merge_operation_not_active@L445`, `canonical_redirect_corrupt@L454`, `unsupported_capability@L464`, `config_error@L473`, `merge_capability_coordinates_are_separate_from_candidate_mutations@L488`
- **Public types:** `MergeLineageReferenceReader@L39`, `PostgresMergeLineageReferenceReader@L60`, `MergeLineageCapabilitySemanticValidator@L147`, `MergeLineageCapabilityExecutor@L286`
- **Detected tests (1):** `merge_capability_coordinates_are_separate_from_candidate_mutations@L488`
- **Error/guard literals:** `IDENTITY_RESOLUTION_MERGE_PARTY_REFERENCE_UNAVAILABLE`, `IDENTITY_RESOLUTION_MERGE_PARTY_VERSION_STALE`, `IDENTITY_RESOLUTION_SURVIVORSHIP_PROVENANCE_PARTY_INVALID`, `IDENTITY_RESOLUTION_SURVIVORSHIP_PROVENANCE_VERSION_INVALID`, `IDENTITY_RESOLUTION_MERGE_SOURCE_ALREADY_REDIRECTED`, `IDENTITY_RESOLUTION_MERGE_SURVIVOR_NOT_CANONICAL`, `IDENTITY_RESOLUTION_MERGE_OPERATION_UNAVAILABLE`, `IDENTITY_RESOLUTION_MERGE_OPERATION_NOT_ACTIVE`, `IDENTITY_RESOLUTION_CANONICAL_REDIRECT_INVALID`, `IDENTITY_RESOLUTION_MERGE_CAPABILITY_UNSUPPORTED`, `IDENTITY_RESOLUTION_MERGE_COMPOSITION_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_identity_resolution`, `crm_identity_resolution_capability_adapter`, `crm_module_sdk`, `crm_parties_capability_adapter`, `crm_proto_contracts`

## 68. `crm-identity-resolution-merge-query-adapter` — LATER / B-LATER + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-merge-query-adapter/src/lib.rs) · 851 lines

- **Public declarations (3):** `query_capability_definitions@L65`, `query_capability_definition@L72`, `new@L126`
- **Internal/trait/helpers (36):** `fmt@L114`, `execute_get@L145`, `execute_list_by_party@L182`, `execute_resolve@L211`, `collect_merge_operations@L263`, `has_more_visible_operation@L332`, `immediate_redirect_target@L373`, `active_operation_for_edge@L410`, `validate@L466`, `execute@L496`, `merge_related_query@L513`, `validate_list@L537`, `merge_operation_to_wire_with_visibility@L553`, `required_party_ref@L579`, `required_merge_operation_ref@L589`, `optional_status@L601`, `status_wire_value@L613`, `cursor_binding@L621`, `decode_after@L643`, `encode_next@L662`, `decode_input@L683`, `validate_record_id@L714`, `configured_capability_id@L719`, `configured_capability_version@L723`, `configured_module_id@L727`, `configured_record_type@L731`, `configured_relationship_type@L735`, `enforce_visibility_scan_limit@L739`, `resource_not_found@L747`, `visibility_scan_limit_error@L756`, `canonical_redirect_corrupt@L765`, `cursor_invalid@L775`, `cursor_error@L784`, `unsupported_query@L807`, `config_error@L816`, `publishes_get_list_and_canonical_resolution_query_coordinates@L831`
- **Public types:** `IdentityResolutionMergeQueryAdapter@L106`
- **Detected tests (1):** `publishes_get_list_and_canonical_resolution_query_coordinates@L831`
- **Error/guard literals:** `IDENTITY_RESOLUTION_MERGE_QUERY_INPUT_CONTRACT_MISMATCH`, `IDENTITY_RESOLUTION_MERGE_QUERY_INPUT_PROTOBUF_INVALID`, `QUERY_RESOURCE_NOT_FOUND`, `IDENTITY_RESOLUTION_MERGE_QUERY_SCAN_LIMIT_EXCEEDED`, `IDENTITY_RESOLUTION_CANONICAL_REDIRECT_INVALID`, `IDENTITY_RESOLUTION_MERGE_QUERY_CURSOR_INVALID`, `IDENTITY_RESOLUTION_MERGE_QUERY_CAPABILITY_UNSUPPORTED`, `IDENTITY_RESOLUTION_MERGE_QUERY_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_identity_resolution`, `crm_identity_resolution_capability_adapter`, `crm_module_sdk`, `crm_parties_capability_adapter`, `crm_proto_contracts`, `crm_query_runtime`

## 69. `crm-identity-resolution-privacy-scope-adapter` — END / B-END + D? adapter

### [`src/contract.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-privacy-scope-adapter/src/contract.rs) · 103 lines

- **Public declarations (8):** `identity_resolution_privacy_scope_definition@L33`, `validate_definition@L53`, `module_id@L64`, `capability_id@L68`, `capability_version@L72`, `schema_id@L76`, `schema_version@L80`, `output_descriptor_hash@L84`
- **Internal/trait/helpers (1):** `payload_contract@L88`
- **Error/guard literals:** `IDENTITY_RESOLUTION_PRIVACY_SCOPE_DEFINITION_MISMATCH`
- **Rust crate references:** `crm_capability_runtime`, `crm_identity_resolution`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/digest.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-privacy-scope-adapter/src/digest.rs) · 44 lines

- **Public declarations (2):** `encode_hex@L3`, `decode_hex@L13`
- **Internal/trait/helpers (2):** `nibble@L30`, `cursor_invalid@L38`
- **Error/guard literals:** `IDENTITY_RESOLUTION_PRIVACY_SCOPE_CURSOR_INVALID`
- **Rust crate references:** `crm_module_sdk`

### [`src/errors.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-privacy-scope-adapter/src/errors.rs) · 196 lines

- **Public declarations (13):** `configured@L6`, `map_request_contract_error@L18`, `map_common_lineage_error@L37`, `map_canonical_party_claim_error@L91`, `lineage_invalid@L117`, `stored_candidate_state_invalid@L131`, `stored_merge_state_invalid@L141`, `topology_invalid@L151`, `limit_exceeded@L161`, `row_decode_error@L171`, `database_unavailable@L175`, `invalid_contract@L185`, `invalid_contract_with_reference@L189`
- **Error/guard literals:** `IDENTITY_RESOLUTION_PRIVACY_SCOPE_CONFIGURATION_INVALID`, `IDENTITY_RESOLUTION_PRIVACY_SCOPE_REQUEST_BINDING_MISMATCH`, `IDENTITY_RESOLUTION_PRIVACY_SCOPE_INPUT_CONTRACT_MISMATCH`, `IDENTITY_RESOLUTION_PRIVACY_SCOPE_INPUT_HASH_MISMATCH`, `IDENTITY_RESOLUTION_PRIVACY_SCOPE_TENANT_MISMATCH`, `IDENTITY_RESOLUTION_PRIVACY_SCOPE_CASE_ID_INVALID`, `IDENTITY_RESOLUTION_PRIVACY_SCOPE_PARTY_INVALID`, `IDENTITY_RESOLUTION_PRIVACY_SCOPE_GENERATION_INVALID`, `IDENTITY_RESOLUTION_PRIVACY_SCOPE_REGISTRY_INVALID`, `IDENTITY_RESOLUTION_PRIVACY_SCOPE_REGISTRY_UNAVAILABLE`, `IDENTITY_RESOLUTION_PRIVACY_SCOPE_REGISTRY_MISMATCH`, `IDENTITY_RESOLUTION_PRIVACY_SCOPE_PURPOSE_INVALID`, `IDENTITY_RESOLUTION_PRIVACY_SCOPE_REQUEST_TIME_INVALID`, `IDENTITY_RESOLUTION_PRIVACY_SCOPE_PAGE_SIZE_INVALID`, `IDENTITY_RESOLUTION_PRIVACY_SCOPE_LINEAGE_INVALID`, `IDENTITY_RESOLUTION_PRIVACY_SCOPE_CANDIDATE_STATE_INVALID`, `IDENTITY_RESOLUTION_PRIVACY_SCOPE_MERGE_STATE_INVALID`, `IDENTITY_RESOLUTION_PRIVACY_SCOPE_UNAVAILABLE`
- **Rust crate references:** `crm_customer_privacy_owner_scope_support`, `crm_module_sdk`

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-privacy-scope-adapter/src/lib.rs) · 37 lines

- **Re-exports:** `pub use contract::{`, `pub use owner_action::{`, `pub use postgres::IdentityResolutionPrivacyScopeQueryAdapter;`

### [`src/owner_action.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-privacy-scope-adapter/src/owner_action.rs) · 234 lines

- **Public declarations (2):** `identity_resolution_privacy_action_definition@L26`, `identity_resolution_privacy_action_planner@L33`
- **Internal/trait/helpers (15):** `owner_module_id@L39`, `capability_id@L43`, `supports_resource_type@L47`, `anonymize@L51`, `deletion_tombstone@L59`, `plan_resource_action@L68`, `minimize_candidate_case@L88`, `candidate_privacy_transition@L98`, `minimized_evidence@L147`, `minimized_evidence_at@L155`, `hex_prefix@L181`, `transition_invalid@L190`, `publishes_the_frozen_owner_action_coordinate@L205`, `supports_both_authoritative_identity_resolution_record_types@L220`, `privacy_evidence_reference_is_deterministic_and_bounded@L228`
- **Public types:** `IdentityResolutionPrivacyActionPlanner@L20`, `IdentityResolutionPrivacyActionPolicy@L24`
- **Detected tests (3):** `publishes_the_frozen_owner_action_coordinate@L205`, `supports_both_authoritative_identity_resolution_record_types@L220`, `privacy_evidence_reference_is_deterministic_and_bounded@L228`
- **Error/guard literals:** `IDENTITY_RESOLUTION_PRIVACY_TRANSITION_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_customer_privacy_owner_scope_support`, `crm_identity_resolution`, `crm_identity_resolution_capability_adapter`, `crm_module_sdk`

### [`src/postgres.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-privacy-scope-adapter/src/postgres.rs) · 885 lines

- **Public declarations (1):** `new@L58`
- **Internal/trait/helpers (20):** `fmt@L49`, `execute_query@L62`, `execute@L98`, `read_identity_resolution_page@L129`, `resource_after_cursor@L266`, `load_redirect_pairs@L280`, `validate_redirects_against_operations@L344`, `reverse_alias_closure@L379`, `load_related_record_ids@L449`, `load_candidate_records@L514`, `load_merge_records@L529`, `load_record_rows@L558`, `strict_candidate_record@L630`, `strict_merge_record@L672`, `record_snapshot@L716`, `validate_and_select_candidates@L752`, `validate_and_select_merges@L797`, `reference_relevant@L859`, `configured@L877`, `_definition_smoke@L882`
- **Public types:** `IdentityResolutionPrivacyScopeQueryAdapter@L44`
- **Error/guard literals:** `IDENTITY_RESOLUTION_PRIVACY_SCOPE_GRAPH_INVALID`, `IDENTITY_RESOLUTION_PRIVACY_SCOPE_RELATIONSHIP_LIMIT_EXCEEDED`, `IDENTITY_RESOLUTION_PRIVACY_SCOPE_OWNER_SCAN_LIMIT_EXCEEDED`, `IDENTITY_RESOLUTION_PRIVACY_SCOPE_EDGE_LIMIT_EXCEEDED`, `IDENTITY_RESOLUTION_PRIVACY_SCOPE_REDIRECT_DUPLICATE`, `IDENTITY_RESOLUTION_PRIVACY_SCOPE_REDIRECT_OPERATION_DUPLICATE`, `IDENTITY_RESOLUTION_PRIVACY_SCOPE_REDIRECT_OPERATION_MISMATCH`, `IDENTITY_RESOLUTION_PRIVACY_SCOPE_ALIAS_DEPTH_EXCEEDED`, `IDENTITY_RESOLUTION_PRIVACY_SCOPE_ALIAS_NODE_LIMIT_EXCEEDED`, `IDENTITY_RESOLUTION_PRIVACY_SCOPE_ALIAS_COMPLETENESS_INVALID`, `IDENTITY_RESOLUTION_PRIVACY_SCOPE_RELATIONSHIP_INVALID`, `IDENTITY_RESOLUTION_PRIVACY_SCOPE_CANDIDATE_SCAN_LIMIT_EXCEEDED`, `IDENTITY_RESOLUTION_PRIVACY_SCOPE_MERGE_SCAN_LIMIT_EXCEEDED`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_privacy_owner_scope_support`, `crm_identity_resolution`, `crm_identity_resolution_capability_adapter`, `crm_module_sdk`, `crm_query_runtime`

### [`src/request.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-privacy-scope-adapter/src/request.rs) · 242 lines

- **Public declarations (4):** `token@L28`, `validate_request_contract@L60`, `validate_wire_request@L65`, `encode_cursor@L126`
- **Internal/trait/helpers (4):** `parse@L35`, `decode_cursor@L155`, `cursor_digest@L204`, `cursor_invalid@L236`
- **Public types:** `ResourceFamily@L22`, `CursorState@L45`, `ValidatedRequest@L51`
- **Error/guard literals:** `IDENTITY_RESOLUTION_PRIVACY_SCOPE_REQUEST_INVALID`, `IDENTITY_RESOLUTION_PRIVACY_SCOPE_LINEAGE_INVALID`, `IDENTITY_RESOLUTION_PRIVACY_SCOPE_CURSOR_INVALID`
- **Rust crate references:** `crm_customer_privacy_owner_scope_support`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`

### [`src/response.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-privacy-scope-adapter/src/response.rs) · 160 lines

- **Public declarations (2):** `build_response@L42`, `typed_output@L115`
- **Internal/trait/helpers (3):** `resource_type@L27`, `retention_policy_id@L34`, `page_digest@L131`
- **Public types:** `VerifiedIdentityResolutionResource@L20`
- **Rust crate references:** `crm_customer_privacy_owner_scope_support`, `crm_identity_resolution`, `crm_identity_resolution_capability_adapter`, `crm_module_sdk`, `crm_proto_contracts`

### [`src/tests.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-privacy-scope-adapter/src/tests.rs) · 290 lines

- **Internal/trait/helpers (9):** `context@L30`, `wire_request@L44`, `query_request@L69`, `assert_invalid_argument@L93`, `publishes_exact_contract_only_coordinate_and_frozen_bounds@L100`, `request_contract_preserves_identity_resolution_integrity_errors@L121`, `compound_cursor_round_trip_is_bound_to_family_lineage_and_page_size@L153`, `response_is_cross_family_reference_only_and_deterministic@L209`, `invalid_lineage_page_size_and_malformed_wire_are_rejected_before_owner_reads@L263`
- **Detected tests (5):** `publishes_exact_contract_only_coordinate_and_frozen_bounds@L100`, `request_contract_preserves_identity_resolution_integrity_errors@L121`, `compound_cursor_round_trip_is_bound_to_family_lineage_and_page_size@L153`, `response_is_cross_family_reference_only_and_deterministic@L209`, `invalid_lineage_page_size_and_malformed_wire_are_rejected_before_owner_reads@L263`
- **Error/guard literals:** `PRIVACY_ERASURE_SCOPE`, `IDENTITY_RESOLUTION_PRIVACY_SCOPE_REQUEST_BINDING_MISMATCH`, `IDENTITY_RESOLUTION_PRIVACY_SCOPE_INPUT_HASH_MISMATCH`, `IDENTITY_RESOLUTION_PRIVACY_SCOPE_INPUT_CONTRACT_MISMATCH`, `IDENTITY_RESOLUTION_PRIVACY_SCOPE_CURSOR_INVALID`, `IDENTITY_RESOLUTION_PRIVACY_SCOPE_TENANT_MISMATCH`, `IDENTITY_RESOLUTION_PRIVACY_SCOPE_PAGE_SIZE_INVALID`, `IDENTITY_RESOLUTION_PRIVACY_SCOPE_REQUEST_INVALID`
- **Rust crate references:** `crm_customer_privacy`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`, `crm_identity_resolution`, `crm_identity_resolution_capability_adapter`

### [`tests/postgres_scope.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-privacy-scope-adapter/tests/postgres_scope.rs) · 493 lines

- **Internal/trait/helpers (5):** `identity_resolution_scope_is_alias_aware_complete_reference_only_and_side_effect_free@L30`, `execute_page@L429`, `decode@L445`, `candidate_id@L452`, `assert_response_omits_private_identity_state@L464`
- **Detected tests (1):** `identity_resolution_scope_is_alias_aware_complete_reference_only_and_side_effect_free@L30`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`, `IDENTITY_RESOLUTION_PRIVACY_SCOPE_LINEAGE_INVALID`, `IDENTITY_RESOLUTION_PRIVACY_SCOPE_CURSOR_INVALID`, `IDENTITY_RESOLUTION_PRIVACY_SCOPE_CANDIDATE_STATE_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_identity_resolution`, `crm_identity_resolution_capability_adapter`, `crm_identity_resolution_privacy_scope_adapter`, `crm_parties_capability_adapter`, `crm_proto_contracts`, `crm_query_runtime`

### [`tests/postgres_scope/support.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-privacy-scope-adapter/tests/postgres_scope/support.rs) · 456 lines

- **Public declarations (8):** `create_party@L39`, `register_candidate@L70`, `merge_party@L121`, `unmerge_party@L173`, `scope_request@L255`, `current_generation@L312`, `write_surface_counts@L347`, `corrupt_candidate_metadata@L388`
- **Internal/trait/helpers (1):** `capability_request@L209`
- **Public types:** `WriteSurfaceCounts@L336`
- **Error/guard literals:** `PRIVACY_ERASURE_SCOPE`
- **Rust crate references:** `crm_capability_runtime`, `crm_customer_privacy`, `crm_identity_resolution_capability_adapter`, `crm_identity_resolution_privacy_scope_adapter`, `crm_module_sdk`, `crm_parties_capability_adapter`, `crm_proto_contracts`, `crm_query_runtime`, `crm_parties`, `crm_identity_resolution`

## 70. `crm-identity-resolution-query-adapter` — LATER / B-LATER + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-query-adapter/src/lib.rs) · 681 lines

- **Public declarations (3):** `new@L66`, `query_capability_definitions@L86`, `query_capability_definition@L93`
- **Internal/trait/helpers (30):** `fmt@L54`, `validate@L126`, `execute@L160`, `execute_get@L177`, `execute_list_by_party@L218`, `collect_candidates@L248`, `has_more_visible_candidate@L316`, `related_query@L358`, `validate_list@L379`, `candidate_to_wire_with_visibility@L395`, `required_party_ref@L420`, `optional_status@L430`, `status_wire_value@L449`, `cursor_binding@L462`, `decode_after@L484`, `encode_next@L503`, `decode_input@L524`, `validate_record_id@L555`, `configured_module_id@L560`, `configured_record_type@L564`, `configured_relationship_type@L568`, `enforce_visibility_scan_limit@L572`, `resource_not_found@L580`, `visibility_scan_limit_error@L589`, `cursor_invalid@L598`, `cursor_error@L607`, `config_error@L622`, `unsupported_query@L632`, `publishes_exact_get_and_list_by_party_coordinates@L646`, `authoritative_party_access_path_coordinates_are_stable@L666`
- **Public types:** `IdentityResolutionQueryAdapter@L46`
- **Detected tests (2):** `publishes_exact_get_and_list_by_party_coordinates@L646`, `authoritative_party_access_path_coordinates_are_stable@L666`
- **Error/guard literals:** `IDENTITY_RESOLUTION_QUERY_INPUT_CONTRACT_MISMATCH`, `IDENTITY_RESOLUTION_QUERY_INPUT_PROTOBUF_INVALID`, `QUERY_RESOURCE_NOT_FOUND`, `IDENTITY_RESOLUTION_QUERY_SCAN_LIMIT_EXCEEDED`, `IDENTITY_RESOLUTION_QUERY_CURSOR_INVALID`, `IDENTITY_RESOLUTION_QUERY_CONFIGURATION_INVALID`, `IDENTITY_RESOLUTION_QUERY_CAPABILITY_UNSUPPORTED`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_identity_resolution`, `crm_identity_resolution_capability_adapter`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`

## 71. `crm-identity-resolution-topology-composition` — LATER / B-LATER + D? adapter

### [`src/current_canonical.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-topology-composition/src/current_canonical.rs) · 69 lines

- **Public declarations (1):** `require_current_canonical_party_in_transaction@L13`
- **Internal/trait/helpers (2):** `topology_generation_unavailable@L50`, `topology_generation_invalid@L60`
- **Error/guard literals:** `IDENTITY_RESOLUTION_TOPOLOGY_GENERATION_UNAVAILABLE`, `IDENTITY_RESOLUTION_TOPOLOGY_GENERATION_INVALID`
- **Rust crate references:** `crm_identity_resolution`, `crm_module_sdk`

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-topology-composition/src/lib.rs) · 555 lines

- **Public declarations (2):** `prove_canonical_party_in_transaction@L139`, `prove_canonical_party_in_snapshot_transaction@L160`
- **Internal/trait/helpers (17):** `transaction_proof_mode@L178`, `prove_canonical_party@L194`, `acquire_topology_lock@L258`, `current_generation@L270`, `require_party@L285`, `immediate_redirect_target@L332`, `active_operation_for_edge@L366`, `decode_operation_snapshot@L417`, `stale_generation@L478`, `canonical_party_mismatch@L487`, `canonical_redirect_invalid@L496`, `topology_store_unavailable@L506`, `party_reference_unavailable@L516`, `party_reference_state_invalid@L525`, `merge_operation_state_invalid@L535`, `operation_row_invalid@L542`, `configuration_error@L546`
- **Public types:** `CanonicalPartyTopologyProof@L120`
- **Error/guard literals:** `IDENTITY_RESOLUTION_TOPOLOGY_GENERATION_STALE`, `IDENTITY_RESOLUTION_CANONICAL_PARTY_MISMATCH`, `IDENTITY_RESOLUTION_CANONICAL_REDIRECT_INVALID`, `IDENTITY_RESOLUTION_TOPOLOGY_STORE_UNAVAILABLE`, `PARTY_REFERENCE_UNAVAILABLE`, `PARTY_REFERENCE_STATE_INVALID`, `IDENTITY_RESOLUTION_TOPOLOGY_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_identity_resolution`, `crm_identity_resolution_capability_adapter`, `crm_module_sdk`, `crm_parties_capability_adapter`, `crm_party_reference_composition`

### [`src/root.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-topology-composition/src/root.rs) · 9 lines

- **Re-exports:** `pub use current_canonical::*;`, `pub use topology::*;`

### [`tests/migration_contract.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-identity-resolution-topology-composition/tests/migration_contract.rs) · 76 lines

- **Internal/trait/helpers (3):** `generation_is_monotonic_tenant_bound_and_transaction_linked@L20`, `shared_topology_and_subject_locks_are_fail_fast@L51`, `rollback_restores_the_pre_generation_redirect_guard@L62`
- **Detected tests (3):** `generation_is_monotonic_tenant_bound_and_transaction_linked@L20`, `shared_topology_and_subject_locks_are_fail_fast@L51`, `rollback_restores_the_pre_generation_redirect_guard@L62`

## 72. `crm-metadata-api-adapter` — END / C?/D?

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-metadata-api-adapter/src/lib.rs) · 520 lines

- **Public declarations (15):** `protobuf_contract@L67`, `protobuf_payload@L83`, `decode_request@L113`, `metadata_capability_definition@L145`, `metadata_mutation_capability_definitions@L211`, `metadata_query_capability_definitions@L218`, `decode_publish_bundle@L225`, `publish_bundle_from_wire@L231`, `definition_input_to_document@L251`, `parse_revision_id@L285`, `key_to_wire@L299`, `document_to_wire@L306`, `revision_to_wire@L315`, `impact_to_wire@L325`, `activation_state_to_wire@L347`
- **Internal/trait/helpers (12):** `kind_to_wire@L359`, `change_type_to_wire@L372`, `severity_to_wire@L380`, `configured@L388`, `unsupported_coordinate@L400`, `invalid_revision_id@L409`, `object_definition_json@L423`, `publishes_three_mutations_and_three_queries_with_exact_coordinates@L431`, `strict_authoring_input_is_validated_and_canonicalized_server_side@L463`, `unknown_authoring_fields_are_rejected_before_document_construction@L482`, `caller_cannot_choose_a_different_definition_schema_version@L495`, `revision_identity_parser_requires_exact_lower_or_upper_hex_bytes@L506`
- **Detected tests (5):** `publishes_three_mutations_and_three_queries_with_exact_coordinates@L431`, `strict_authoring_input_is_validated_and_canonicalized_server_side@L463`, `unknown_authoring_fields_are_rejected_before_document_construction@L482`, `caller_cannot_choose_a_different_definition_schema_version@L495`, `revision_identity_parser_requires_exact_lower_or_upper_hex_bytes@L506`
- **Error/guard literals:** `METADATA_PROTOBUF_PAYLOAD_TOO_LARGE`, `METADATA_CAPABILITY_INPUT_CONTRACT_MISMATCH`, `METADATA_CAPABILITY_INPUT_PROTOBUF_INVALID`, `METADATA_BUNDLE_INVALID`, `METADATA_DEFINITION_SCHEMA_VERSION_UNSUPPORTED`, `METADATA_DEFINITION_JSON_INVALID`, `METADATA_DEFINITION_INVALID`, `METADATA_API_CONFIGURATION_INVALID`, `METADATA_API_COORDINATE_UNSUPPORTED`, `METADATA_REVISION_ID_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_metadata_runtime`, `crm_metadata_schema`, `crm_module_sdk`, `crm_proto_contracts`

## 73. `crm-metadata-query-adapter` — END / C?/D?

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-metadata-query-adapter/src/lib.rs) · 439 lines

- **Public declarations (1):** `new@L65`
- **Internal/trait/helpers (18):** `impact_for@L32`, `revision@L38`, `tenant_state@L44`, `fmt@L56`, `validate@L71`, `execute@L99`, `validated_route@L167`, `decode_query_input@L214`, `revision_not_found@L245`, `configuration_error@L254`, `impact_for@L280`, `revision@L292`, `tenant_state@L304`, `fixture@L315`, `request@L351`, `revision_query_executes_from_query_context_without_mutation_identifiers@L380`, `activation_query_returns_tenant_scoped_snapshot@L402`, `unknown_or_mutation_coordinate_cannot_enter_metadata_query_router@L422`
- **Public types:** `MetadataQueryStore@L31`, `MetadataQueryAdapter@L51`
- **Detected tests (3):** `revision_query_executes_from_query_context_without_mutation_identifiers@L380`, `activation_query_returns_tenant_scoped_snapshot@L402`, `unknown_or_mutation_coordinate_cannot_enter_metadata_query_router@L422`
- **Error/guard literals:** `METADATA_QUERY_ROUTE_UNSUPPORTED`, `METADATA_QUERY_DEFINITION_MISMATCH`, `METADATA_QUERY_REQUEST_BINDING_MISMATCH`, `METADATA_QUERY_INPUT_CONTRACT_MISMATCH`, `METADATA_QUERY_INPUT_PROTOBUF_INVALID`, `METADATA_REVISION_NOT_FOUND`
- **Rust crate references:** `crm_capability_runtime`, `crm_metadata_api_adapter`, `crm_metadata_runtime`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`

## 74. `crm-metadata-runtime` — END / C?/D?

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-metadata-runtime/src/lib.rs) · 947 lines

- **Public declarations (29):** `as_str@L35`, `try_new@L72`, `as_str@L96`, `new@L114`, `kind@L118`, `id@L122`, `new@L142`, `key@L189`, `schema_version@L193`, `canonical_content@L197`, `dependencies@L201`, `new@L212`, `documents@L247`, `revision_id@L251`, `from_bytes@L264`, `as_bytes@L268`, `to_hex@L272`, `revision_id@L295`, `documents@L299`, `published_at_unix_millis@L303`, `has_breaking_changes@L343`, `requires_review@L349`, `new@L393`, `publish@L397`, `revision@L423`, `tenant_state@L436`, `impact_for@L445`, `activate@L498`, `rollback@L562`
- **Internal/trait/helpers (21):** `canonical_tag@L48`, `fmt@L63`, `fmt@L102`, `fmt@L128`, `into_documents@L255`, `fmt@L282`, `new@L645`, `fmt@L655`, `metadata_revision_id@L666`, `hash_key@L685`, `hash_bytes@L690`, `hash_usize@L695`, `key@L704`, `document@L708`, `bundle@L723`, `revision_identity_is_deterministic_and_order_independent@L728`, `publication_is_content_addressed_immutable_and_idempotent@L744`, `invalid_bundle_dependencies_are_rejected@L788`, `impact_analysis_and_activation_require_explicit_breaking_confirmation@L817`, `rollback_restores_previous_revision_without_mutating_publications@L885`, `tenant_activation_state_is_isolated@L925`
- **Public types:** `MetadataKind@L23`, `MetadataId@L69`, `MetadataKey@L108`, `MetadataDocument@L134`, `MetadataBundleDraft@L207`, `MetadataRevisionId@L261`, `PublishedMetadataRevision@L288`, `PublishResult@L309`, `MetadataChangeType@L315`, `MetadataImpactSeverity@L322`, `MetadataChange@L329`, `MetadataImpactReport@L336`, `TenantMetadataSnapshot@L357`, `ActivationResult@L364`, `RollbackResult@L373`, `MetadataCatalog@L387`, `MetadataErrorCode@L621`, `MetadataError@L638`
- **Detected tests (6):** `revision_identity_is_deterministic_and_order_independent@L728`, `publication_is_content_addressed_immutable_and_idempotent@L744`, `invalid_bundle_dependencies_are_rejected@L788`, `impact_analysis_and_activation_require_explicit_breaking_confirmation@L817`, `rollback_restores_previous_revision_without_mutating_publications@L885`, `tenant_activation_state_is_isolated@L925`
- **Rust crate references:** `crm_module_sdk`

### [`src/root.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-metadata-runtime/src/root.rs) · 22 lines

- **Re-exports:** `pub use scoped::{`, `pub use tenant::TenantMetadataCatalog;`
- **Rust crate references:** `crm_module_sdk`

### [`src/tenant.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-metadata-runtime/src/tenant.rs) · 211 lines

- **Public declarations (7):** `new@L21`, `publish@L25`, `revision@L37`, `tenant_state@L47`, `impact_for@L54`, `activate@L65`, `rollback@L88`
- **Internal/trait/helpers (7):** `empty_catalog@L100`, `tenant@L109`, `document@L113`, `bundle@L126`, `publication_authority_is_tenant_scoped@L131`, `identical_content_keeps_identity_without_sharing_authority@L164`, `activation_and_rollback_histories_remain_isolated@L183`
- **Public types:** `TenantMetadataCatalog@L16`
- **Detected tests (3):** `publication_authority_is_tenant_scoped@L131`, `identical_content_keeps_identity_without_sharing_authority@L164`, `activation_and_rollback_histories_remain_isolated@L183`
- **Rust crate references:** `crm_module_sdk`

## 75. `crm-metadata-schema` — END / C?/D?

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-metadata-schema/src/lib.rs) · 1160 lines

- **Public declarations (6):** `kind@L43`, `id@L56`, `validate@L69`, `dependencies@L83`, `canonical_bytes@L181`, `to_document@L192`
- **Internal/trait/helpers (40):** `normalized@L205`, `validate@L232`, `validate@L256`, `validate@L277`, `validate@L295`, `validate@L315`, `validate@L339`, `validate@L377`, `validate@L397`, `validate@L423`, `validate@L474`, `validate@L538`, `validate@L599`, `as_str@L640`, `validate@L662`, `validate@L696`, `validate@L712`, `new@L748`, `fmt@L758`, `metadata_id@L769`, `metadata_key@L775`, `validate_label@L779`, `validate_optional_description@L793`, `validate_local_id@L806`, `validate_bounded_collection@L826`, `validate_unique_text@L837`, `validate_exact_version@L860`, `duplicate@L867`, `runtime_contract_error@L875`, `module_id@L888`, `capability_id@L892`, `capability_version@L896`, `object@L900`, `field@L911`, `all_eight_definition_kinds_produce_documents@L924`, `set_like_members_are_canonicalized_independently_of_input_order@L1004`, `meaningful_layout_order_is_preserved_in_canonical_bytes@L1053`, `invalid_bounds_duplicates_and_intra_definition_references_fail_typed_validation@L1090`, `dependencies_feed_bundle_level_missing_dependency_validation@L1143`, `workflow_actions_reject_arbitrary_script_fields@L1150`
- **Public types:** `MetadataDefinition@L31`, `ObjectDefinition@L221`, `FieldDefinition@L243`, `FieldType@L265`, `TextFieldConfig@L290`, `DecimalFieldConfig@L309`, `EnumFieldConfig@L334`, `EnumOption@L365`, `ReferenceFieldConfig@L372`, `RelationshipDefinition@L388`, `RelationshipCardinality@L407`, `LayoutDefinition@L415`, `LayoutSection@L456`, `ViewDefinition@L464`, `ViewSort@L515`, `SortDirection@L522`, `PipelineDefinition@L529`, `PipelineStage@L579`, `PermissionDefinition@L589`, `PermissionAction@L630`, `WorkflowDefinition@L654`, `WorkflowTrigger@L690`, `CapabilityReference@L706`, `WorkflowAction@L722`, `SchemaErrorCode@L729`, `SchemaError@L741`
- **Detected tests (6):** `all_eight_definition_kinds_produce_documents@L924`, `set_like_members_are_canonicalized_independently_of_input_order@L1004`, `meaningful_layout_order_is_preserved_in_canonical_bytes@L1053`, `invalid_bounds_duplicates_and_intra_definition_references_fail_typed_validation@L1090`, `dependencies_feed_bundle_level_missing_dependency_validation@L1143`, `workflow_actions_reject_arbitrary_script_fields@L1150`
- **Rust crate references:** `crm_metadata_runtime`, `crm_module_sdk`

## 76. `crm-module-manifest` — RETIRE-REVIEW / D? + B? invariants

### [`src/bin/validate-module-manifest.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-module-manifest/src/bin/validate-module-manifest.rs) · 64 lines

- **Internal/trait/helpers (2):** `validate_path@L9`, `main@L43`
- **Rust crate references:** `crm_module_manifest`

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-module-manifest/src/lib.rs) · 1252 lines

- **Public declarations (10):** `from_normalized_json@L287`, `validate_or_error@L293`, `validate@L302`, `canonical_json_bytes@L556`, `identity@L564`, `build@L587`, `manifest@L649`, `capability_owner@L653`, `event_owner@L659`, `object_owner@L665`
- **Internal/trait/helpers (32):** `new@L229`, `fmt@L246`, `source@L266`, `from@L275`, `register_contract_owners@L670`, `required_dependency_cycle_issues@L694`, `visit_dependency_node@L730`, `validate_dependencies@L765`, `validate_dependency_list@L826`, `validate_contracts@L858`, `validate_namespaced_list@L881`, `validate_unique_strings@L888`, `duplicates@L901`, `validate_ascii_id@L912`, `validate_module_id@L922`, `validate_namespaced_id@L932`, `validate_semver@L942`, `parse_semver@L946`, `valid_ascii_id@L960`, `valid_segmented_id@L969`, `valid_contact@L988`, `valid_codeowner@L996`, `valid_path@L1011`, `valid_sha256_reference@L1020`, `valid_network_target@L1029`, `write_canonical_value@L1064`, `parses_and_matches_authoring_digest@L1163`, `canonical_identity_ignores_json_object_input_order@L1175`, `rejects_unknown_fields@L1184`, `rejects_self_dependency@L1197`, `catalog_rejects_duplicate_object_ownership@L1212`, `catalog_rejects_required_dependency_cycle@L1225`
- **Public types:** `ModuleManifest@L24`, `Owner@L47`, `Runtime@L56`, `RuntimeKind@L66`, `PlatformCompatibility@L74`, `Dependencies@L82`, `ModuleDependency@L90`, `Provides@L97`, `Consumes@L106`, `VersionedContract@L113`, `UiExtension@L120`, `UiExtensionType@L130`, `UiFallback@L151`, `Storage@L159`, `Security@L166`, `DataClass@L174`, `Lifecycle@L188`, `UpgradePolicy@L199`, `RollbackPolicy@L207`, `UninstallPolicy@L215`, `ValidationIssue@L222`, `ManifestError@L239`, `ManifestIdentity@L281`, `ManifestCatalog@L579`
- **Detected tests (6):** `parses_and_matches_authoring_digest@L1163`, `canonical_identity_ignores_json_object_input_order@L1175`, `rejects_unknown_fields@L1184`, `rejects_self_dependency@L1197`, `catalog_rejects_duplicate_object_ownership@L1212`, `catalog_rejects_required_dependency_cycle@L1225`

## 77. `crm-module-registry` — RETIRE-REVIEW / D? + B? invariants

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-module-registry/src/lib.rs) · 288 lines

- **Internal/trait/helpers (9):** `manifest@L21`, `coordinate@L63`, `grants@L70`, `resolver_selects_highest_compatible_version_in_dependency_order@L84`, `activation_requires_active_dependencies@L123`, `publication_is_immutable_and_idempotent@L157`, `registry_upgrade_and_rollback_restore_previous_version@L174`, `installed_dependents_block_uninstall@L219`, `dependency_cycles_are_rejected@L263`
- **Re-exports:** `pub use registry::*;`, `pub use state::*;`
- **Detected tests (6):** `resolver_selects_highest_compatible_version_in_dependency_order@L84`, `activation_requires_active_dependencies@L123`, `publication_is_immutable_and_idempotent@L157`, `registry_upgrade_and_rollback_restore_previous_version@L174`, `installed_dependents_block_uninstall@L219`, `dependency_cycles_are_rejected@L263`
- **Rust crate references:** `crm_module_manifest`, `crm_module_sdk`

### [`src/registry.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-module-registry/src/registry.rs) · 813 lines

- **Public declarations (16):** `new@L119`, `publish@L128`, `published_version@L172`, `installation@L181`, `resolve_dependencies@L190`, `install@L203`, `activate@L253`, `suspend@L266`, `begin_upgrade@L286`, `complete_upgrade@L314`, `fail_operation@L326`, `begin_rollback@L339`, `complete_rollback@L351`, `uninstall_impact@L363`, `begin_uninstall@L419`, `complete_uninstall@L439`
- **Internal/trait/helpers (19):** `new@L37`, `blocked@L45`, `fmt@L55`, `from@L63`, `from@L69`, `from@L75`, `resolve_coordinate@L470`, `select_dependency@L556`, `ensure_manifest_conflicts_absent@L595`, `ensure_published_and_compatible@L624`, `is_platform_compatible@L638`, `ensure_required_dependencies_active@L667`, `ensure_required_dependencies_active_for_coordinate@L681`, `active_dependents@L720`, `installed_dependents@L728`, `dependents@L736`, `installation_mut@L768`, `parse_version@L793`, `parse_requirement@L802`
- **Public types:** `RegistryErrorCode@L12`, `RegistryError@L30`, `PublishedModuleVersion@L81`, `DependencyResolution@L89`, `ImpactReport@L96`, `PublishOutcome@L104`, `ModuleRegistry@L111`
- **Rust crate references:** `crm_module_manifest`, `crm_module_sdk`

### [`src/state.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-module-registry/src/state.rs) · 428 lines

- **Public declarations (13):** `new@L13`, `is_stable@L43`, `stable@L47`, `installed@L115`, `activate@L137`, `suspend@L154`, `begin_upgrade@L171`, `complete_upgrade@L204`, `fail@L230`, `begin_rollback@L251`, `complete_rollback@L285`, `begin_uninstall@L312`, `ensure_uninstalling@L332`
- **Internal/trait/helpers (12):** `fmt@L19`, `from@L58`, `new@L83`, `fmt@L92`, `ensure_generation@L340`, `invalid_transition@L354`, `bump@L361`, `coordinate@L371`, `installation@L378`, `generation_protects_transitions@L389`, `upgrade_and_rollback_preserve_versions@L396`, `active_installation_cannot_be_uninstalled_directly@L419`
- **Public types:** `ModuleCoordinate@L7`, `StableInstallationStatus@L25`, `InstallationStatus@L32`, `TransitionErrorCode@L68`, `TransitionError@L77`, `ModuleInstallation@L100`
- **Detected tests (3):** `generation_protects_transitions@L389`, `upgrade_and_rollback_preserve_versions@L396`, `active_installation_cannot_be_uninstalled_directly@L419`
- **Rust crate references:** `crm_module_sdk`

## 78. `crm-parties-capability-adapter` — NOW / A? + B-NOW-if-gap + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-parties-capability-adapter/src/lib.rs) · 9 lines

- **Re-exports:** `pub use planner::*;`

### [`src/planner.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-parties-capability-adapter/src/planner.rs) · 445 lines

- **Public declarations (6):** `capability_definitions@L34`, `capability_definition@L41`, `party_to_wire@L292`, `persisted_contract@L314`, `persisted_payload@L325`, `party_from_snapshot@L333`
- **Internal/trait/helpers (12):** `target@L74`, `plan@L118`, `plan_create@L133`, `plan_update@L198`, `mutation_plan@L263`, `party_id_from_ref@L349`, `party_kind_from_wire@L358`, `ensure_definition@L369`, `invalid_plan@L382`, `unsupported_capability@L391`, `publishes_create_and_update_as_personal_idempotent_mutations@L405`, `wire_mapping_preserves_typed_kind_and_version_metadata@L427`
- **Public types:** `PartyCapabilityPlanner@L32`
- **Detected tests (2):** `publishes_create_and_update_as_personal_idempotent_mutations@L405`, `wire_mapping_preserves_typed_kind_and_version_metadata@L427`
- **Error/guard literals:** `PARTIES_PERSISTED_PARTY_IDENTITY_INVALID`, `PARTIES_CAPABILITY_PLAN_INVALID`, `PARTIES_CAPABILITY_UNSUPPORTED`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_module_sdk`, `crm_parties`, `crm_proto_contracts`

### [`tests/postgres_evidence_rollback.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-parties-capability-adapter/tests/postgres_evidence_rollback.rs) · 322 lines

- **Internal/trait/helpers (12):** `target@L48`, `plan@L56`, `validate@L72`, `missing_party_audit_evidence_rolls_back_every_transactional_side_effect@L101`, `compose@L154`, `token_store@L205`, `payload@L221`, `http_request@L235`, `insert_header@L267`, `evidence_counts@L274`, `scalar_count@L304`, `party_record_count@L312`
- **Detected tests (1):** `missing_party_audit_evidence_rolls_back_every_transactional_side_effect@L101`
- **Error/guard literals:** `CAPABILITY_INPUT_HASH_INVALID`, `DATABASE_URL`, `ADMIN_DATABASE_URL`, `CAPABILITY_EXECUTION_PLAN_INVALID`
- **Rust crate references:** `crm_capability_adapters`, `crm_capability_ingress`, `crm_capability_runtime`, `crm_core_data`, `crm_module_sdk`, `crm_parties_capability_adapter`, `crm_proto_contracts`

## 79. `crm-parties-privacy-scope-adapter` — END / B-END + D? adapter

### [`src/contract.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-parties-privacy-scope-adapter/src/contract.rs) · 93 lines

- **Public declarations (8):** `parties_privacy_scope_definition@L23`, `validate_definition@L43`, `module_id@L54`, `capability_id@L58`, `capability_version@L62`, `schema_id@L66`, `schema_version@L70`, `output_descriptor_hash@L74`
- **Internal/trait/helpers (1):** `payload_contract@L78`
- **Error/guard literals:** `PARTIES_PRIVACY_SCOPE_DEFINITION_MISMATCH`
- **Rust crate references:** `crm_capability_runtime`, `crm_module_sdk`, `crm_parties`, `crm_proto_contracts`

### [`src/errors.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-parties-privacy-scope-adapter/src/errors.rs) · 73 lines

- **Public declarations (8):** `configured@L3`, `lineage_invalid@L15`, `subject_not_found@L29`, `stored_state_invalid@L38`, `row_decode_error@L48`, `database_unavailable@L52`, `invalid_contract@L62`, `invalid_contract_with_reference@L66`
- **Error/guard literals:** `PARTIES_PRIVACY_SCOPE_CONFIGURATION_INVALID`, `PARTIES_PRIVACY_SCOPE_LINEAGE_INVALID`, `PARTIES_PRIVACY_SCOPE_SUBJECT_NOT_FOUND`, `PARTIES_PRIVACY_SCOPE_STORED_STATE_INVALID`, `PARTIES_PRIVACY_SCOPE_UNAVAILABLE`
- **Rust crate references:** `crm_module_sdk`

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-parties-privacy-scope-adapter/src/lib.rs) · 30 lines

- **Re-exports:** `pub use contract::{`, `pub use owner_action::{`, `pub use postgres::PartiesPrivacyScopeQueryAdapter;`

### [`src/owner_action.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-parties-privacy-scope-adapter/src/owner_action.rs) · 149 lines

- **Public declarations (2):** `parties_privacy_action_definition@L17`, `parties_privacy_action_planner@L21`
- **Internal/trait/helpers (12):** `owner_module_id@L26`, `capability_id@L30`, `supports_resource_type@L34`, `anonymize@L38`, `deletion_tombstone@L46`, `minimize_party@L55`, `minimized_display_name@L71`, `hex_prefix@L85`, `party@L99`, `publishes_the_frozen_owner_action_coordinate@L110`, `minimized_labels_are_deterministic_kind_aware_and_non_identifying@L122`, `minimized_label_cannot_be_a_semantic_no_op@L141`
- **Public types:** `PartiesPrivacyActionPlanner@L12`, `PartiesPrivacyActionPolicy@L15`
- **Detected tests (3):** `publishes_the_frozen_owner_action_coordinate@L110`, `minimized_labels_are_deterministic_kind_aware_and_non_identifying@L122`, `minimized_label_cannot_be_a_semantic_no_op@L141`
- **Rust crate references:** `crm_capability_runtime`, `crm_customer_privacy`, `crm_customer_privacy_owner_scope_support`, `crm_module_sdk`, `crm_parties`, `crm_parties_capability_adapter`

### [`src/postgres.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-parties-privacy-scope-adapter/src/postgres.rs) · 211 lines

- **Public declarations (1):** `new@L41`
- **Internal/trait/helpers (6):** `fmt@L32`, `execute_query@L45`, `execute@L115`, `map_canonical_claim_error@L124`, `strict_party_snapshot@L150`, `configured@L208`
- **Public types:** `PartiesPrivacyScopeQueryAdapter@L27`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_privacy_owner_scope_support`, `crm_module_sdk`, `crm_parties`, `crm_parties_capability_adapter`, `crm_query_runtime`

### [`src/request.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-parties-privacy-scope-adapter/src/request.rs) · 146 lines

- **Public declarations (2):** `validate_request_contract@L20`, `validate_wire_request@L25`
- **Internal/trait/helpers (2):** `map_request_contract_error@L74`, `map_common_lineage_error@L93`
- **Public types:** `ValidatedRequest@L13`
- **Error/guard literals:** `PARTIES_PRIVACY_SCOPE_REQUEST_INVALID`, `PARTIES_PRIVACY_SCOPE_LINEAGE_INVALID`, `PARTIES_PRIVACY_SCOPE_CURSOR_INVALID`, `PARTIES_PRIVACY_SCOPE_REQUEST_BINDING_MISMATCH`, `PARTIES_PRIVACY_SCOPE_INPUT_CONTRACT_MISMATCH`, `PARTIES_PRIVACY_SCOPE_INPUT_HASH_MISMATCH`, `PARTIES_PRIVACY_SCOPE_TENANT_MISMATCH`, `PARTIES_PRIVACY_SCOPE_CASE_ID_INVALID`, `PARTIES_PRIVACY_SCOPE_PARTY_INVALID`, `PARTIES_PRIVACY_SCOPE_GENERATION_INVALID`, `PARTIES_PRIVACY_SCOPE_REGISTRY_INVALID`, `PARTIES_PRIVACY_SCOPE_REGISTRY_UNAVAILABLE`, `PARTIES_PRIVACY_SCOPE_REGISTRY_MISMATCH`, `PARTIES_PRIVACY_SCOPE_PURPOSE_INVALID`, `PARTIES_PRIVACY_SCOPE_REQUEST_TIME_INVALID`, `PARTIES_PRIVACY_SCOPE_PAGE_SIZE_INVALID`
- **Rust crate references:** `crm_customer_privacy_owner_scope_support`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`

### [`src/response.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-parties-privacy-scope-adapter/src/response.rs) · 87 lines

- **Public declarations (2):** `build_response@L14`, `typed_output@L72`
- **Rust crate references:** `crm_customer_privacy_owner_scope_support`, `crm_module_sdk`, `crm_parties`, `crm_parties_capability_adapter`, `crm_proto_contracts`

### [`src/tests.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-parties-privacy-scope-adapter/src/tests.rs) · 259 lines

- **Internal/trait/helpers (9):** `context@L23`, `valid_wire_request@L37`, `query_request@L59`, `assert_invalid_argument@L81`, `definition_is_internal_read_only_and_exactly_bound@L88`, `request_contract_preserves_parties_specific_integrity_errors@L112`, `wire_validation_defaults_page_size_and_preserves_parties_error_mapping@L144`, `response_is_reference_only_terminal_and_deterministic@L200`, `non_empty_cursor_future_time_and_invalid_purpose_fail_closed@L223`
- **Detected tests (5):** `definition_is_internal_read_only_and_exactly_bound@L88`, `request_contract_preserves_parties_specific_integrity_errors@L112`, `wire_validation_defaults_page_size_and_preserves_parties_error_mapping@L144`, `response_is_reference_only_terminal_and_deterministic@L200`, `non_empty_cursor_future_time_and_invalid_purpose_fail_closed@L223`
- **Error/guard literals:** `PRIVACY_ERASURE_SCOPE`, `PARTIES_PRIVACY_SCOPE_REQUEST_BINDING_MISMATCH`, `PARTIES_PRIVACY_SCOPE_INPUT_HASH_MISMATCH`, `PARTIES_PRIVACY_SCOPE_INPUT_CONTRACT_MISMATCH`, `PARTIES_PRIVACY_SCOPE_REGISTRY_MISMATCH`, `PARTIES_PRIVACY_SCOPE_TENANT_MISMATCH`, `PARTIES_PRIVACY_SCOPE_GENERATION_INVALID`, `PARTIES_PRIVACY_SCOPE_PAGE_SIZE_INVALID`, `PARTIES_PRIVACY_SCOPE_CURSOR_INVALID`, `PARTIES_PRIVACY_SCOPE_REQUEST_TIME_INVALID`, `PARTIES_PRIVACY_SCOPE_PURPOSE_INVALID`
- **Rust crate references:** `crm_customer_privacy`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`, `crm_parties`

### [`tests/postgres_owner_action.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-parties-privacy-scope-adapter/tests/postgres_owner_action.rs) · 466 lines

- **Internal/trait/helpers (9):** `postgres_owner_action_is_replay_safe_tenant_bound_and_fail_closed@L33`, `attempt@L154`, `capability_request@L241`, `transaction_id@L274`, `seed_tenants@L278`, `seed_actors@L295`, `seed_owner_action_capability@L325`, `seed_party@L349`, `assert_evidence_counts@L420`
- **Detected tests (1):** `postgres_owner_action_is_replay_safe_tenant_bound_and_fail_closed@L33`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`, `PRIVACY_OWNER_RECORD_STALE`, `PRIVACY_OWNER_RECORD_NOT_FOUND`, `ERASURE_DISCOVERY`, `GLOBAL`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_privacy`, `crm_customer_privacy_owner_scope_support`, `crm_module_sdk`, `crm_parties`, `crm_parties_capability_adapter`, `crm_parties_privacy_scope_adapter`

### [`tests/postgres_scope.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-parties-privacy-scope-adapter/tests/postgres_scope.rs) · 353 lines

- **Internal/trait/helpers (6):** `parties_scope_is_tenant_bound_strict_and_side_effect_free@L36`, `corrupt_party_metadata@L141`, `create_party@L189`, `capability_request@L217`, `scope_request@L255`, `write_surface_counts@L317`
- **Detected tests (1):** `parties_scope_is_tenant_bound_strict_and_side_effect_free@L36`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`, `PARTIES_PRIVACY_SCOPE_LINEAGE_INVALID`, `PARTIES_PRIVACY_SCOPE_STORED_STATE_INVALID`, `PRIVACY_ERASURE_SCOPE`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_privacy`, `crm_module_sdk`, `crm_parties_capability_adapter`, `crm_parties_privacy_scope_adapter`, `crm_proto_contracts`, `crm_query_runtime`, `crm_parties`

## 80. `crm-parties-query-adapter` — NOW / A? + B-NOW-if-gap + D? adapter

### [`src/export_execution.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-parties-query-adapter/src/export_execution.rs) · 159 lines

- **Public declarations (2):** `export_execution_query_request@L34`, `get_for_export_execution@L75`
- **Internal/trait/helpers (2):** `config_error@L127`, `exact_get_request_is_bound_to_party_identity@L142`
- **Public types:** `PartyExportExecutionKind@L15`, `PartyExportExecutionRead@L21`
- **Detected tests (1):** `exact_get_request_is_bound_to_party_identity@L142`
- **Error/guard literals:** `PARTIES_EXPORT_EXECUTION_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_core_data`, `crm_module_sdk`, `crm_parties`, `crm_parties_capability_adapter`, `crm_proto_contracts`, `crm_query_runtime`

### [`src/export_selection.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-parties-query-adapter/src/export_selection.rs) · 223 lines

- **Public declarations (2):** `export_selection_query_request@L41`, `list_for_export_selection@L94`
- **Internal/trait/helpers (4):** `selection_kind_matches@L179`, `selection_config_error@L187`, `export_selection_unavailable@L197`, `kind_filter_mapping_is_exact@L211`
- **Public types:** `PartyExportSelectionKind@L19`, `PartyExportSelectionCandidate@L25`, `PartyExportSelectionPage@L31`
- **Detected tests (1):** `kind_filter_mapping_is_exact@L211`
- **Error/guard literals:** `PARTIES_EXPORT_SELECTION_CONFIGURATION_INVALID`, `PARTIES_EXPORT_SELECTION_UNAVAILABLE`
- **Rust crate references:** `crm_core_data`, `crm_module_sdk`, `crm_parties`, `crm_parties_capability_adapter`, `crm_proto_contracts`, `crm_query_runtime`

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-parties-query-adapter/src/lib.rs) · 642 lines

- **Public declarations (3):** `new@L62`, `query_capability_definitions@L82`, `query_capability_definition@L89`
- **Internal/trait/helpers (27):** `fmt@L50`, `validate@L122`, `execute@L149`, `execute_get@L166`, `execute_list@L201`, `collect_parties@L232`, `has_more_visible_party@L284`, `validate_list@L323`, `party_matches_kind@L361`, `party_to_wire_with_visibility@L370`, `resolve_page_size@L384`, `cursor_binding@L393`, `decode_after@L412`, `encode_next@L441`, `party_filter_hash@L462`, `decode_input@L467`, `validate_record_id@L498`, `party_record_type@L503`, `enforce_scan_limit@L507`, `resource_not_found@L515`, `unsupported_query@L524`, `cursor_error@L533`, `scan_limit_error@L543`, `config_error@L552`, `publishes_get_and_list_as_personal_read_only_queries@L569`, `field_visibility_redacts_personal_fields_without_hiding_resource_identity@L587`, `kind_filter_is_exact_and_cursor_bound@L610`
- **Public types:** `PartyQueryAdapter@L42`
- **Re-exports:** `pub use export_execution::*;`, `pub use export_selection::*;`
- **Detected tests (3):** `publishes_get_and_list_as_personal_read_only_queries@L569`, `field_visibility_redacts_personal_fields_without_hiding_resource_identity@L587`, `kind_filter_is_exact_and_cursor_bound@L610`
- **Error/guard literals:** `PARTIES_QUERY_CURSOR_INVALID`, `PARTIES_QUERY_INPUT_CONTRACT_MISMATCH`, `PARTIES_QUERY_INPUT_PROTOBUF_INVALID`, `QUERY_RESOURCE_NOT_FOUND`, `PARTIES_QUERY_CAPABILITY_UNSUPPORTED`, `PARTIES_QUERY_VISIBILITY_SCAN_LIMIT_EXCEEDED`, `PARTIES_QUERY_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_module_sdk`, `crm_parties`, `crm_parties_capability_adapter`, `crm_proto_contracts`, `crm_query_runtime`

## 81. `crm-party-reference-composition` — NOW / A? + B-NOW-if-gap + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-party-reference-composition/src/lib.rs) · 297 lines

- **Public declarations (7):** `new@L52`, `parties_runtime_identity@L93`, `mutation_capability_definitions@L104`, `query_capability_definitions@L110`, `build_contribution@L116`, `require_party_reference_in_transaction@L191`, `lock_customer_subject_in_transaction@L235`
- **Internal/trait/helpers (9):** `exists@L39`, `exists@L58`, `parties_cursor@L165`, `production_composition_error@L177`, `party_reference_unavailable@L249`, `party_reference_store_unavailable@L258`, `party_reference_state_invalid@L268`, `customer_subject_lock_unavailable@L278`, `configuration_error@L288`
- **Public types:** `PartyReferenceReader@L38`, `PostgresPartyReferenceReader@L47`, `PartiesProductionDependencies@L83`
- **Error/guard literals:** `PARTIES_CURSOR_CONFIGURATION_INVALID`, `PARTIES_PRODUCTION_COMPOSITION_INVALID`, `PARTY_REFERENCE_UNAVAILABLE`, `PARTY_REFERENCE_STORE_UNAVAILABLE`, `PARTY_REFERENCE_STATE_INVALID`, `CUSTOMER_SUBJECT_LOCK_UNAVAILABLE`, `PARTY_REFERENCE_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_application_composition`, `crm_capability_runtime`, `crm_core_data`, `crm_module_sdk`, `crm_parties_capability_adapter`, `crm_parties_query_adapter`, `crm_query_runtime`

## 82. `crm-party-relationships-capability-adapter` — NOW / A? + B-NOW-if-gap + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-party-relationships-capability-adapter/src/lib.rs) · 135 lines

- **Public declarations (2):** `capability_definitions@L38`, `capability_definition@L45`
- **Internal/trait/helpers (4):** `configured@L80`, `configuration_error@L90`, `publishes_exact_create_and_update_coordinates_as_personal_mutations@L99`, `rejects_unknown_party_relationship_mutation_coordinate@L129`
- **Re-exports:** `pub use planner::*;`
- **Detected tests (2):** `publishes_exact_create_and_update_coordinates_as_personal_mutations@L99`, `rejects_unknown_party_relationship_mutation_coordinate@L129`
- **Error/guard literals:** `PARTY_RELATIONSHIPS_CAPABILITY_UNSUPPORTED`, `PARTY_RELATIONSHIPS_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_module_sdk`

### [`src/planner.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-party-relationships-capability-adapter/src/planner.rs) · 515 lines

- **Public declarations (5):** `party_relationship_to_wire@L271`, `persisted_contract@L312`, `persisted_payload@L323`, `party_relationship_from_snapshot@L333`, `referenced_party_ids_from_create@L352`
- **Internal/trait/helpers (16):** `target@L29`, `plan@L81`, `plan_create@L96`, `plan_update@L173`, `mutation_plan@L242`, `relationship_type_to_wire@L367`, `relationship_type_from_wire@L383`, `directionality_from_wire@L400`, `status_from_wire@L417`, `party_relationship_id_from_ref@L428`, `party_reference_from_ref@L438`, `optional_time@L447`, `ensure_definition@L451`, `invalid_plan@L464`, `unsupported_capability@L473`, `wire_mapping_preserves_endpoints_semantics_and_version_metadata@L487`
- **Public types:** `PartyRelationshipCapabilityPlanner@L26`
- **Detected tests (1):** `wire_mapping_preserves_endpoints_semantics_and_version_metadata@L487`
- **Error/guard literals:** `PARTY_RELATIONSHIPS_PERSISTED_RELATIONSHIP_IDENTITY_INVALID`, `PARTY_RELATIONSHIPS_CAPABILITY_PLAN_INVALID`, `PARTY_RELATIONSHIPS_CAPABILITY_UNSUPPORTED`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_data`, `crm_module_sdk`, `crm_party_relationships`, `crm_proto_contracts`

## 83. `crm-party-relationships-capability-composition` — NOW / A? + B-NOW-if-gap + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-party-relationships-capability-composition/src/lib.rs) · 198 lines

- **Public declarations (4):** `new@L47`, `mutation_capability_definitions@L95`, `query_capability_definitions@L101`, `build_contribution@L107`
- **Internal/trait/helpers (6):** `fmt@L38`, `validate@L53`, `party_relationships_cursor@L157`, `production_composition_error@L169`, `reference_unavailable@L179`, `unsupported_capability@L188`
- **Public types:** `PartyRelationshipReferenceSemanticValidator@L33`, `PartyRelationshipsProductionDependencies@L85`
- **Error/guard literals:** `PARTY_RELATIONSHIPS_CURSOR_CONFIGURATION_INVALID`, `PARTY_RELATIONSHIPS_PRODUCTION_COMPOSITION_INVALID`, `PARTY_RELATIONSHIPS_PARTY_REFERENCE_UNAVAILABLE`, `PARTY_RELATIONSHIPS_COMPOSITION_CAPABILITY_UNSUPPORTED`
- **Rust crate references:** `crm_application_composition`, `crm_capability_runtime`, `crm_core_data`, `crm_module_sdk`, `crm_party_reference_composition`, `crm_party_relationships_capability_adapter`, `crm_party_relationships_query_adapter`, `crm_query_runtime`

## 84. `crm-party-relationships-privacy-scope-adapter` — END / B-END + D? adapter

### [`src/contract.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-party-relationships-privacy-scope-adapter/src/contract.rs) · 96 lines

- **Public declarations (8):** `party_relationships_privacy_scope_definition@L26`, `validate_definition@L46`, `module_id@L57`, `capability_id@L61`, `capability_version@L65`, `schema_id@L69`, `schema_version@L73`, `output_descriptor_hash@L77`
- **Internal/trait/helpers (1):** `payload_contract@L81`
- **Error/guard literals:** `PARTY_RELATIONSHIPS_PRIVACY_SCOPE_DEFINITION_MISMATCH`
- **Rust crate references:** `crm_capability_runtime`, `crm_module_sdk`, `crm_party_relationships`, `crm_proto_contracts`

### [`src/digest.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-party-relationships-privacy-scope-adapter/src/digest.rs) · 44 lines

- **Public declarations (2):** `encode_hex@L3`, `decode_hex@L13`
- **Internal/trait/helpers (2):** `nibble@L30`, `cursor_invalid@L38`
- **Error/guard literals:** `PARTY_RELATIONSHIPS_PRIVACY_SCOPE_CURSOR_INVALID`
- **Rust crate references:** `crm_module_sdk`

### [`src/errors.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-party-relationships-privacy-scope-adapter/src/errors.rs) · 166 lines

- **Public declarations (10):** `configured@L6`, `map_request_contract_error@L18`, `map_common_lineage_error@L37`, `map_canonical_party_claim_error@L91`, `lineage_invalid@L117`, `stored_state_invalid@L131`, `row_decode_error@L141`, `database_unavailable@L145`, `invalid_contract@L155`, `invalid_contract_with_reference@L159`
- **Error/guard literals:** `PARTY_RELATIONSHIPS_PRIVACY_SCOPE_CONFIGURATION_INVALID`, `PARTY_RELATIONSHIPS_PRIVACY_SCOPE_REQUEST_BINDING_MISMATCH`, `PARTY_RELATIONSHIPS_PRIVACY_SCOPE_INPUT_CONTRACT_MISMATCH`, `PARTY_RELATIONSHIPS_PRIVACY_SCOPE_INPUT_HASH_MISMATCH`, `PARTY_RELATIONSHIPS_PRIVACY_SCOPE_TENANT_MISMATCH`, `PARTY_RELATIONSHIPS_PRIVACY_SCOPE_CASE_ID_INVALID`, `PARTY_RELATIONSHIPS_PRIVACY_SCOPE_PARTY_INVALID`, `PARTY_RELATIONSHIPS_PRIVACY_SCOPE_GENERATION_INVALID`, `PARTY_RELATIONSHIPS_PRIVACY_SCOPE_REGISTRY_INVALID`, `PARTY_RELATIONSHIPS_PRIVACY_SCOPE_REGISTRY_UNAVAILABLE`, `PARTY_RELATIONSHIPS_PRIVACY_SCOPE_REGISTRY_MISMATCH`, `PARTY_RELATIONSHIPS_PRIVACY_SCOPE_PURPOSE_INVALID`, `PARTY_RELATIONSHIPS_PRIVACY_SCOPE_REQUEST_TIME_INVALID`, `PARTY_RELATIONSHIPS_PRIVACY_SCOPE_PAGE_SIZE_INVALID`, `PARTY_RELATIONSHIPS_PRIVACY_SCOPE_LINEAGE_INVALID`, `PARTY_RELATIONSHIPS_PRIVACY_SCOPE_STORED_STATE_INVALID`, `PARTY_RELATIONSHIPS_PRIVACY_SCOPE_UNAVAILABLE`
- **Rust crate references:** `crm_customer_privacy_owner_scope_support`, `crm_module_sdk`

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-party-relationships-privacy-scope-adapter/src/lib.rs) · 34 lines

- **Re-exports:** `pub use contract::{`, `pub use owner_action::{`, `pub use postgres::PartyRelationshipsPrivacyScopeQueryAdapter;`

### [`src/owner_action.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-party-relationships-privacy-scope-adapter/src/owner_action.rs) · 172 lines

- **Public declarations (2):** `party_relationships_privacy_action_definition@L20`, `party_relationships_privacy_action_planner@L27`
- **Internal/trait/helpers (13):** `owner_module_id@L33`, `capability_id@L37`, `supports_resource_type@L41`, `anonymize@L45`, `deletion_tombstone@L53`, `minimize_relationship@L62`, `privacy_transition@L71`, `transition_invalid@L93`, `relationship@L110`, `publishes_the_frozen_owner_action_coordinate@L125`, `privacy_transition_preserves_topology_and_semantics@L140`, `repeated_privacy_transition_advances_version_without_reintroducing_state@L157`, `privacy_transition_rejects_time_regression@L167`
- **Public types:** `PartyRelationshipsPrivacyActionPlanner@L14`, `PartyRelationshipsPrivacyActionPolicy@L18`
- **Detected tests (4):** `publishes_the_frozen_owner_action_coordinate@L125`, `privacy_transition_preserves_topology_and_semantics@L140`, `repeated_privacy_transition_advances_version_without_reintroducing_state@L157`, `privacy_transition_rejects_time_regression@L167`
- **Error/guard literals:** `PARTY_RELATIONSHIPS_PRIVACY_TRANSITION_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_customer_privacy_owner_scope_support`, `crm_module_sdk`, `crm_party_relationships`, `crm_party_relationships_capability_adapter`

### [`src/postgres.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-party-relationships-privacy-scope-adapter/src/postgres.rs) · 302 lines

- **Public declarations (1):** `new@L44`
- **Internal/trait/helpers (7):** `fmt@L35`, `execute_query@L48`, `execute@L84`, `read_party_relationship_page@L99`, `strict_party_relationship_resource@L218`, `configured@L294`, `_definition_smoke@L299`
- **Public types:** `PartyRelationshipsPrivacyScopeQueryAdapter@L30`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_customer_privacy_owner_scope_support`, `crm_module_sdk`, `crm_party_relationships`, `crm_party_relationships_capability_adapter`, `crm_query_runtime`

### [`src/request.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-party-relationships-privacy-scope-adapter/src/request.rs) · 180 lines

- **Public declarations (3):** `validate_request_contract@L30`, `validate_wire_request@L35`, `encode_cursor@L90`
- **Internal/trait/helpers (3):** `decode_cursor@L113`, `cursor_digest@L147`, `cursor_invalid@L174`
- **Public types:** `ValidatedRequest@L21`
- **Error/guard literals:** `PARTY_RELATIONSHIPS_PRIVACY_SCOPE_REQUEST_INVALID`, `PARTY_RELATIONSHIPS_PRIVACY_SCOPE_LINEAGE_INVALID`, `PARTY_RELATIONSHIPS_PRIVACY_SCOPE_CURSOR_INVALID`
- **Rust crate references:** `crm_customer_privacy_owner_scope_support`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`

### [`src/response.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-party-relationships-privacy-scope-adapter/src/response.rs) · 143 lines

- **Public declarations (2):** `build_response@L23`, `typed_output@L95`
- **Internal/trait/helpers (1):** `page_digest@L111`
- **Public types:** `VerifiedPartyRelationshipResource@L18`
- **Rust crate references:** `crm_customer_privacy_owner_scope_support`, `crm_module_sdk`, `crm_party_relationships`, `crm_party_relationships_capability_adapter`, `crm_proto_contracts`

### [`src/tests.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-party-relationships-privacy-scope-adapter/src/tests.rs) · 240 lines

- **Internal/trait/helpers (10):** `context@L25`, `wire_request@L39`, `query_request@L64`, `assert_invalid_argument@L88`, `publishes_exact_contract_only_coordinate@L95`, `request_contract_preserves_party_relationships_integrity_errors@L109`, `cursor_round_trip_is_bound_to_lineage_and_page_size@L141`, `common_lineage_failures_keep_party_relationships_codes@L173`, `response_is_reference_only_deterministic_and_supports_sparse_progress@L196`, `malformed_wire_request_is_rejected_before_owner_reads@L236`
- **Detected tests (6):** `publishes_exact_contract_only_coordinate@L95`, `request_contract_preserves_party_relationships_integrity_errors@L109`, `cursor_round_trip_is_bound_to_lineage_and_page_size@L141`, `common_lineage_failures_keep_party_relationships_codes@L173`, `response_is_reference_only_deterministic_and_supports_sparse_progress@L196`, `malformed_wire_request_is_rejected_before_owner_reads@L236`
- **Error/guard literals:** `PRIVACY_ERASURE_SCOPE`, `PARTY_RELATIONSHIPS_PRIVACY_SCOPE_REQUEST_BINDING_MISMATCH`, `PARTY_RELATIONSHIPS_PRIVACY_SCOPE_INPUT_HASH_MISMATCH`, `PARTY_RELATIONSHIPS_PRIVACY_SCOPE_INPUT_CONTRACT_MISMATCH`, `PARTY_RELATIONSHIPS_PRIVACY_SCOPE_CURSOR_INVALID`, `PARTY_RELATIONSHIPS_PRIVACY_SCOPE_TENANT_MISMATCH`, `PARTY_RELATIONSHIPS_PRIVACY_SCOPE_PAGE_SIZE_INVALID`, `PARTY_RELATIONSHIPS_PRIVACY_SCOPE_REQUEST_INVALID`
- **Rust crate references:** `crm_customer_privacy`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`, `crm_party_relationships`

### [`tests/postgres_scope.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-party-relationships-privacy-scope-adapter/tests/postgres_scope.rs) · 504 lines

- **Internal/trait/helpers (3):** `party_relationships_scope_is_bounded_strict_tenant_bound_and_side_effect_free@L30`, `assert_response_omits_relationship_state@L480`, `assert_no_query_side_writes@L491`
- **Detected tests (1):** `party_relationships_scope_is_bounded_strict_tenant_bound_and_side_effect_free@L30`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`, `PARTY_RELATIONSHIPS_PRIVACY_SCOPE_CURSOR_INVALID`, `PARTY_RELATIONSHIPS_PRIVACY_SCOPE_LINEAGE_INVALID`, `PARTY_RELATIONSHIPS_PRIVACY_SCOPE_STORED_STATE_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_parties_capability_adapter`, `crm_party_relationships_capability_adapter`, `crm_party_relationships_privacy_scope_adapter`, `crm_proto_contracts`, `crm_query_runtime`, `crm_party_relationships`

### [`tests/postgres_scope/support.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-party-relationships-privacy-scope-adapter/tests/postgres_scope/support.rs) · 461 lines

- **Public declarations (7):** `create_party@L45`, `create_party_relationship@L77`, `update_party_relationship@L131`, `scope_request@L207`, `insert_canonical_redirect@L265`, `corrupt_party_relationship_metadata@L358`, `write_surface_counts@L421`
- **Internal/trait/helpers (1):** `capability_request@L171`
- **Public types:** `WriteSurfaceCounts@L410`
- **Error/guard literals:** `PRIVACY_ERASURE_SCOPE`
- **Rust crate references:** `crm_capability_runtime`, `crm_customer_privacy`, `crm_identity_resolution_capability_adapter`, `crm_module_sdk`, `crm_parties_capability_adapter`, `crm_party_relationships_capability_adapter`, `crm_party_relationships_privacy_scope_adapter`, `crm_proto_contracts`, `crm_query_runtime`, `crm_parties`, `crm_party_relationships`

## 85. `crm-party-relationships-projection` — NOW/NEXT / A? + B-CONDITIONAL

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-party-relationships-projection/src/lib.rs) · 575 lines

- **Public declarations (8):** `party_relationship_hierarchy_projection_registry@L47`, `new@L65`, `run_batch@L74`, `rebuild@L88`, `runner@L98`, `from_json@L121`, `is_active_at@L126`, `traverse_projected_hierarchy@L140`
- **Internal/trait/helpers (18):** `project@L42`, `hierarchy_writes@L178`, `adjacency_write@L294`, `validate_contract@L329`, `decode@L345`, `directionality_name@L353`, `status_name@L363`, `configured_projection_id@L373`, `configured_module_id@L378`, `configured_event_type@L382`, `projection_configuration_invalid@L386`, `projection_event_invalid@L396`, `hierarchy_document_invalid@L406`, `registry_is_stable_and_consumes_only_authoritative_relationship_events@L426`, `directional_event_produces_forward_and_reverse_adjacency_documents@L436`, `bounded_traversal_uses_only_active_effective_projected_edges@L463`, `edge@L490`, `created_delivery@L514`
- **Public types:** `PartyRelationshipHierarchyProjectionWorker@L60`, `HierarchyAdjacencyDocument@L105`
- **Detected tests (3):** `registry_is_stable_and_consumes_only_authoritative_relationship_events@L426`, `directional_event_produces_forward_and_reverse_adjacency_documents@L436`, `bounded_traversal_uses_only_active_effective_projected_edges@L463`
- **Error/guard literals:** `PARTY_RELATIONSHIP_HIERARCHY_PROJECTION_CONFIGURATION_INVALID`, `PARTY_RELATIONSHIP_HIERARCHY_PROJECTION_EVENT_INVALID`, `PARTY_RELATIONSHIP_HIERARCHY_DOCUMENT_INVALID`
- **Rust crate references:** `crm_core_data`, `crm_core_events`, `crm_module_sdk`, `crm_projection_runtime`, `crm_proto_contracts`

## 86. `crm-party-relationships-query-adapter` — NOW / A? + B-NOW-if-gap + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-party-relationships-query-adapter/src/lib.rs) · 817 lines

- **Public declarations (3):** `new@L62`, `query_capability_definitions@L82`, `query_capability_definition@L89`
- **Internal/trait/helpers (34):** `fmt@L50`, `validate@L122`, `execute@L153`, `execute_get@L170`, `execute_list@L211`, `collect_relationships@L243`, `has_more_visible_relationship@L304`, `from_wire@L352`, `validate_list@L377`, `relationship_matches_filters@L408`, `party_relationship_to_wire_with_visibility@L438`, `resolve_page_size@L462`, `cursor_binding@L471`, `decode_after@L490`, `encode_next@L519`, `relationship_filter_hash@L540`, `decode_input@L557`, `validate_record_id@L588`, `validate_party_id@L597`, `validate_type_code_filter@L603`, `directionality_from_wire@L629`, `status_from_wire@L646`, `relationship_record_type@L657`, `enforce_scan_limit@L661`, `resource_not_found@L669`, `unsupported_query@L678`, `cursor_error@L687`, `scan_limit_error@L697`, `config_error@L706`, `relationship@L724`, `publishes_get_and_list_as_personal_read_only_queries@L738`, `type_code_filter_accepts_reserved_reciprocal_codes_without_fabricating_semantics@L756`, `field_visibility_redacts_relationship_data_without_hiding_resource_identity@L763`, `endpoint_type_directionality_and_status_filters_are_exact_and_cursor_bound@L787`
- **Public types:** `PartyRelationshipQueryAdapter@L42`
- **Detected tests (4):** `publishes_get_and_list_as_personal_read_only_queries@L738`, `type_code_filter_accepts_reserved_reciprocal_codes_without_fabricating_semantics@L756`, `field_visibility_redacts_relationship_data_without_hiding_resource_identity@L763`, `endpoint_type_directionality_and_status_filters_are_exact_and_cursor_bound@L787`
- **Error/guard literals:** `PARTY_RELATIONSHIPS_QUERY_CURSOR_INVALID`, `PARTY_RELATIONSHIPS_QUERY_INPUT_CONTRACT_MISMATCH`, `PARTY_RELATIONSHIPS_QUERY_INPUT_PROTOBUF_INVALID`, `QUERY_RESOURCE_NOT_FOUND`, `PARTY_RELATIONSHIPS_QUERY_CAPABILITY_UNSUPPORTED`, `PARTY_RELATIONSHIPS_QUERY_VISIBILITY_SCAN_LIMIT_EXCEEDED`, `PARTY_RELATIONSHIPS_QUERY_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_module_sdk`, `crm_party_relationships`, `crm_party_relationships_capability_adapter`, `crm_proto_contracts`, `crm_query_runtime`

## 87. `crm-projection-runtime` — RETIRE-REVIEW / D? + B? invariants

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-projection-runtime/src/lib.rs) · 702 lines

- **Public declarations (15):** `try_new@L24`, `as_str@L36`, `new@L79`, `projection_id@L111`, `consumer_module_id@L115`, `event_types@L119`, `new@L130`, `get@L149`, `len@L159`, `is_empty@L163`, `projection_ids@L167`, `new@L203`, `registry@L207`, `run_batch@L211`, `rebuild@L265`
- **Internal/trait/helpers (26):** `borrow@L42`, `fmt@L48`, `project@L55`, `fmt@L67`, `fmt@L193`, `mark_failed@L288`, `projection_application@L312`, `validate_delivery_binding@L347`, `validate_page_size@L377`, `configuration_error@L389`, `runtime_error@L398`, `with_delivery@L440`, `projection_checkpoint@L451`, `list_event_history@L471`, `apply_projection_event@L491`, `mark_projection_failed@L514`, `reset_projection@L528`, `project@L546`, `project@L562`, `registry_rejects_duplicate_projection_ids@L576`, `runner_resumes_from_checkpoint_and_rebuild_resets_state@L584`, `deterministic_handler_failure_poison_marks_without_advancing_checkpoint@L615`, `cross_tenant_delivery_is_rejected_without_poisoning_another_tenant@L637`, `definition@L656`, `tenant@L666`, `delivery_for_tenant@L670`
- **Public types:** `ProjectionId@L21`, `ProjectionHandler@L54`, `ProjectionDefinition@L59`, `ProjectionRegistry@L125`, `ProjectionBatchResult@L173`, `ProjectionRunner@L187`
- **Detected tests (4):** `registry_rejects_duplicate_projection_ids@L576`, `runner_resumes_from_checkpoint_and_rebuild_resets_state@L584`, `deterministic_handler_failure_poison_marks_without_advancing_checkpoint@L615`, `cross_tenant_delivery_is_rejected_without_poisoning_another_tenant@L637`
- **Error/guard literals:** `PROJECTION_ID_INVALID`, `PROJECTION_EVENT_TYPES_EMPTY`, `PROJECTION_EVENT_TYPES_DUPLICATE`, `PROJECTION_ID_DUPLICATE`, `PROJECTION_NOT_REGISTERED`, `PROJECTION_FAILURE_RECORD_INVALID`, `PROJECTION_HANDLER_OUTPUT_INVALID`, `PROJECTION_DELIVERY_TENANT_MISMATCH`, `PROJECTION_DELIVERY_CONSUMER_MISMATCH`, `PROJECTION_EVENT_TYPE_NOT_SUBSCRIBED`, `PROJECTION_PAGE_SIZE_INVALID`, `PROJECTION_CHECKPOINT_FAILED`, `TEST_PROJECTION_POISON`
- **Rust crate references:** `crm_core_events`, `crm_module_sdk`

## 88. `crm-query-runtime` — RETIRE-REVIEW / D? + B? invariants

### [`src/gateway.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-query-runtime/src/gateway.rs) · 508 lines

- **Public declarations (4):** `validate@L24`, `new@L95`, `execute@L109`, `query_gateway_error_to_sdk@L254`
- **Internal/trait/helpers (18):** `validate@L51`, `authorize@L59`, `execute@L67`, `fmt@L83`, `validate_definition_binding@L165`, `validate_output_contract@L192`, `fmt@L233`, `resolve@L302`, `validate@L323`, `authorize@L339`, `execute@L363`, `live_authorization_denial_never_reaches_query_executor@L378`, `query_gateway_rejects_mutation_definition_before_authorization@L404`, `definition@L430`, `contract@L448`, `request@L460`, `input_payload@L479`, `output_payload@L493`
- **Public types:** `QueryExecutionContext@L11`, `QueryRequest@L38`, `QueryExecutionResult@L46`, `QuerySemanticValidator@L50`, `QueryAuthorizer@L58`, `QueryExecutor@L66`, `QueryGateway@L75`, `QueryGatewayError@L212`
- **Detected tests (2):** `live_authorization_denial_never_reaches_query_executor@L378`, `query_gateway_rejects_mutation_definition_before_authorization@L404`
- **Error/guard literals:** `QUERY_EXECUTION_CONTEXT_INVALID`, `QUERY_NOT_FOUND`, `QUERY_PERMISSION_DENIED`, `QUERY_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_module_sdk`

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-query-runtime/src/lib.rs) · 534 lines

- **Public declarations (10):** `validate@L40`, `validate@L58`, `new@L81`, `encode@L91`, `decode@L111`, `validate@L159`, `resolve@L167`, `normalized_filter_hash@L183`, `code@L215`, `safe_message@L232`
- **Internal/trait/helpers (24):** `fmt@L72`, `sign@L136`, `verify@L143`, `fmt@L251`, `encode_payload@L258`, `decode_payload@L284`, `push_string@L340`, `push_bytes@L344`, `hash_field@L351`, `new@L362`, `byte@L366`, `string@L370`, `bytes@L374`, `take@L383`, `is_finished@L396`, `codec@L405`, `binding@L409`, `continuation@L425`, `cursor_round_trip_preserves_keyset_continuation@L433`, `one_byte_tampering_is_rejected_before_payload_use@L441`, `cursor_cannot_move_between_query_bindings@L453`, `normalized_filter_hash_is_order_independent_but_value_sensitive@L482`, `page_size_policy_has_deterministic_default_and_hard_maximum@L500`, `cursor_secret_and_continuation_have_governed_bounds@L519`
- **Public types:** `CursorBinding@L28`, `CursorContinuation@L52`, `CursorCodec@L67`, `PageSizePolicy@L153`, `CursorError@L198`
- **Detected tests (6):** `cursor_round_trip_preserves_keyset_continuation@L433`, `one_byte_tampering_is_rejected_before_payload_use@L441`, `cursor_cannot_move_between_query_bindings@L453`, `normalized_filter_hash_is_order_independent_but_value_sensitive@L482`, `page_size_policy_has_deterministic_default_and_hard_maximum@L500`, `cursor_secret_and_continuation_have_governed_bounds@L519`
- **Error/guard literals:** `QUERY_CURSOR_CONFIGURATION_INVALID`, `QUERY_CURSOR_BINDING_INVALID`, `QUERY_CURSOR_TOO_LARGE`, `QUERY_CURSOR_INVALID`, `QUERY_CURSOR_TAMPERED`, `QUERY_CURSOR_BINDING_MISMATCH`, `QUERY_CURSOR_VERSION_UNSUPPORTED`, `QUERY_PAGE_POLICY_INVALID`, `QUERY_PAGE_SIZE_INVALID`, `QUERY_PAGE_SIZE_EXCEEDS_LIMIT`
- **Rust crate references:** `crm_module_sdk`

### [`src/query_lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-query-runtime/src/query_lib.rs) · 11 lines

- **Re-exports:** `pub use cursor::*;`, `pub use gateway::*;`, `pub use visibility::*;`

### [`src/visibility.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-query-runtime/src/visibility.rs) · 35 lines

- **Public declarations (2):** `denied@L14`, `allows_field@L23`
- **Internal/trait/helpers (1):** `authorize_visibility@L29`
- **Public types:** `QueryVisibilityDecision@L6`, `QueryVisibilityAuthorizer@L28`
- **Rust crate references:** `crm_module_sdk`

## 89. `crm-sales-activities-capability-composition` — NOW / A? + B-NOW-if-gap + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-activities-capability-composition/src/lib.rs) · 316 lines

- **Public declarations (2):** `capability_definitions@L55`, `capability_catalog@L64`
- **Internal/trait/helpers (12):** `target@L80`, `plan@L91`, `validated_route@L112`, `expected_definition@L153`, `configuration_error@L191`, `catalog_contains_exactly_the_seven_phase6_coordinates_in_stable_order@L206`, `retired_create_v1_coordinate_is_rejected_before_payload_decoding@L226`, `router_rejects_unknown_coordinate_before_payload_decoding@L238`, `router_rejects_version_mismatch_before_payload_decoding@L249`, `router_rejects_owner_mismatch_before_payload_decoding@L260`, `router_rejects_request_coordinate_mismatch_before_payload_decoding@L271`, `request_for@L281`
- **Public types:** `SalesActivitiesCapabilityPlannerRouter@L77`
- **Re-exports:** `pub use production_contribution::{`, `pub use crm_global_search_composition::*;`, `pub use link_event_processor::*;`, `pub use phase6_projections::*;`, `pub use query_catalog::*;`, `pub use query_router::*;`
- **Detected tests (6):** `catalog_contains_exactly_the_seven_phase6_coordinates_in_stable_order@L206`, `retired_create_v1_coordinate_is_rejected_before_payload_decoding@L226`, `router_rejects_unknown_coordinate_before_payload_decoding@L238`, `router_rejects_version_mismatch_before_payload_decoding@L249`, `router_rejects_owner_mismatch_before_payload_decoding@L260`, `router_rejects_request_coordinate_mismatch_before_payload_decoding@L271`
- **Error/guard literals:** `CAPABILITY_CATALOG_CONFIGURATION_INVALID`, `CAPABILITY_PLANNER_VERSION_MISMATCH`, `CAPABILITY_PLANNER_OWNER_MISMATCH`, `CAPABILITY_PLANNER_REQUEST_BINDING_MISMATCH`, `CAPABILITY_PLANNER_ROUTE_UNSUPPORTED`
- **Rust crate references:** `crm_global_search_composition`, `crm_activities_capability_adapter`, `crm_capability_adapters`, `crm_capability_runtime`, `crm_core_data`, `crm_module_sdk`, `crm_sales_capability_adapter`

### [`src/link_event_processor.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-activities-capability-composition/src/link_event_processor.rs) · 512 lines

- **Public declarations (2):** `new@L70`, `process@L85`
- **Internal/trait/helpers (17):** `validate@L48`, `finish_error@L212`, `retry_at@L244`, `delivery_context@L251`, `is_terminal_delivery_error@L289`, `configured_module_id@L297`, `configured_capability_id@L301`, `configured_capability_version@L305`, `configuration_invalid@L309`, `get@L324`, `put@L340`, `delete@L372`, `state_version_conflict@L395`, `state_store_unavailable@L404`, `target_context_is_stable_across_delivery_retries_except_request_attempt_identity@L422`, `only_non_retryable_invalid_or_conflicting_evidence_is_terminal@L448`, `delivery@L475`
- **Public types:** `SalesActivitiesLinkDeliveryOutcome@L23`, `SalesActivitiesLinkEventProcessorConfig@L40`, `SalesActivitiesLinkEventProcessor@L61`
- **Detected tests (2):** `target_context_is_stable_across_delivery_retries_except_request_attempt_identity@L422`, `only_non_retryable_invalid_or_conflicting_evidence_is_terminal@L448`
- **Error/guard literals:** `EVENT_DELIVERY_CONSUMER_INACTIVE`, `LINK_EVENT_PROCESSOR_CONFIGURATION_INVALID`, `SDK_VERSION_CONFLICT`, `LINK_EVENT_PROCESSOR_STATE_UNAVAILABLE`, `INVALID`, `CONFLICT`
- **Rust crate references:** `crm_capability_adapters`, `crm_capability_runtime`, `crm_core_data`, `crm_module_sdk`, `crm_sales_activities_link`, `crm_sales_activities_link_contract_adapter`

### [`src/phase6_projections.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-activities-capability-composition/src/phase6_projections.rs) · 371 lines

- **Public declarations (6):** `phase6_projection_registry@L65`, `new@L99`, `try_new@L103`, `run_batch@L109`, `rebuild@L120`, `runner@L131`
- **Internal/trait/helpers (15):** `project@L46`, `project@L55`, `configured_event_types@L136`, `deal_timeline_writes@L146`, `task_status_writes@L185`, `validate_task_snapshot@L254`, `task_status_name@L267`, `validate_contract@L275`, `decode@L295`, `configured_projection_id@L303`, `configured_module_id@L308`, `projection_configuration_invalid@L312`, `projection_event_invalid@L322`, `projection_coordinates_are_stable_and_non_overlapping@L337`, `compatibility_worker_is_backed_by_the_generic_registry@L360`
- **Public types:** `Phase6ProjectionWorker@L94`
- **Re-exports:** `pub use crm_projection_runtime::ProjectionBatchResult;`
- **Detected tests (2):** `projection_coordinates_are_stable_and_non_overlapping@L337`, `compatibility_worker_is_backed_by_the_generic_registry@L360`
- **Error/guard literals:** `PHASE6_PROJECTION_CONFIGURATION_INVALID`, `PHASE6_PROJECTION_EVENT_INVALID`
- **Rust crate references:** `crm_core_data`, `crm_core_events`, `crm_module_sdk`, `crm_projection_runtime`, `crm_proto_contracts`

### [`src/production_contribution.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-activities-capability-composition/src/production_contribution.rs) · 109 lines

- **Public declarations (3):** `mutation_capability_definitions@L31`, `production_query_capability_definitions@L35`, `build_contribution@L39`
- **Internal/trait/helpers (2):** `cursor@L88`, `production_composition_error@L100`
- **Public types:** `SalesActivitiesProductionDependencies@L24`
- **Error/guard literals:** `SALES_ACTIVITIES_CURSOR_CONFIGURATION_INVALID`, `SALES_ACTIVITIES_PRODUCTION_COMPOSITION_INVALID`
- **Rust crate references:** `crm_application_composition`, `crm_capability_runtime`, `crm_core_data`, `crm_module_sdk`, `crm_query_runtime`, `crm_sales_activities_query_adapter`

### [`src/query_catalog.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-activities-capability-composition/src/query_catalog.rs) · 51 lines

- **Public declarations (2):** `query_capability_definitions@L7`, `query_capability_catalog@L13`
- **Internal/trait/helpers (1):** `query_catalog_contains_owner_queries_and_governed_search@L32`
- **Detected tests (1):** `query_catalog_contains_owner_queries_and_governed_search@L32`
- **Error/guard literals:** `QUERY_CAPABILITY_CATALOG_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_capability_adapters`, `crm_capability_runtime`, `crm_module_sdk`, `crm_sales_activities_query_adapter`, `crm_search_query_adapter`

### [`src/query_router.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-activities-capability-composition/src/query_router.rs) · 92 lines

- **Public declarations (1):** `new@L18`
- **Internal/trait/helpers (4):** `validate@L27`, `execute@L41`, `route@L60`, `routes_existing_owner_queries_and_platform_search_without_overlap@L80`
- **Public types:** `ProductionQueryRouter@L12`
- **Detected tests (1):** `routes_existing_owner_queries_and_platform_search_without_overlap@L80`
- **Error/guard literals:** `PRODUCTION_QUERY_ROUTE_UNSUPPORTED`
- **Rust crate references:** `crm_capability_runtime`, `crm_module_sdk`, `crm_query_runtime`, `crm_sales_activities_query_adapter`, `crm_search_query_adapter`

### [`tests/catalog_contract.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-activities-capability-composition/tests/catalog_contract.rs) · 65 lines

- **Internal/trait/helpers (1):** `production_catalog_coordinates_are_unique_and_partitioned_by_owner@L11`
- **Detected tests (1):** `production_catalog_coordinates_are_unique_and_partitioned_by_owner@L11`
- **Rust crate references:** `crm_activities_capability_adapter`, `crm_sales_activities_capability_composition`, `crm_sales_capability_adapter`

### [`tests/postgres_link_delivery.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-activities-capability-composition/tests/postgres_link_delivery.rs) · 538 lines

- **Internal/trait/helpers (17):** `production_link_delivery_is_idempotent_tenant_bound_and_lifecycle_gated@L47`, `production_gateway@L189`, `create_deal@L238`, `advance_deal@L277`, `caller_context@L315`, `definition@L336`, `payload@L344`, `actor_owner@L363`, `stage_changed_event_id@L369`, `assert_task_exists@L391`, `evidence_counts@L410`, `count@L420`, `provision_link_module@L440`, `set_link_installation_status@L459`, `remove_link_installation@L493`, `bind_fixture_context@L508`, `delete_rebuildable_delivery_ledger@L527`
- **Detected tests (1):** `production_link_delivery_is_idempotent_tenant_bound_and_lifecycle_gated@L47`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`
- **Rust crate references:** `crm_capability_adapters`, `crm_capability_runtime`, `crm_core_data`, `crm_module_sdk`, `crm_proto_contracts`, `crm_sales_activities_capability_composition`, `crm_sales_activities_link`

### [`tests/postgres_projections.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-activities-capability-composition/tests/postgres_projections.rs) · 450 lines

- **Internal/trait/helpers (9):** `projections_rebuild_from_immutable_events_without_authoritative_side_effects@L47`, `production_gateway@L287`, `invoke@L342`, `latest_definition@L362`, `caller_context@L375`, `payload@L396`, `actor_owner@L415`, `evidence_counts@L421`, `count@L431`
- **Detected tests (1):** `projections_rebuild_from_immutable_events_without_authoritative_side_effects@L47`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`
- **Rust crate references:** `crm_capability_adapters`, `crm_capability_runtime`, `crm_core_data`, `crm_module_sdk`, `crm_proto_contracts`, `crm_sales_activities_capability_composition`

### [`tests/postgres_queries.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-activities-capability-composition/tests/postgres_queries.rs) · 797 lines

- **Internal/trait/helpers (23):** `validate@L50`, `production_queries_are_permission_bound_cursor_safe_and_side_effect_free@L69`, `authorization_grant@L466`, `visibility_grant@L483`, `mutation_definition@L509`, `query_definition@L517`, `mutation_request@L525`, `query_request@L561`, `execute_query@L587`, `payload@L609`, `sales_create@L625`, `task_create@L650`, `list_deals@L670`, `list_tasks@L683`, `actor_owner@L701`, `sales_fields@L707`, `task_fields@L727`, `assert_execution_code@L746`, `assert_semantic_code@L753`, `assert_cursor_rejected@L760`, `tamper_token@L774`, `evidence_counts@L781`, `scalar_count@L791`
- **Detected tests (1):** `production_queries_are_permission_bound_cursor_safe_and_side_effect_free@L69`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`, `QUERY_RESOURCE_NOT_FOUND`, `QUERY_CURSOR_BINDING_MISMATCH`, `QUERY_CURSOR_TAMPERED`, `QUERY_CURSOR_INVALID`
- **Rust crate references:** `crm_capability_adapters`, `crm_capability_runtime`, `crm_core_data`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`, `crm_sales_activities_capability_composition`, `crm_sales_activities_query_adapter`

## 90. `crm-sales-activities-query-adapter` — NOW / A? + B-NOW-if-gap + D? adapter

### [`src/definitions.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-activities-query-adapter/src/definitions.rs) · 118 lines

- **Public declarations (2):** `query_capability_definitions@L31`, `query_capability_definition@L38`
- **Internal/trait/helpers (1):** `publishes_exactly_four_read_only_query_coordinates@L100`
- **Detected tests (1):** `publishes_exactly_four_read_only_query_coordinates@L100`
- **Error/guard literals:** `QUERY_CAPABILITY_UNSUPPORTED`
- **Rust crate references:** `crm_capability_runtime`, `crm_module_sdk`

### [`src/executor.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-activities-query-adapter/src/executor.rs) · 792 lines

- **Public declarations (1):** `new@L50`
- **Internal/trait/helpers (31):** `fmt@L38`, `validate@L71`, `execute@L109`, `execute_get_deal@L128`, `execute_get_task@L160`, `execute_list_deals@L192`, `execute_list_tasks@L226`, `collect_deals@L263`, `collect_tasks@L323`, `has_more_visible_deal@L384`, `has_more_visible_task@L424`, `validate_sales_list@L466`, `validate_activities_list@L502`, `validate_sales_sort@L536`, `validate_activities_sort@L547`, `validate_owner@L560`, `validate_record_id@L578`, `resolve_page_size@L583`, `cursor_binding@L592`, `decode_after@L611`, `encode_next@L640`, `sales_filter_hash@L661`, `activities_filter_hash@L680`, `decode_input@L699`, `enforce_scan_limit@L731`, `resource_not_found@L739`, `unsupported_query@L748`, `unsupported_sort@L757`, `scan_limit_error@L764`, `cursor_error@L773`, `config_error@L783`
- **Public types:** `SalesActivitiesQueryAdapter@L30`
- **Error/guard literals:** `QUERY_CURSOR_INVALID`, `QUERY_INPUT_CONTRACT_MISMATCH`, `QUERY_INPUT_PROTOBUF_INVALID`, `QUERY_RESOURCE_NOT_FOUND`, `QUERY_CAPABILITY_UNSUPPORTED`, `QUERY_SCAN_LIMIT_EXCEEDED`, `QUERY_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-activities-query-adapter/src/lib.rs) · 12 lines

- **Re-exports:** `pub use definitions::*;`, `pub use executor::*;`

### [`src/wire.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-activities-query-adapter/src/wire.rs) · 389 lines

- **Public declarations (9):** `sales_record_type@L22`, `activities_record_type@L26`, `deal_from_snapshot@L31`, `task_from_snapshot@L51`, `deal_to_wire@L75`, `task_to_wire@L189`, `deal_matches@L261`, `task_matches@L272`, `validate_related_resource_tenant@L369`
- **Internal/trait/helpers (9):** `deal_owner_matches@L291`, `task_owner_matches@L300`, `owner_to_wire@L309`, `task_owner_to_wire@L319`, `stage_to_wire@L329`, `deal_status_to_wire@L337`, `close_outcome_to_wire@L345`, `task_priority_to_wire@L353`, `task_status_to_wire@L362`
- **Error/guard literals:** `SALES_QUERY_PERSISTED_DEAL_IDENTITY_INVALID`, `ACTIVITIES_QUERY_PERSISTED_TASK_IDENTITY_INVALID`
- **Rust crate references:** `crm_activities`, `crm_capability_plan_support`, `crm_core_data`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`, `crm_sales`

## 91. `crm-sales-capability-adapter` — NOW / A? + B-NOW-if-gap + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-capability-adapter/src/lib.rs) · 10 lines

- **Re-exports:** `pub use planner::*;`

### [`src/planner.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-capability-adapter/src/planner.rs) · 839 lines

- **Public declarations (1):** `capability_definition@L43`
- **Internal/trait/helpers (34):** `target@L77`, `plan@L108`, `plan_create@L124`, `plan_update@L200`, `plan_advance@L268`, `mutation_plan@L346`, `persisted_contract@L374`, `persisted_payload@L385`, `deal_from_snapshot@L389`, `deal_to_wire@L402`, `owner_from_wire@L442`, `owner_to_wire@L457`, `stage_from_wire@L467`, `stage_to_wire@L476`, `status_from_wire@L484`, `status_to_wire@L493`, `close_outcome_to_wire@L501`, `optional_resource@L509`, `string_patch@L519`, `owner_patch@L540`, `resource_patch@L561`, `money_patch@L585`, `date_patch@L606`, `probability_patch@L627`, `probability@L650`, `changed_fields@L658`, `ensure_target@L684`, `required@L691`, `ensure_definition@L695`, `unsupported_capability@L710`, `invalid_plan@L719`, `create_plan_is_deterministic_and_uses_internal_persisted_schema@L739`, `cross_tenant_resource_is_rejected@L762`, `create_request@L771`
- **Public types:** `SalesDealCapabilityPlanner@L41`
- **Detected tests (2):** `create_plan_is_deterministic_and_uses_internal_persisted_schema@L739`, `cross_tenant_resource_is_rejected@L762`
- **Error/guard literals:** `SALES_PERSISTED_DEAL_IDENTITY_INVALID`, `SALES_CAPABILITY_UNSUPPORTED`, `SALES_MUTATION_PLAN_INVALID`
- **Rust crate references:** `crm_capability_plan_support`, `crm_capability_runtime`, `crm_core_contracts`, `crm_core_data`, `crm_module_sdk`, `crm_proto_contracts`, `crm_sales`

### [`tests/postgres_mutations.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-sales-capability-adapter/tests/postgres_mutations.rs) · 229 lines

- **Internal/trait/helpers (2):** `deal_mutations_persist_versions_events_and_original_replay_output@L25`, `request@L187`
- **Detected tests (1):** `deal_mutations_persist_versions_events_and_original_replay_output@L25`
- **Error/guard literals:** `DATABASE_URL`, `ADMIN_DATABASE_URL`
- **Rust crate references:** `crm_capability_runtime`, `crm_core_data`, `crm_module_sdk`, `crm_proto_contracts`, `crm_sales_capability_adapter`

## 92. `crm-search-query-adapter` — LATER / B-LATER + D? adapter

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-search-query-adapter/src/lib.rs) · 277 lines

- **Public declarations (2):** `search_query_capability_definition@L34`, `new@L75`
- **Internal/trait/helpers (9):** `fmt@L65`, `validate@L89`, `execute@L115`, `runtime_request@L153`, `decode_input@L175`, `validate_definition@L203`, `configured@L219`, `publishes_one_stable_read_only_personal_search_capability@L236`, `runtime_request_deduplicates_resource_type_filters@L262`
- **Public types:** `SearchQueryAdapter@L59`
- **Detected tests (2):** `publishes_one_stable_read_only_personal_search_capability@L236`, `runtime_request_deduplicates_resource_type_filters@L262`
- **Error/guard literals:** `SEARCH_QUERY_INPUT_CONTRACT_MISMATCH`, `SEARCH_QUERY_INPUT_PROTOBUF_INVALID`, `SEARCH_QUERY_DEFINITION_MISMATCH`, `SEARCH_QUERY_CONFIGURATION_INVALID`
- **Rust crate references:** `crm_capability_runtime`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`, `crm_search_runtime`

### [`tests/live_permission_revocation.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-search-query-adapter/tests/live_permission_revocation.rs) · 193 lines

- **Internal/trait/helpers (5):** `search_candidates@L33`, `live_field_grant_revocation_is_reflected_without_reindexing@L70`, `match_on_a_hidden_field_discloses_neither_identity_nor_match_metadata@L112`, `search_grant@L137`, `query_request@L152`
- **Detected tests (2):** `live_field_grant_revocation_is_reflected_without_reindexing@L70`, `match_on_a_hidden_field_discloses_neither_identity_nor_match_metadata@L112`
- **Rust crate references:** `crm_capability_adapters`, `crm_module_sdk`, `crm_proto_contracts`, `crm_query_runtime`, `crm_search_query_adapter`, `crm_search_runtime`, `crm_capability_runtime`

### [`tests/public_contract.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-search-query-adapter/tests/public_contract.rs) · 25 lines

- **Internal/trait/helpers (1):** `public_search_response_round_trips_only_client_safe_match_data@L6`
- **Detected tests (1):** `public_search_response_round_trips_only_client_safe_match_data@L6`
- **Rust crate references:** `crm_proto_contracts`

## 93. `crm-search-runtime` — LATER / B-LATER + D? adapter

### [`src/generation.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-search-runtime/src/generation.rs) · 243 lines

- **Public declarations (7):** `new@L96`, `index_id@L121`, `generation_id@L125`, `projection_id@L129`, `ensure_ready@L133`, `reindex@L156`, `catch_up@L200`
- **Internal/trait/helpers (8):** `active_generation@L25`, `register_building_generation@L31`, `activate_generation@L36`, `add_batch@L52`, `fmt@L82`, `reindex_with_active@L168`, `active_generation_rebuild_conflict@L223`, `validate_coordinate@L232`
- **Public types:** `SearchGenerationStatus@L8`, `SearchIndexGeneration@L15`, `SearchGenerationStore@L24`, `SearchCatchUpResult@L45`, `SearchGenerationAction@L66`, `SearchReindexCoordinator@L72`
- **Error/guard literals:** `SEARCH_REINDEX_ACTIVE_GENERATION_CONFLICT`, `SEARCH_GENERATION_COORDINATE_INVALID`
- **Rust crate references:** `crm_module_sdk`, `crm_projection_runtime`

### [`src/lib.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-search-runtime/src/lib.rs) · 761 lines

- **Public declarations (7):** `try_new@L40`, `as_str@L46`, `validate@L70`, `into_projection_write@L90`, `validate@L125`, `new@L251`, `search@L271`
- **Internal/trait/helpers (23):** `fmt@L52`, `cursor@L166`, `validate@L174`, `search_candidates@L205`, `fmt@L239`, `search_cursor_binding@L404`, `search_filter_hash@L423`, `hash_field@L437`, `encode_candidate_cursor@L442`, `decode_candidate_cursor@L452`, `normalize_search_text@L482`, `validate_coordinate@L496`, `validate_fields@L503`, `cursor_error@L519`, `search_invalid@L528`, `search_internal@L532`, `search_candidates@L561`, `candidate_after@L608`, `authorize_visibility@L622`, `hidden_resource_and_hidden_matching_field_are_non_disclosing@L643`, `projection_document_has_deterministic_search_text_and_no_permission_snapshot@L693`, `candidate@L717`, `query_request@L732`
- **Public types:** `SearchIndexId@L37`, `SearchProjectionDocument@L58`, `SearchRequest@L116`, `SearchCandidateCursor@L138`, `SearchCandidateRequest@L145`, `SearchCandidate@L155`, `SearchCandidatePage@L199`, `SearchCandidateStore@L204`, `SearchHit@L212`, `SearchPage@L224`, `PermissionAwareSearch@L230`
- **Re-exports:** `pub use generation::*;`
- **Detected tests (2):** `hidden_resource_and_hidden_matching_field_are_non_disclosing@L643`, `projection_document_has_deterministic_search_text_and_no_permission_snapshot@L693`
- **Error/guard literals:** `SEARCH_INDEX_ID_INVALID`, `SEARCH_GENERATION_ID_INVALID`, `SEARCH_SCHEMA_VERSION_INVALID`, `SEARCH_SOURCE_VERSION_INVALID`, `SEARCH_FIELDS_EMPTY`, `SEARCH_TEXT_EMPTY`, `SEARCH_CANDIDATE_INVALID`, `SEARCH_CANDIDATE_MATCH_EVIDENCE_INVALID`, `SEARCH_STORE_CURSOR_INVALID`, `SEARCH_STORE_CURSOR_STALLED`, `SEARCH_CURSOR_RESOURCE_INVALID`, `SEARCH_CURSOR_INVALID`, `SEARCH_TEXT_TOO_LARGE`, `SEARCH_FIELD_INVALID`
- **Rust crate references:** `crm_core_events`, `crm_module_sdk`, `crm_query_runtime`

### [`tests/generation.rs`](https://github.com/iamaman11/crm/blob/a627e6a9f1966aaac5fcc0179c943811f879eb38/crates/crm-search-runtime/tests/generation.rs) · 229 lines

- **Internal/trait/helpers (14):** `projection_checkpoint@L21`, `list_event_history@L29`, `apply_projection_event@L41`, `mark_projection_failed@L53`, `reset_projection@L57`, `active_generation@L82`, `register_building_generation@L90`, `activate_generation@L100`, `project@L125`, `inactive_generation_rebuilds_before_activation@L134`, `matching_active_generation_catches_up_without_reset_or_reactivation@L156`, `explicit_reindex_rejects_the_active_generation_without_resetting_live_documents@L175`, `active_generation@L193`, `coordinator@L204`
- **Detected tests (3):** `inactive_generation_rebuilds_before_activation@L134`, `matching_active_generation_catches_up_without_reset_or_reactivation@L156`, `explicit_reindex_rejects_the_active_generation_without_resetting_live_documents@L175`
- **Error/guard literals:** `SEARCH_REINDEX_ACTIVE_GENERATION_CONFLICT`
- **Rust crate references:** `crm_core_events`, `crm_module_sdk`, `crm_projection_runtime`, `crm_search_runtime`


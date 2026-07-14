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
    let source = manifest_dir.join("src/lib.rs");

    replace_once(
        &source,
        "    BatchMutationPlan, PostgresDataStore, RecordGetQuery, RecordMutation, RelationshipMutation,\n    batch_error_to_sdk,\n",
        "    BatchMutationPlan, FileArtifactCapabilityEvidence, FileArtifactCapabilityMutation,\n    FileArtifactCapabilityMutationResult, PostgresDataStore, RecordGetQuery, RecordMutation,\n    RelationshipMutation, batch_error_to_sdk,\n",
    );
    replace_once(
        &source,
        "use crm_core_files::{FileArtifactStatus, ImmutableFileArtifactStore};\n",
        "use crm_core_files::{\n    AppendImmutableFileChunk, CreateImmutableFileArtifact, FileArtifactMetadata, FileArtifactStatus,\n    ImmutableFileArtifactStore,\n};\n",
    );
    replace_once(
        &source,
        "    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, FileId, ModuleId, PortFuture,\n    RecordId, RecordRef, RecordType, RelationshipRef, RelationshipType, SdkError, TypedPayload,\n",
        "    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, FileId, ModuleId, PortFuture,\n    RecordId, RecordRef, RecordType, RelationshipRef, RelationshipType, ResourceRef,\n    RetentionPolicyId, SdkError, TypedPayload,\n",
    );
    replace_once(
        &source,
        "pub const CREATE_JOB_FROM_SOURCE_CAPABILITY: &str = \"customer_data.import.party.source.job.create\";\n",
        "pub const CREATE_SOURCE_ARTIFACT_CAPABILITY: &str =\n    \"customer_data.import.party.source.create\";\npub const APPEND_SOURCE_CHUNK_CAPABILITY: &str =\n    \"customer_data.import.party.source.chunk.append\";\npub const FINALIZE_SOURCE_ARTIFACT_CAPABILITY: &str =\n    \"customer_data.import.party.source.finalize\";\npub const CREATE_JOB_FROM_SOURCE_CAPABILITY: &str = \"customer_data.import.party.source.job.create\";\n",
    );
    replace_once(
        &source,
        "pub const CREATE_JOB_FROM_SOURCE_REQUEST_SCHEMA: &str =\n",
        "pub const CREATE_SOURCE_ARTIFACT_REQUEST_SCHEMA: &str =\n    \"crm.customer_data_operations.v1.CreatePartyImportSourceArtifactRequest\";\npub const CREATE_SOURCE_ARTIFACT_RESPONSE_SCHEMA: &str =\n    \"crm.customer_data_operations.v1.CreatePartyImportSourceArtifactResponse\";\npub const APPEND_SOURCE_CHUNK_REQUEST_SCHEMA: &str =\n    \"crm.customer_data_operations.v1.AppendPartyImportSourceChunkRequest\";\npub const APPEND_SOURCE_CHUNK_RESPONSE_SCHEMA: &str =\n    \"crm.customer_data_operations.v1.AppendPartyImportSourceChunkResponse\";\npub const FINALIZE_SOURCE_ARTIFACT_REQUEST_SCHEMA: &str =\n    \"crm.customer_data_operations.v1.FinalizePartyImportSourceArtifactRequest\";\npub const FINALIZE_SOURCE_ARTIFACT_RESPONSE_SCHEMA: &str =\n    \"crm.customer_data_operations.v1.FinalizePartyImportSourceArtifactResponse\";\n\npub const SOURCE_ARTIFACT_CREATED_EVENT_TYPE: &str =\n    \"customer_data.import.party.source.created\";\npub const SOURCE_ARTIFACT_CREATED_EVENT_SCHEMA: &str =\n    \"crm.customer_data_operations.v1.PartyImportSourceArtifactCreatedEvent\";\npub const SOURCE_CHUNK_APPENDED_EVENT_TYPE: &str =\n    \"customer_data.import.party.source.chunk_appended\";\npub const SOURCE_CHUNK_APPENDED_EVENT_SCHEMA: &str =\n    \"crm.customer_data_operations.v1.PartyImportSourceChunkAppendedEvent\";\npub const SOURCE_ARTIFACT_FINALIZED_EVENT_TYPE: &str =\n    \"customer_data.import.party.source.finalized\";\npub const SOURCE_ARTIFACT_FINALIZED_EVENT_SCHEMA: &str =\n    \"crm.customer_data_operations.v1.PartyImportSourceArtifactFinalizedEvent\";\n\npub const CREATE_JOB_FROM_SOURCE_REQUEST_SCHEMA: &str =\n",
    );
    replace_once(
        &source,
        "pub const SOURCE_MUTATION_CAPABILITY_IDS: [&str; 2] = [\n    CREATE_JOB_FROM_SOURCE_CAPABILITY,\n    VALIDATE_SOURCE_BATCH_CAPABILITY,\n];\n",
        "pub const SOURCE_MUTATION_CAPABILITY_IDS: [&str; 5] = [\n    CREATE_SOURCE_ARTIFACT_CAPABILITY,\n    APPEND_SOURCE_CHUNK_CAPABILITY,\n    FINALIZE_SOURCE_ARTIFACT_CAPABILITY,\n    CREATE_JOB_FROM_SOURCE_CAPABILITY,\n    VALIDATE_SOURCE_BATCH_CAPABILITY,\n];\n",
    );
    replace_once(
        &source,
        "    let (input_schema, output_schema, risk) = match capability_id {\n        CREATE_JOB_FROM_SOURCE_CAPABILITY => (\n",
        "    let (input_schema, output_schema, risk) = match capability_id {\n        CREATE_SOURCE_ARTIFACT_CAPABILITY => (\n            CREATE_SOURCE_ARTIFACT_REQUEST_SCHEMA,\n            CREATE_SOURCE_ARTIFACT_RESPONSE_SCHEMA,\n            CapabilityRisk::High,\n        ),\n        APPEND_SOURCE_CHUNK_CAPABILITY => (\n            APPEND_SOURCE_CHUNK_REQUEST_SCHEMA,\n            APPEND_SOURCE_CHUNK_RESPONSE_SCHEMA,\n            CapabilityRisk::High,\n        ),\n        FINALIZE_SOURCE_ARTIFACT_CAPABILITY => (\n            FINALIZE_SOURCE_ARTIFACT_REQUEST_SCHEMA,\n            FINALIZE_SOURCE_ARTIFACT_RESPONSE_SCHEMA,\n            CapabilityRisk::High,\n        ),\n        CREATE_JOB_FROM_SOURCE_CAPABILITY => (\n",
    );
    replace_once(
        &source,
        "            match definition.capability_id.as_str() {\n                CREATE_JOB_FROM_SOURCE_CAPABILITY => {\n",
        "            match definition.capability_id.as_str() {\n                CREATE_SOURCE_ARTIFACT_CAPABILITY => {\n                    self.create_source_artifact(definition, request).await\n                }\n                APPEND_SOURCE_CHUNK_CAPABILITY => {\n                    self.append_source_chunk(definition, request).await\n                }\n                FINALIZE_SOURCE_ARTIFACT_CAPABILITY => {\n                    self.finalize_source_artifact(definition, request).await\n                }\n                CREATE_JOB_FROM_SOURCE_CAPABILITY => {\n",
    );

    let method_anchor = "impl CustomerDataOperationsSourceExecutor {\n    async fn create_job_from_source(\n";
    let methods = r#"impl CustomerDataOperationsSourceExecutor {
    async fn create_source_artifact(
        &self,
        definition: &CapabilityDefinition,
        request: CapabilityRequest,
    ) -> Result<CapabilityExecutionResult, SdkError> {
        let command: wire::CreatePartyImportSourceArtifactRequest =
            support::decode_request_with_data_class(
                &request,
                MODULE_ID,
                CREATE_SOURCE_ARTIFACT_REQUEST_SCHEMA,
                DataClass::Personal,
            )?;
        let file_id = artifact_id_from_ref(command.source_artifact_ref)?;
        let expected_sha256 = sha256_array(
            &command.expected_sha256,
            "customer_data.import.source_artifact.expected_sha256",
        )?;
        let mutation = FileArtifactCapabilityMutation::Create(CreateImmutableFileArtifact {
            file_id,
            owner_module_id: ModuleId::try_new(MODULE_ID).map_err(identifier_configuration_error)?,
            media_type: "text/csv".to_owned(),
            data_class: DataClass::Personal,
            retention_policy_id: RetentionPolicyId::try_new("crm.customer_data.import_source")
                .map_err(identifier_configuration_error)?,
            expected_size_bytes: command.expected_size_bytes,
            expected_sha256,
        });
        self.store
            .execute_file_artifact_capability(definition, request, mutation, |result, request| {
                source_artifact_create_evidence(definition, result, request)
            })
            .await
            .map_err(batch_error_to_sdk)
    }

    async fn append_source_chunk(
        &self,
        definition: &CapabilityDefinition,
        request: CapabilityRequest,
    ) -> Result<CapabilityExecutionResult, SdkError> {
        let command: wire::AppendPartyImportSourceChunkRequest =
            support::decode_request_with_data_class(
                &request,
                MODULE_ID,
                APPEND_SOURCE_CHUNK_REQUEST_SCHEMA,
                DataClass::Personal,
            )?;
        let file_id = artifact_id_from_ref(command.source_artifact_ref)?;
        let chunk_index = command.chunk_index;
        let mutation = FileArtifactCapabilityMutation::AppendChunk(AppendImmutableFileChunk {
            file_id,
            chunk_index,
            chunk_sha256: sha256_array(
                &command.chunk_sha256,
                "customer_data.import.source_artifact.chunk_sha256",
            )?,
            bytes: command.chunk_bytes,
        });
        self.store
            .execute_file_artifact_capability(definition, request, mutation, |result, request| {
                source_chunk_append_evidence(definition, result, request, chunk_index)
            })
            .await
            .map_err(batch_error_to_sdk)
    }

    async fn finalize_source_artifact(
        &self,
        definition: &CapabilityDefinition,
        request: CapabilityRequest,
    ) -> Result<CapabilityExecutionResult, SdkError> {
        let command: wire::FinalizePartyImportSourceArtifactRequest =
            support::decode_request_with_data_class(
                &request,
                MODULE_ID,
                FINALIZE_SOURCE_ARTIFACT_REQUEST_SCHEMA,
                DataClass::Personal,
            )?;
        let mutation = FileArtifactCapabilityMutation::Finalize {
            file_id: artifact_id_from_ref(command.source_artifact_ref)?,
        };
        self.store
            .execute_file_artifact_capability(definition, request, mutation, |result, request| {
                source_artifact_finalize_evidence(definition, result, request)
            })
            .await
            .map_err(batch_error_to_sdk)
    }

    async fn create_job_from_source(
"#;
    replace_once(&source, method_anchor, methods);

    let helper_anchor = "fn validate_source_row(\n";
    let helpers = r#"fn source_artifact_create_evidence(
    definition: &CapabilityDefinition,
    result: &FileArtifactCapabilityMutationResult,
    request: &CapabilityRequest,
) -> Result<FileArtifactCapabilityEvidence, SdkError> {
    let public = source_artifact_to_wire(&result.metadata);
    let output = support::protobuf_payload(
        MODULE_ID,
        CREATE_SOURCE_ARTIFACT_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::CreatePartyImportSourceArtifactResponse {
            source_artifact: Some(public.clone()),
        },
    )?;
    file_artifact_evidence(
        definition,
        result,
        request,
        output,
        if result.changed {
            Some((
                SOURCE_ARTIFACT_CREATED_EVENT_TYPE,
                SOURCE_ARTIFACT_CREATED_EVENT_SCHEMA,
                support::protobuf_payload(
                    MODULE_ID,
                    SOURCE_ARTIFACT_CREATED_EVENT_SCHEMA,
                    DataClass::Personal,
                    &wire::PartyImportSourceArtifactCreatedEvent {
                        source_artifact: Some(public),
                    },
                )?,
            ))
        } else {
            None
        },
    )
}

fn source_chunk_append_evidence(
    definition: &CapabilityDefinition,
    result: &FileArtifactCapabilityMutationResult,
    request: &CapabilityRequest,
    chunk_index: u64,
) -> Result<FileArtifactCapabilityEvidence, SdkError> {
    let public = source_artifact_to_wire(&result.metadata);
    let output = support::protobuf_payload(
        MODULE_ID,
        APPEND_SOURCE_CHUNK_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::AppendPartyImportSourceChunkResponse {
            source_artifact: Some(public.clone()),
            replayed: result.chunk_replayed,
        },
    )?;
    file_artifact_evidence(
        definition,
        result,
        request,
        output,
        if result.changed {
            Some((
                SOURCE_CHUNK_APPENDED_EVENT_TYPE,
                SOURCE_CHUNK_APPENDED_EVENT_SCHEMA,
                support::protobuf_payload(
                    MODULE_ID,
                    SOURCE_CHUNK_APPENDED_EVENT_SCHEMA,
                    DataClass::Personal,
                    &wire::PartyImportSourceChunkAppendedEvent {
                        source_artifact: Some(public),
                        chunk_index,
                    },
                )?,
            ))
        } else {
            None
        },
    )
}

fn source_artifact_finalize_evidence(
    definition: &CapabilityDefinition,
    result: &FileArtifactCapabilityMutationResult,
    request: &CapabilityRequest,
) -> Result<FileArtifactCapabilityEvidence, SdkError> {
    let public = source_artifact_to_wire(&result.metadata);
    let output = support::protobuf_payload(
        MODULE_ID,
        FINALIZE_SOURCE_ARTIFACT_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::FinalizePartyImportSourceArtifactResponse {
            source_artifact: Some(public.clone()),
        },
    )?;
    file_artifact_evidence(
        definition,
        result,
        request,
        output,
        if result.changed {
            Some((
                SOURCE_ARTIFACT_FINALIZED_EVENT_TYPE,
                SOURCE_ARTIFACT_FINALIZED_EVENT_SCHEMA,
                support::protobuf_payload(
                    MODULE_ID,
                    SOURCE_ARTIFACT_FINALIZED_EVENT_SCHEMA,
                    DataClass::Personal,
                    &wire::PartyImportSourceArtifactFinalizedEvent {
                        source_artifact: Some(public),
                    },
                )?,
            ))
        } else {
            None
        },
    )
}

fn file_artifact_evidence(
    definition: &CapabilityDefinition,
    result: &FileArtifactCapabilityMutationResult,
    request: &CapabilityRequest,
    output: TypedPayload,
    event: Option<(&str, &str, TypedPayload)>,
) -> Result<FileArtifactCapabilityEvidence, SdkError> {
    let aggregate = support::record_ref(
        "file_artifact",
        result.metadata.file_id.as_str(),
        "customer_data.import.source_artifact_ref.file_id",
    )?;
    let version = file_artifact_version(&result.metadata)?;
    let events = event
        .map(|(event_type, event_schema_id, payload)| {
            let mut evidence = support::event_evidence_with_data_class(
                request,
                aggregate.clone(),
                MODULE_ID,
                EventSpec {
                    event_type,
                    event_schema_id,
                    aggregate_version: version,
                    previous_version: version.checked_sub(1),
                },
                DataClass::Personal,
                &wire::PartyImportSourceArtifactCreatedEvent {
                    source_artifact: Some(source_artifact_to_wire(&result.metadata)),
                },
            )?;
            evidence.event.payload = payload;
            Ok::<_, SdkError>(evidence)
        })
        .transpose()?
        .into_iter()
        .collect();
    Ok(FileArtifactCapabilityEvidence {
        output: output.clone(),
        events,
        audits: vec![support::audit_intent(
            request,
            &aggregate,
            version,
            definition.capability_id.as_str(),
            &output.bytes,
        )?],
        affected_resources: vec![ResourceRef {
            resource_type: "file_artifact".to_owned(),
            resource_id: result.metadata.file_id.as_str().to_owned(),
            version: Some(version),
        }],
    })
}

fn source_artifact_to_wire(metadata: &FileArtifactMetadata) -> wire::PartyImportSourceArtifact {
    wire::PartyImportSourceArtifact {
        source_artifact_ref: Some(wire::PartyImportSourceArtifactRef {
            file_id: metadata.file_id.as_str().to_owned(),
        }),
        expected_size_bytes: metadata.expected_size_bytes,
        expected_sha256: metadata.expected_sha256.to_vec(),
        received_size_bytes: metadata.received_size_bytes,
        next_chunk_index: metadata.next_chunk_index,
        finalized: metadata.status == FileArtifactStatus::Finalized,
    }
}

fn file_artifact_version(metadata: &FileArtifactMetadata) -> Result<i64, SdkError> {
    let base = metadata
        .next_chunk_index
        .checked_add(1)
        .and_then(|value| {
            if metadata.status == FileArtifactStatus::Finalized {
                value.checked_add(1)
            } else {
                Some(value)
            }
        })
        .ok_or_else(invalid_plan)?;
    i64::try_from(base).map_err(|_| invalid_plan())
}

fn sha256_array(bytes: &[u8], field: &'static str) -> Result<[u8; 32], SdkError> {
    bytes.try_into().map_err(|_| {
        SdkError::invalid_argument(field, "SHA-256 must contain exactly 32 bytes")
    })
}

fn validate_source_row(
"#;
    replace_once(&source, helper_anchor, helpers);

    run(repo, "cargo", &["fmt", "--all"]);
    fs::remove_file(manifest_dir.join("build.rs"))
        .expect("temporary source upload capability patch must be removable");
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
            "feat(phase8a7): add evidenced import source upload capabilities",
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

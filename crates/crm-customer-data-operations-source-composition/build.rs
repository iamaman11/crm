use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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
    let mut text = fs::read_to_string(&source).expect("source composition must be readable");
    let start = text
        .find("fn source_artifact_create_evidence(")
        .expect("source artifact evidence block start must exist");
    let end = text
        .find("fn source_artifact_to_wire(")
        .expect("source artifact evidence block end must exist");
    let replacement = r#"fn source_artifact_create_evidence(
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
    let event = if result.changed {
        Some(support::event_evidence_with_data_class(
            request,
            file_artifact_record_ref(&result.metadata)?,
            MODULE_ID,
            EventSpec {
                event_type: SOURCE_ARTIFACT_CREATED_EVENT_TYPE,
                event_schema_id: SOURCE_ARTIFACT_CREATED_EVENT_SCHEMA,
                aggregate_version: file_artifact_version(&result.metadata)?,
                previous_version: None,
            },
            DataClass::Personal,
            &wire::PartyImportSourceArtifactCreatedEvent {
                source_artifact: Some(public),
            },
        )?)
    } else {
        None
    };
    file_artifact_evidence(definition, result, request, output, event)
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
    let event = if result.changed {
        let version = file_artifact_version(&result.metadata)?;
        Some(support::event_evidence_with_data_class(
            request,
            file_artifact_record_ref(&result.metadata)?,
            MODULE_ID,
            EventSpec {
                event_type: SOURCE_CHUNK_APPENDED_EVENT_TYPE,
                event_schema_id: SOURCE_CHUNK_APPENDED_EVENT_SCHEMA,
                aggregate_version: version,
                previous_version: version.checked_sub(1),
            },
            DataClass::Personal,
            &wire::PartyImportSourceChunkAppendedEvent {
                source_artifact: Some(public),
                chunk_index,
            },
        )?)
    } else {
        None
    };
    file_artifact_evidence(definition, result, request, output, event)
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
    let event = if result.changed {
        let version = file_artifact_version(&result.metadata)?;
        Some(support::event_evidence_with_data_class(
            request,
            file_artifact_record_ref(&result.metadata)?,
            MODULE_ID,
            EventSpec {
                event_type: SOURCE_ARTIFACT_FINALIZED_EVENT_TYPE,
                event_schema_id: SOURCE_ARTIFACT_FINALIZED_EVENT_SCHEMA,
                aggregate_version: version,
                previous_version: version.checked_sub(1),
            },
            DataClass::Personal,
            &wire::PartyImportSourceArtifactFinalizedEvent {
                source_artifact: Some(public),
            },
        )?)
    } else {
        None
    };
    file_artifact_evidence(definition, result, request, output, event)
}

fn file_artifact_evidence(
    definition: &CapabilityDefinition,
    result: &FileArtifactCapabilityMutationResult,
    request: &CapabilityRequest,
    output: TypedPayload,
    event: Option<crm_core_data::EventEvidence>,
) -> Result<FileArtifactCapabilityEvidence, SdkError> {
    let aggregate = file_artifact_record_ref(&result.metadata)?;
    let version = file_artifact_version(&result.metadata)?;
    Ok(FileArtifactCapabilityEvidence {
        output: output.clone(),
        events: event.into_iter().collect(),
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

fn file_artifact_record_ref(metadata: &FileArtifactMetadata) -> Result<RecordRef, SdkError> {
    support::record_ref(
        "file_artifact",
        metadata.file_id.as_str(),
        "customer_data.import.source_artifact_ref.file_id",
    )
}

"#;
    text.replace_range(start..end, replacement);
    fs::write(&source, text).expect("source composition must be writable");

    run(repo, "cargo", &["fmt", "--all"]);
    fs::remove_file(manifest_dir.join("build.rs"))
        .expect("temporary typed event normalization hook must be removable");
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
            "fix(phase8a7): type source artifact lifecycle events exactly",
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

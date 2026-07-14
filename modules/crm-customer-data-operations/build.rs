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
        .expect("module must live under repository/modules");

    let domain = manifest_dir.join("src/domain.rs");
    replace_once(
        &domain,
        "use crm_module_sdk::{ErrorCategory, FieldName, FieldViolation, RecordId, SdkError};",
        "use crm_module_sdk::{ErrorCategory, FieldName, FieldViolation, FileId, RecordId, SdkError};",
    );
    replace_once(
        &domain,
        r#"#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceDescriptor {
    source_name: String,
    content_sha256: String,
    row_count: u32,
    source_system_id: SourceSystemId,
    parser_profile: ImportParserProfile,
}

impl SourceDescriptor {
    pub fn try_new(
        source_name: impl Into<String>,
        content_sha256: impl Into<String>,
        row_count: u32,
        source_system_id: SourceSystemId,
        parser_profile: ImportParserProfile,
    ) -> Result<Self, SdkError> {
        let source_name = normalize_bounded_text(
            source_name.into(),
            MAX_SOURCE_NAME_BYTES,
            "CUSTOMER_DATA_SOURCE_NAME_INVALID",
            "customer_data.source.name",
            "source name",
        )?;
        let content_sha256 = normalize_sha256(
            content_sha256.into(),
            "CUSTOMER_DATA_SOURCE_SHA256_INVALID",
            "customer_data.source.content_sha256",
        )?;
        validate_row_count(row_count)?;
        Ok(Self {
            source_name,
            content_sha256,
            row_count,
            source_system_id,
            parser_profile,
        })
    }

    pub fn source_name(&self) -> &str {
        &self.source_name
    }

    pub fn content_sha256(&self) -> &str {
        &self.content_sha256
    }

    pub const fn row_count(&self) -> u32 {
        self.row_count
    }

    pub fn source_system_id(&self) -> &SourceSystemId {
        &self.source_system_id
    }

    pub fn parser_profile(&self) -> &ImportParserProfile {
        &self.parser_profile
    }
}
"#,
        r#"#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceDescriptor {
    source_artifact_id: Option<FileId>,
    source_name: String,
    content_sha256: String,
    row_count: u32,
    source_system_id: SourceSystemId,
    parser_profile: ImportParserProfile,
}

impl SourceDescriptor {
    pub fn try_new(
        source_name: impl Into<String>,
        content_sha256: impl Into<String>,
        row_count: u32,
        source_system_id: SourceSystemId,
        parser_profile: ImportParserProfile,
    ) -> Result<Self, SdkError> {
        Self::try_new_internal(
            None,
            source_name,
            content_sha256,
            row_count,
            source_system_id,
            parser_profile,
        )
    }

    pub fn try_new_bound(
        source_artifact_id: FileId,
        source_name: impl Into<String>,
        content_sha256: impl Into<String>,
        row_count: u32,
        source_system_id: SourceSystemId,
        parser_profile: ImportParserProfile,
    ) -> Result<Self, SdkError> {
        Self::try_new_internal(
            Some(source_artifact_id),
            source_name,
            content_sha256,
            row_count,
            source_system_id,
            parser_profile,
        )
    }

    fn try_new_internal(
        source_artifact_id: Option<FileId>,
        source_name: impl Into<String>,
        content_sha256: impl Into<String>,
        row_count: u32,
        source_system_id: SourceSystemId,
        parser_profile: ImportParserProfile,
    ) -> Result<Self, SdkError> {
        let source_name = normalize_bounded_text(
            source_name.into(),
            MAX_SOURCE_NAME_BYTES,
            "CUSTOMER_DATA_SOURCE_NAME_INVALID",
            "customer_data.source.name",
            "source name",
        )?;
        let content_sha256 = normalize_sha256(
            content_sha256.into(),
            "CUSTOMER_DATA_SOURCE_SHA256_INVALID",
            "customer_data.source.content_sha256",
        )?;
        validate_row_count(row_count)?;
        Ok(Self {
            source_artifact_id,
            source_name,
            content_sha256,
            row_count,
            source_system_id,
            parser_profile,
        })
    }

    pub fn source_artifact_id(&self) -> Option<&FileId> {
        self.source_artifact_id.as_ref()
    }

    pub fn source_name(&self) -> &str {
        &self.source_name
    }

    pub fn content_sha256(&self) -> &str {
        &self.content_sha256
    }

    pub const fn row_count(&self) -> u32 {
        self.row_count
    }

    pub fn source_system_id(&self) -> &SourceSystemId {
        &self.source_system_id
    }

    pub fn parser_profile(&self) -> &ImportParserProfile {
        &self.parser_profile
    }
}
"#,
    );

    let persistence = manifest_dir.join("src/persistence.rs");
    replace_once(
        &persistence,
        "use crm_module_sdk::{ErrorCategory, SdkError};",
        "use crm_module_sdk::{ErrorCategory, FileId, SdkError};",
    );
    replace_once(
        &persistence,
        "source[source_name,content_sha256,row_count,source_system_id,parser_profile[",
        "source[source_artifact_id,source_name,content_sha256,row_count,source_system_id,parser_profile[",
    );
    replace_once(
        &persistence,
        "struct SourceDescriptorStateV1 {\n    source_name: String,",
        "struct SourceDescriptorStateV1 {\n    source_artifact_id: Option<String>,\n    source_name: String,",
    );
    replace_once(
        &persistence,
        "        Self {\n            source_name: value.source_name().to_owned(),",
        "        Self {\n            source_artifact_id: value.source_artifact_id().map(|value| value.as_str().to_owned()),\n            source_name: value.source_name().to_owned(),",
    );
    replace_once(
        &persistence,
        r#"        let parser_profile: ImportParserProfile = value.parser_profile.try_into()?;
        let source = SourceDescriptor::try_new(
            value.source_name.clone(),
            value.content_sha256.clone(),
            value.row_count,
            source_system_id,
            parser_profile,
        )
        .map_err(|error| persisted_error(error.to_string()))?;
        if source.source_name() != value.source_name
            || source.content_sha256() != value.content_sha256
            || source.source_system_id().as_str() != value.source_system_id
        {
"#,
        r#"        let parser_profile: ImportParserProfile = value.parser_profile.try_into()?;
        let source_artifact_id = value
            .source_artifact_id
            .as_ref()
            .map(|value| FileId::try_new(value.clone()))
            .transpose()
            .map_err(|error| persisted_error(error.to_string()))?;
        let source = match source_artifact_id {
            Some(source_artifact_id) => SourceDescriptor::try_new_bound(
                source_artifact_id,
                value.source_name.clone(),
                value.content_sha256.clone(),
                value.row_count,
                source_system_id,
                parser_profile,
            ),
            None => SourceDescriptor::try_new(
                value.source_name.clone(),
                value.content_sha256.clone(),
                value.row_count,
                source_system_id,
                parser_profile,
            ),
        }
        .map_err(|error| persisted_error(error.to_string()))?;
        if source.source_artifact_id().map(|value| value.as_str())
                != value.source_artifact_id.as_deref()
            || source.source_name() != value.source_name
            || source.content_sha256() != value.content_sha256
            || source.source_system_id().as_str() != value.source_system_id
        {
"#,
    );

    run(repo, "cargo", &["fmt", "--all"]);
    fs::remove_file(manifest_dir.join("build.rs")).expect("temporary source binding patch must be removable");
}

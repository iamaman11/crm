#![forbid(unsafe_code)]

//! Strict typed v1 schemas for Admin Studio metadata definitions.
//!
//! This crate validates authoring-time metadata, extracts deterministic
//! dependencies and produces canonical `MetadataDocument` values for the
//! immutable publication runtime. It deliberately contains no persistence,
//! transport, browser, raw script, SQL or arbitrary network execution surface.

use crm_metadata_runtime::{
    MetadataDocument, MetadataError, MetadataId, MetadataKey, MetadataKind,
};
use crm_module_sdk::{CapabilityId, CapabilityVersion, DataClass, EventType, ModuleId};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

pub const METADATA_DEFINITION_SCHEMA_VERSION: &str = "crm.metadata.definition/v1";
pub const MAX_LABEL_BYTES: usize = 200;
pub const MAX_DESCRIPTION_BYTES: usize = 4_000;
pub const MAX_LOCAL_ID_BYTES: usize = 80;
pub const MAX_TEXT_LENGTH: u32 = 1_000_000;
pub const MAX_DECIMAL_PRECISION: u8 = 38;
pub const MAX_ENUM_OPTIONS: usize = 1_000;
pub const MAX_COLLECTION_MEMBERS: usize = 2_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "definition", rename_all = "snake_case")]
pub enum MetadataDefinition {
    Object(ObjectDefinition),
    Field(FieldDefinition),
    Relationship(RelationshipDefinition),
    Layout(LayoutDefinition),
    View(ViewDefinition),
    Pipeline(PipelineDefinition),
    Permission(PermissionDefinition),
    Workflow(WorkflowDefinition),
}

impl MetadataDefinition {
    pub fn kind(&self) -> MetadataKind {
        match self {
            Self::Object(_) => MetadataKind::Object,
            Self::Field(_) => MetadataKind::Field,
            Self::Relationship(_) => MetadataKind::Relationship,
            Self::Layout(_) => MetadataKind::Layout,
            Self::View(_) => MetadataKind::View,
            Self::Pipeline(_) => MetadataKind::Pipeline,
            Self::Permission(_) => MetadataKind::Permission,
            Self::Workflow(_) => MetadataKind::Workflow,
        }
    }

    pub fn id(&self) -> &str {
        match self {
            Self::Object(definition) => &definition.id,
            Self::Field(definition) => &definition.id,
            Self::Relationship(definition) => &definition.id,
            Self::Layout(definition) => &definition.id,
            Self::View(definition) => &definition.id,
            Self::Pipeline(definition) => &definition.id,
            Self::Permission(definition) => &definition.id,
            Self::Workflow(definition) => &definition.id,
        }
    }

    pub fn validate(&self) -> Result<(), SchemaError> {
        metadata_id(self.id(), "definition.id")?;
        match self {
            Self::Object(definition) => definition.validate(),
            Self::Field(definition) => definition.validate(),
            Self::Relationship(definition) => definition.validate(),
            Self::Layout(definition) => definition.validate(),
            Self::View(definition) => definition.validate(),
            Self::Pipeline(definition) => definition.validate(),
            Self::Permission(definition) => definition.validate(),
            Self::Workflow(definition) => definition.validate(),
        }
    }

    pub fn dependencies(&self) -> Result<BTreeSet<MetadataKey>, SchemaError> {
        self.validate()?;
        let mut dependencies = BTreeSet::new();
        match self {
            Self::Object(_) | Self::Workflow(_) => {}
            Self::Field(definition) => {
                dependencies.insert(metadata_key(
                    MetadataKind::Object,
                    &definition.object_id,
                    "field.object_id",
                )?);
                if let FieldType::Reference(config) = &definition.field_type {
                    dependencies.insert(metadata_key(
                        MetadataKind::Object,
                        &config.target_object_id,
                        "field.field_type.reference.target_object_id",
                    )?);
                }
            }
            Self::Relationship(definition) => {
                dependencies.insert(metadata_key(
                    MetadataKind::Object,
                    &definition.source_object_id,
                    "relationship.source_object_id",
                )?);
                dependencies.insert(metadata_key(
                    MetadataKind::Object,
                    &definition.target_object_id,
                    "relationship.target_object_id",
                )?);
            }
            Self::Layout(definition) => {
                dependencies.insert(metadata_key(
                    MetadataKind::Object,
                    &definition.object_id,
                    "layout.object_id",
                )?);
                for section in &definition.sections {
                    for field_id in &section.field_ids {
                        dependencies.insert(metadata_key(
                            MetadataKind::Field,
                            field_id,
                            "layout.sections.field_ids",
                        )?);
                    }
                }
            }
            Self::View(definition) => {
                dependencies.insert(metadata_key(
                    MetadataKind::Object,
                    &definition.object_id,
                    "view.object_id",
                )?);
                for field_id in &definition.column_field_ids {
                    dependencies.insert(metadata_key(
                        MetadataKind::Field,
                        field_id,
                        "view.column_field_ids",
                    )?);
                }
                for sort in &definition.sorts {
                    dependencies.insert(metadata_key(
                        MetadataKind::Field,
                        &sort.field_id,
                        "view.sorts.field_id",
                    )?);
                }
            }
            Self::Pipeline(definition) => {
                dependencies.insert(metadata_key(
                    MetadataKind::Object,
                    &definition.object_id,
                    "pipeline.object_id",
                )?);
                dependencies.insert(metadata_key(
                    MetadataKind::Field,
                    &definition.stage_field_id,
                    "pipeline.stage_field_id",
                )?);
            }
            Self::Permission(definition) => {
                dependencies.insert(metadata_key(
                    MetadataKind::Object,
                    &definition.object_id,
                    "permission.object_id",
                )?);
                for field_id in &definition.field_ids {
                    dependencies.insert(metadata_key(
                        MetadataKind::Field,
                        field_id,
                        "permission.field_ids",
                    )?);
                }
            }
        }
        Ok(dependencies)
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, SchemaError> {
        self.validate()?;
        serde_json::to_vec(&self.normalized()).map_err(|error| {
            SchemaError::new(
                SchemaErrorCode::CanonicalizationFailed,
                "definition",
                error.to_string(),
            )
        })
    }

    pub fn to_document(&self) -> Result<MetadataDocument, SchemaError> {
        let key = metadata_key(self.kind(), self.id(), "definition.id")?;
        let dependencies = self.dependencies()?;
        let canonical_content = self.canonical_bytes()?;
        MetadataDocument::new(
            key,
            METADATA_DEFINITION_SCHEMA_VERSION,
            canonical_content,
            dependencies,
        )
        .map_err(runtime_contract_error)
    }

    fn normalized(&self) -> Self {
        let mut definition = self.clone();
        match &mut definition {
            Self::Object(object) => object.tags.sort(),
            Self::Permission(permission) => {
                permission.actions.sort();
                permission.field_ids.sort();
            }
            _ => {}
        }
        definition
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectDefinition {
    pub id: String,
    pub owner_module_id: ModuleId,
    pub label: String,
    pub plural_label: String,
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl ObjectDefinition {
    fn validate(&self) -> Result<(), SchemaError> {
        validate_label(&self.label, "object.label")?;
        validate_label(&self.plural_label, "object.plural_label")?;
        validate_optional_description(self.description.as_deref(), "object.description")?;
        validate_bounded_collection(self.tags.len(), "object.tags")?;
        validate_unique_text(&self.tags, "object.tags", false)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldDefinition {
    pub id: String,
    pub object_id: String,
    pub label: String,
    pub data_class: DataClass,
    pub field_type: FieldType,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub immutable: bool,
}

impl FieldDefinition {
    fn validate(&self) -> Result<(), SchemaError> {
        metadata_id(&self.object_id, "field.object_id")?;
        validate_label(&self.label, "field.label")?;
        self.field_type.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "config", rename_all = "snake_case")]
pub enum FieldType {
    Text(TextFieldConfig),
    Integer,
    Decimal(DecimalFieldConfig),
    Boolean,
    Date,
    Timestamp,
    Enum(EnumFieldConfig),
    Reference(ReferenceFieldConfig),
}

impl FieldType {
    fn validate(&self) -> Result<(), SchemaError> {
        match self {
            Self::Text(config) => config.validate(),
            Self::Decimal(config) => config.validate(),
            Self::Enum(config) => config.validate(),
            Self::Reference(config) => config.validate(),
            Self::Integer | Self::Boolean | Self::Date | Self::Timestamp => Ok(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextFieldConfig {
    pub max_length: u32,
}

impl TextFieldConfig {
    fn validate(&self) -> Result<(), SchemaError> {
        if self.max_length == 0 || self.max_length > MAX_TEXT_LENGTH {
            return Err(SchemaError::new(
                SchemaErrorCode::InvalidBound,
                "field.field_type.text.max_length",
                format!("must be between 1 and {MAX_TEXT_LENGTH}"),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecimalFieldConfig {
    pub precision: u8,
    pub scale: u8,
}

impl DecimalFieldConfig {
    fn validate(&self) -> Result<(), SchemaError> {
        if self.precision == 0
            || self.precision > MAX_DECIMAL_PRECISION
            || self.scale > self.precision
        {
            return Err(SchemaError::new(
                SchemaErrorCode::InvalidBound,
                "field.field_type.decimal",
                format!(
                    "precision must be 1..={MAX_DECIMAL_PRECISION} and scale must not exceed precision"
                ),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnumFieldConfig {
    pub options: Vec<EnumOption>,
}

impl EnumFieldConfig {
    fn validate(&self) -> Result<(), SchemaError> {
        if self.options.is_empty() || self.options.len() > MAX_ENUM_OPTIONS {
            return Err(SchemaError::new(
                SchemaErrorCode::InvalidBound,
                "field.field_type.enum.options",
                format!("must contain between 1 and {MAX_ENUM_OPTIONS} options"),
            ));
        }
        let mut values = BTreeSet::new();
        for option in &self.options {
            validate_local_id(&option.value, "field.field_type.enum.options.value")?;
            validate_label(&option.label, "field.field_type.enum.options.label")?;
            if !values.insert(option.value.as_str()) {
                return Err(SchemaError::new(
                    SchemaErrorCode::DuplicateMember,
                    "field.field_type.enum.options.value",
                    format!("duplicate enum value `{}`", option.value),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnumOption {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceFieldConfig {
    pub target_object_id: String,
}

impl ReferenceFieldConfig {
    fn validate(&self) -> Result<(), SchemaError> {
        metadata_id(
            &self.target_object_id,
            "field.field_type.reference.target_object_id",
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationshipDefinition {
    pub id: String,
    pub label: String,
    pub source_object_id: String,
    pub target_object_id: String,
    pub cardinality: RelationshipCardinality,
}

impl RelationshipDefinition {
    fn validate(&self) -> Result<(), SchemaError> {
        validate_label(&self.label, "relationship.label")?;
        metadata_id(
            &self.source_object_id,
            "relationship.source_object_id",
        )?;
        metadata_id(
            &self.target_object_id,
            "relationship.target_object_id",
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipCardinality {
    OneToOne,
    OneToMany,
    ManyToMany,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayoutDefinition {
    pub id: String,
    pub object_id: String,
    pub label: String,
    pub sections: Vec<LayoutSection>,
}

impl LayoutDefinition {
    fn validate(&self) -> Result<(), SchemaError> {
        metadata_id(&self.object_id, "layout.object_id")?;
        validate_label(&self.label, "layout.label")?;
        if self.sections.is_empty() {
            return Err(SchemaError::new(
                SchemaErrorCode::InvalidBound,
                "layout.sections",
                "must contain at least one section",
            ));
        }
        validate_bounded_collection(self.sections.len(), "layout.sections")?;
        let mut section_ids = BTreeSet::new();
        let mut field_ids = BTreeSet::new();
        for section in &self.sections {
            validate_local_id(&section.id, "layout.sections.id")?;
            validate_label(&section.label, "layout.sections.label")?;
            if !section_ids.insert(section.id.as_str()) {
                return duplicate("layout.sections.id", &section.id);
            }
            validate_bounded_collection(section.field_ids.len(), "layout.sections.field_ids")?;
            for field_id in &section.field_ids {
                metadata_id(field_id, "layout.sections.field_ids")?;
                if !field_ids.insert(field_id.as_str()) {
                    return duplicate("layout.sections.field_ids", field_id);
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayoutSection {
    pub id: String,
    pub label: String,
    pub field_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewDefinition {
    pub id: String,
    pub object_id: String,
    pub label: String,
    pub column_field_ids: Vec<String>,
    #[serde(default)]
    pub sorts: Vec<ViewSort>,
}

impl ViewDefinition {
    fn validate(&self) -> Result<(), SchemaError> {
        metadata_id(&self.object_id, "view.object_id")?;
        validate_label(&self.label, "view.label")?;
        if self.column_field_ids.is_empty() {
            return Err(SchemaError::new(
                SchemaErrorCode::InvalidBound,
                "view.column_field_ids",
                "must contain at least one field",
            ));
        }
        validate_bounded_collection(self.column_field_ids.len(), "view.column_field_ids")?;
        let mut columns = BTreeSet::new();
        for field_id in &self.column_field_ids {
            metadata_id(field_id, "view.column_field_ids")?;
            if !columns.insert(field_id.as_str()) {
                return duplicate("view.column_field_ids", field_id);
            }
        }
        let mut sort_fields = BTreeSet::new();
        for sort in &self.sorts {
            metadata_id(&sort.field_id, "view.sorts.field_id")?;
            if !columns.contains(sort.field_id.as_str()) {
                return Err(SchemaError::new(
                    SchemaErrorCode::InvalidReference,
                    "view.sorts.field_id",
                    format!("sort field `{}` is not present in view columns", sort.field_id),
                ));
            }
            if !sort_fields.insert(sort.field_id.as_str()) {
                return duplicate("view.sorts.field_id", &sort.field_id);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewSort {
    pub field_id: String,
    pub direction: SortDirection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PipelineDefinition {
    pub id: String,
    pub object_id: String,
    pub stage_field_id: String,
    pub label: String,
    pub stages: Vec<PipelineStage>,
}

impl PipelineDefinition {
    fn validate(&self) -> Result<(), SchemaError> {
        metadata_id(&self.object_id, "pipeline.object_id")?;
        metadata_id(&self.stage_field_id, "pipeline.stage_field_id")?;
        validate_label(&self.label, "pipeline.label")?;
        if self.stages.is_empty() {
            return Err(SchemaError::new(
                SchemaErrorCode::InvalidBound,
                "pipeline.stages",
                "must contain at least one stage",
            ));
        }
        validate_bounded_collection(self.stages.len(), "pipeline.stages")?;
        let mut stage_ids = BTreeSet::new();
        let mut stage_orders = BTreeSet::new();
        let mut terminal_count = 0_usize;
        for stage in &self.stages {
            validate_local_id(&stage.id, "pipeline.stages.id")?;
            validate_label(&stage.label, "pipeline.stages.label")?;
            if !stage_ids.insert(stage.id.as_str()) {
                return duplicate("pipeline.stages.id", &stage.id);
            }
            if !stage_orders.insert(stage.order) {
                return duplicate("pipeline.stages.order", &stage.order.to_string());
            }
            if stage.terminal {
                terminal_count += 1;
            }
        }
        if terminal_count == 0 {
            return Err(SchemaError::new(
                SchemaErrorCode::InvalidReference,
                "pipeline.stages.terminal",
                "at least one terminal stage is required",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PipelineStage {
    pub id: String,
    pub label: String,
    pub order: u32,
    #[serde(default)]
    pub terminal: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PermissionDefinition {
    pub id: String,
    pub object_id: String,
    pub label: String,
    pub actions: Vec<PermissionAction>,
    #[serde(default)]
    pub field_ids: Vec<String>,
}

impl PermissionDefinition {
    fn validate(&self) -> Result<(), SchemaError> {
        metadata_id(&self.object_id, "permission.object_id")?;
        validate_label(&self.label, "permission.label")?;
        if self.actions.is_empty() {
            return Err(SchemaError::new(
                SchemaErrorCode::InvalidBound,
                "permission.actions",
                "must contain at least one action",
            ));
        }
        validate_bounded_collection(self.actions.len(), "permission.actions")?;
        let mut actions = BTreeSet::new();
        for action in &self.actions {
            if !actions.insert(*action) {
                return duplicate("permission.actions", action.as_str());
            }
        }
        validate_bounded_collection(self.field_ids.len(), "permission.field_ids")?;
        let mut fields = BTreeSet::new();
        for field_id in &self.field_ids {
            metadata_id(field_id, "permission.field_ids")?;
            if !fields.insert(field_id.as_str()) {
                return duplicate("permission.field_ids", field_id);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionAction {
    View,
    Create,
    Update,
    Delete,
    Export,
    Administer,
}

impl PermissionAction {
    const fn as_str(self) -> &'static str {
        match self {
            Self::View => "view",
            Self::Create => "create",
            Self::Update => "update",
            Self::Delete => "delete",
            Self::Export => "export",
            Self::Administer => "administer",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowDefinition {
    pub id: String,
    pub label: String,
    pub trigger: WorkflowTrigger,
    pub actions: Vec<WorkflowAction>,
}

impl WorkflowDefinition {
    fn validate(&self) -> Result<(), SchemaError> {
        validate_label(&self.label, "workflow.label")?;
        self.trigger.validate()?;
        if self.actions.is_empty() {
            return Err(SchemaError::new(
                SchemaErrorCode::InvalidBound,
                "workflow.actions",
                "must contain at least one governed capability action",
            ));
        }
        validate_bounded_collection(self.actions.len(), "workflow.actions")?;
        let mut action_ids = BTreeSet::new();
        for action in &self.actions {
            validate_local_id(&action.id, "workflow.actions.id")?;
            validate_exact_version(
                &action.capability_version,
                "workflow.actions.capability_version",
            )?;
            if !action_ids.insert(action.id.as_str()) {
                return duplicate("workflow.actions.id", &action.id);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "config", rename_all = "snake_case")]
pub enum WorkflowTrigger {
    Event(EventType),
    CapabilityCompleted(CapabilityReference),
}

impl WorkflowTrigger {
    fn validate(&self) -> Result<(), SchemaError> {
        match self {
            Self::Event(_) => Ok(()),
            Self::CapabilityCompleted(reference) => reference.validate("workflow.trigger"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityReference {
    pub capability_id: CapabilityId,
    pub capability_version: CapabilityVersion,
}

impl CapabilityReference {
    fn validate(&self, path: &str) -> Result<(), SchemaError> {
        validate_exact_version(
            &self.capability_version,
            &format!("{path}.capability_version"),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowAction {
    pub id: String,
    pub capability_id: CapabilityId,
    pub capability_version: CapabilityVersion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaErrorCode {
    InvalidIdentifier,
    InvalidLabel,
    InvalidBound,
    DuplicateMember,
    InvalidReference,
    InvalidVersion,
    CanonicalizationFailed,
    RuntimeContract,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaError {
    pub code: SchemaErrorCode,
    pub path: String,
    pub message: String,
}

impl SchemaError {
    fn new(
        code: SchemaErrorCode,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            path: path.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for SchemaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {} ({:?})", self.path, self.message, self.code)
    }
}

impl Error for SchemaError {}

fn metadata_id(value: &str, path: &str) -> Result<MetadataId, SchemaError> {
    MetadataId::try_new(value.to_owned()).map_err(|error| {
        SchemaError::new(
            SchemaErrorCode::InvalidIdentifier,
            path,
            error.safe_message,
        )
    })
}

fn metadata_key(kind: MetadataKind, value: &str, path: &str) -> Result<MetadataKey, SchemaError> {
    Ok(MetadataKey::new(kind, metadata_id(value, path)?))
}

fn validate_label(value: &str, path: &str) -> Result<(), SchemaError> {
    if value.trim().is_empty()
        || value.len() > MAX_LABEL_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(SchemaError::new(
            SchemaErrorCode::InvalidLabel,
            path,
            format!("must be non-empty, control-free and at most {MAX_LABEL_BYTES} bytes"),
        ));
    }
    Ok(())
}

fn validate_optional_description(value: Option<&str>, path: &str) -> Result<(), SchemaError> {
    if let Some(value) = value
        && (value.len() > MAX_DESCRIPTION_BYTES || value.chars().any(char::is_control))
    {
        return Err(SchemaError::new(
            SchemaErrorCode::InvalidBound,
            path,
            format!("must be control-free and at most {MAX_DESCRIPTION_BYTES} bytes"),
        ));
    }
    Ok(())
}

fn validate_local_id(value: &str, path: &str) -> Result<(), SchemaError> {
    let valid = !value.is_empty()
        && value.len() <= MAX_LOCAL_ID_BYTES
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase())
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'.' | b'_' | b'-')
        });
    if !valid {
        return Err(SchemaError::new(
            SchemaErrorCode::InvalidIdentifier,
            path,
            format!("must be a lowercase local identifier of at most {MAX_LOCAL_ID_BYTES} bytes"),
        ));
    }
    Ok(())
}

fn validate_bounded_collection(length: usize, path: &str) -> Result<(), SchemaError> {
    if length > MAX_COLLECTION_MEMBERS {
        return Err(SchemaError::new(
            SchemaErrorCode::InvalidBound,
            path,
            format!("must not contain more than {MAX_COLLECTION_MEMBERS} members"),
        ));
    }
    Ok(())
}

fn validate_unique_text(
    values: &[String],
    path: &str,
    validate_as_metadata_id: bool,
) -> Result<(), SchemaError> {
    let mut seen = BTreeSet::new();
    for value in values {
        if validate_as_metadata_id {
            metadata_id(value, path)?;
        } else if value.trim().is_empty() || value.chars().any(char::is_control) {
            return Err(SchemaError::new(
                SchemaErrorCode::InvalidLabel,
                path,
                "members must be non-empty and control-free",
            ));
        }
        if !seen.insert(value.as_str()) {
            return duplicate(path, value);
        }
    }
    Ok(())
}

fn validate_exact_version(value: &CapabilityVersion, path: &str) -> Result<(), SchemaError> {
    Version::parse(value.as_str()).map_err(|error| {
        SchemaError::new(SchemaErrorCode::InvalidVersion, path, error.to_string())
    })?;
    Ok(())
}

fn duplicate(path: &str, value: &str) -> Result<(), SchemaError> {
    Err(SchemaError::new(
        SchemaErrorCode::DuplicateMember,
        path,
        format!("duplicate member `{value}`"),
    ))
}

fn runtime_contract_error(error: MetadataError) -> SchemaError {
    SchemaError::new(
        SchemaErrorCode::RuntimeContract,
        "metadata_document",
        error.safe_message,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_metadata_runtime::{MetadataBundleDraft, MetadataErrorCode};

    fn module_id() -> ModuleId {
        ModuleId::try_new("crm.sales").unwrap()
    }

    fn capability_id() -> CapabilityId {
        CapabilityId::try_new("sales.deal.update").unwrap()
    }

    fn capability_version() -> CapabilityVersion {
        CapabilityVersion::try_new("1.0.0").unwrap()
    }

    fn object() -> MetadataDefinition {
        MetadataDefinition::Object(ObjectDefinition {
            id: "crm.sales.deal".to_owned(),
            owner_module_id: module_id(),
            label: "Deal".to_owned(),
            plural_label: "Deals".to_owned(),
            description: None,
            tags: vec!["sales".to_owned(), "commercial".to_owned()],
        })
    }

    fn field() -> MetadataDefinition {
        MetadataDefinition::Field(FieldDefinition {
            id: "crm.sales.deal.name".to_owned(),
            object_id: "crm.sales.deal".to_owned(),
            label: "Name".to_owned(),
            data_class: DataClass::Internal,
            field_type: FieldType::Text(TextFieldConfig { max_length: 200 }),
            required: true,
            immutable: false,
        })
    }

    #[test]
    fn all_eight_definition_kinds_produce_documents() {
        let definitions = vec![
            object(),
            field(),
            MetadataDefinition::Relationship(RelationshipDefinition {
                id: "crm.sales.deal.account".to_owned(),
                label: "Deal account".to_owned(),
                source_object_id: "crm.sales.deal".to_owned(),
                target_object_id: "crm.customer.account".to_owned(),
                cardinality: RelationshipCardinality::OneToMany,
            }),
            MetadataDefinition::Layout(LayoutDefinition {
                id: "crm.sales.deal.default".to_owned(),
                object_id: "crm.sales.deal".to_owned(),
                label: "Default deal layout".to_owned(),
                sections: vec![LayoutSection {
                    id: "main".to_owned(),
                    label: "Main".to_owned(),
                    field_ids: vec!["crm.sales.deal.name".to_owned()],
                }],
            }),
            MetadataDefinition::View(ViewDefinition {
                id: "crm.sales.deal.open".to_owned(),
                object_id: "crm.sales.deal".to_owned(),
                label: "Open deals".to_owned(),
                column_field_ids: vec!["crm.sales.deal.name".to_owned()],
                sorts: vec![ViewSort {
                    field_id: "crm.sales.deal.name".to_owned(),
                    direction: SortDirection::Ascending,
                }],
            }),
            MetadataDefinition::Pipeline(PipelineDefinition {
                id: "crm.sales.deal.default_pipeline".to_owned(),
                object_id: "crm.sales.deal".to_owned(),
                stage_field_id: "crm.sales.deal.stage".to_owned(),
                label: "Default deal pipeline".to_owned(),
                stages: vec![
                    PipelineStage {
                        id: "open".to_owned(),
                        label: "Open".to_owned(),
                        order: 10,
                        terminal: false,
                    },
                    PipelineStage {
                        id: "won".to_owned(),
                        label: "Won".to_owned(),
                        order: 20,
                        terminal: true,
                    },
                ],
            }),
            MetadataDefinition::Permission(PermissionDefinition {
                id: "crm.sales.deal.standard_access".to_owned(),
                object_id: "crm.sales.deal".to_owned(),
                label: "Standard deal access".to_owned(),
                actions: vec![PermissionAction::View, PermissionAction::Update],
                field_ids: vec!["crm.sales.deal.name".to_owned()],
            }),
            MetadataDefinition::Workflow(WorkflowDefinition {
                id: "crm.sales.deal.follow_up".to_owned(),
                label: "Deal follow-up".to_owned(),
                trigger: WorkflowTrigger::Event(
                    EventType::try_new("crm.sales.deal.updated").unwrap(),
                ),
                actions: vec![WorkflowAction {
                    id: "update_deal".to_owned(),
                    capability_id: capability_id(),
                    capability_version: capability_version(),
                }],
            }),
        ];

        let kinds = definitions
            .iter()
            .map(|definition| definition.to_document().unwrap().key().kind())
            .collect::<BTreeSet<_>>();
        assert_eq!(kinds.len(), 8);
    }

    #[test]
    fn set_like_members_are_canonicalized_independently_of_input_order() {
        let first = MetadataDefinition::Object(ObjectDefinition {
            id: "crm.sales.deal".to_owned(),
            owner_module_id: module_id(),
            label: "Deal".to_owned(),
            plural_label: "Deals".to_owned(),
            description: None,
            tags: vec!["sales".to_owned(), "commercial".to_owned()],
        });
        let second = MetadataDefinition::Object(ObjectDefinition {
            id: "crm.sales.deal".to_owned(),
            owner_module_id: module_id(),
            label: "Deal".to_owned(),
            plural_label: "Deals".to_owned(),
            description: None,
            tags: vec!["commercial".to_owned(), "sales".to_owned()],
        });
        assert_eq!(first.canonical_bytes().unwrap(), second.canonical_bytes().unwrap());

        let first_permission = MetadataDefinition::Permission(PermissionDefinition {
            id: "crm.sales.deal.standard_access".to_owned(),
            object_id: "crm.sales.deal".to_owned(),
            label: "Standard access".to_owned(),
            actions: vec![PermissionAction::Update, PermissionAction::View],
            field_ids: vec![
                "crm.sales.deal.value".to_owned(),
                "crm.sales.deal.name".to_owned(),
            ],
        });
        let second_permission = MetadataDefinition::Permission(PermissionDefinition {
            id: "crm.sales.deal.standard_access".to_owned(),
            object_id: "crm.sales.deal".to_owned(),
            label: "Standard access".to_owned(),
            actions: vec![PermissionAction::View, PermissionAction::Update],
            field_ids: vec![
                "crm.sales.deal.name".to_owned(),
                "crm.sales.deal.value".to_owned(),
            ],
        });
        assert_eq!(
            first_permission.canonical_bytes().unwrap(),
            second_permission.canonical_bytes().unwrap()
        );
    }

    #[test]
    fn meaningful_layout_order_is_preserved_in_canonical_bytes() {
        let first = MetadataDefinition::Layout(LayoutDefinition {
            id: "crm.sales.deal.default".to_owned(),
            object_id: "crm.sales.deal".to_owned(),
            label: "Default".to_owned(),
            sections: vec![
                LayoutSection {
                    id: "main".to_owned(),
                    label: "Main".to_owned(),
                    field_ids: vec!["crm.sales.deal.name".to_owned()],
                },
                LayoutSection {
                    id: "commercial".to_owned(),
                    label: "Commercial".to_owned(),
                    field_ids: vec!["crm.sales.deal.value".to_owned()],
                },
            ],
        });
        let second = MetadataDefinition::Layout(LayoutDefinition {
            id: "crm.sales.deal.default".to_owned(),
            object_id: "crm.sales.deal".to_owned(),
            label: "Default".to_owned(),
            sections: match first.clone() {
                MetadataDefinition::Layout(mut layout) => {
                    layout.sections.reverse();
                    layout.sections
                }
                _ => unreachable!(),
            },
        });
        assert_ne!(first.canonical_bytes().unwrap(), second.canonical_bytes().unwrap());
    }

    #[test]
    fn invalid_bounds_duplicates_and_intra_definition_references_fail_typed_validation() {
        let invalid_decimal = MetadataDefinition::Field(FieldDefinition {
            id: "crm.sales.deal.value".to_owned(),
            object_id: "crm.sales.deal".to_owned(),
            label: "Value".to_owned(),
            data_class: DataClass::Financial,
            field_type: FieldType::Decimal(DecimalFieldConfig {
                precision: 2,
                scale: 3,
            }),
            required: false,
            immutable: false,
        });
        assert_eq!(
            invalid_decimal.validate().unwrap_err().code,
            SchemaErrorCode::InvalidBound
        );

        let duplicate_layout = MetadataDefinition::Layout(LayoutDefinition {
            id: "crm.sales.deal.default".to_owned(),
            object_id: "crm.sales.deal".to_owned(),
            label: "Default".to_owned(),
            sections: vec![LayoutSection {
                id: "main".to_owned(),
                label: "Main".to_owned(),
                field_ids: vec![
                    "crm.sales.deal.name".to_owned(),
                    "crm.sales.deal.name".to_owned(),
                ],
            }],
        });
        assert_eq!(
            duplicate_layout.validate().unwrap_err().code,
            SchemaErrorCode::DuplicateMember
        );

        let invalid_view = MetadataDefinition::View(ViewDefinition {
            id: "crm.sales.deal.open".to_owned(),
            object_id: "crm.sales.deal".to_owned(),
            label: "Open deals".to_owned(),
            column_field_ids: vec!["crm.sales.deal.name".to_owned()],
            sorts: vec![ViewSort {
                field_id: "crm.sales.deal.value".to_owned(),
                direction: SortDirection::Descending,
            }],
        });
        assert_eq!(
            invalid_view.validate().unwrap_err().code,
            SchemaErrorCode::InvalidReference
        );
    }

    #[test]
    fn dependencies_feed_bundle_level_missing_dependency_validation() {
        let field_document = field().to_document().unwrap();
        let error = MetadataBundleDraft::new(vec![field_document]).unwrap_err();
        assert_eq!(error.code, MetadataErrorCode::MissingDependency);
    }

    #[test]
    fn workflow_actions_reject_arbitrary_script_fields() {
        let json = r#"{
            "id":"step",
            "capability_id":"sales.deal.update",
            "capability_version":"1.0.0",
            "script":"fetch('https://example.com')"
        }"#;
        assert!(serde_json::from_str::<WorkflowAction>(json).is_err());
    }
}

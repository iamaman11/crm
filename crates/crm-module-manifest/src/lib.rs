#![forbid(unsafe_code)]

//! Typed runtime representation of an Ultimate CRM module manifest.
//!
//! Raw YAML is intentionally outside this crate. Governance tooling compiles
//! authoring YAML into normalized JSON. Runtime code consumes that JSON,
//! validates typed and semantic invariants, and derives a deterministic
//! `crm.cjson/v1` identity.

use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Write as _};

pub const SCHEMA_VERSION: &str = "crm.module/v1";
pub const CANONICALIZATION_PROFILE: &str = "crm.cjson/v1";
pub const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleManifest {
    pub schema_version: String,
    pub module_id: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub owner: Owner,
    pub runtime: Runtime,
    pub platform: PlatformCompatibility,
    pub dependencies: Dependencies,
    pub provides: Provides,
    pub consumes: Consumes,
    pub storage: Storage,
    pub security: Security,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub quotas: BTreeMap<String, u64>,
    pub lifecycle: Lifecycle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Owner {
    pub team: String,
    pub contact: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub codeowners: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Runtime {
    pub kind: RuntimeKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_digest: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeKind {
    InProcess,
    Service,
    Wasm,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlatformCompatibility {
    pub minimum_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum_exclusive_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dependencies {
    pub required: Vec<ModuleDependency>,
    pub optional: Vec<ModuleDependency>,
    pub conflicts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleDependency {
    pub module_id: String,
    pub version_range: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provides {
    pub capabilities: Vec<VersionedContract>,
    pub events: Vec<VersionedContract>,
    pub objects: Vec<String>,
    pub ui_extensions: Vec<UiExtension>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Consumes {
    pub capabilities: Vec<VersionedContract>,
    pub events: Vec<VersionedContract>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VersionedContract {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiExtension {
    pub id: String,
    #[serde(rename = "type")]
    pub extension_type: UiExtensionType,
    pub version: String,
    pub permission: String,
    pub fallback: UiFallback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiExtensionType {
    #[serde(rename = "navigation.item")]
    NavigationItem,
    #[serde(rename = "record_page.tab")]
    RecordPageTab,
    #[serde(rename = "record_page.panel")]
    RecordPagePanel,
    #[serde(rename = "list_view.column")]
    ListViewColumn,
    #[serde(rename = "command_palette.action")]
    CommandPaletteAction,
    #[serde(rename = "dashboard.widget")]
    DashboardWidget,
    #[serde(rename = "workflow.node")]
    WorkflowNode,
    #[serde(rename = "admin_builder.component")]
    AdminBuilderComponent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiFallback {
    Hide,
    Readonly,
    Placeholder,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Storage {
    pub record_types: Vec<String>,
    pub private_state_namespaces: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Security {
    pub data_classes: Vec<DataClass>,
    pub network_egress: Vec<String>,
    pub secret_handles: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataClass {
    Public,
    Internal,
    Confidential,
    Restricted,
    Personal,
    SensitivePersonal,
    Biometric,
    Financial,
    Credential,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lifecycle {
    pub upgrade_policy: UpgradePolicy,
    pub rollback_policy: RollbackPolicy,
    pub uninstall_policy: UninstallPolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migrations_path: Option<String>,
    pub retained_record_types: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpgradePolicy {
    AutomaticCompatible,
    Manual,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RollbackPolicy {
    Supported,
    CompensatingOnly,
    NotSupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UninstallPolicy {
    DeletePrivateState,
    RetainBusinessRecords,
    BlockedWhileReferenced,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub code: String,
    pub path: String,
    pub message: String,
}

impl ValidationIssue {
    fn new(code: &str, path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            path: path.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug)]
pub enum ManifestError {
    Json(serde_json::Error),
    Validation(Vec<ValidationIssue>),
    Canonical(String),
}

impl fmt::Display for ManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(formatter, "invalid normalized manifest JSON: {error}"),
            Self::Validation(issues) => {
                write!(formatter, "module manifest validation failed")?;
                for issue in issues {
                    write!(
                        formatter,
                        "\n- {} at {}: {}",
                        issue.code, issue.path, issue.message
                    )?;
                }
                Ok(())
            }
            Self::Canonical(message) => write!(formatter, "canonicalization failed: {message}"),
        }
    }
}

impl Error for ManifestError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            Self::Validation(_) | Self::Canonical(_) => None,
        }
    }
}

impl From<serde_json::Error> for ManifestError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestIdentity {
    pub profile: &'static str,
    pub sha256: String,
}

impl ModuleManifest {
    pub fn from_normalized_json(input: &str) -> Result<Self, ManifestError> {
        let manifest: Self = serde_json::from_str(input)?;
        manifest.validate_or_error()?;
        Ok(manifest)
    }

    pub fn validate_or_error(&self) -> Result<(), ManifestError> {
        let issues = self.validate();
        if issues.is_empty() {
            Ok(())
        } else {
            Err(ManifestError::Validation(issues))
        }
    }

    pub fn validate(&self) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();

        if self.schema_version != SCHEMA_VERSION {
            issues.push(ValidationIssue::new(
                "schema_version",
                "$.schema_version",
                format!("expected {SCHEMA_VERSION}"),
            ));
        }
        validate_module_id(&self.module_id, "$.module_id", &mut issues);
        validate_semver(&self.version, "$.version", &mut issues);

        if self.display_name.as_ref().is_some_and(|value| value.is_empty() || value.len() > 120) {
            issues.push(ValidationIssue::new(
                "display_name",
                "$.display_name",
                "must contain 1 to 120 bytes",
            ));
        }
        if self.description.as_ref().is_some_and(|value| value.len() > 2_000) {
            issues.push(ValidationIssue::new(
                "description",
                "$.description",
                "must not exceed 2000 bytes",
            ));
        }

        validate_ascii_id(&self.owner.team, "$.owner.team", &mut issues);
        if !valid_contact(&self.owner.contact) {
            issues.push(ValidationIssue::new(
                "owner_contact",
                "$.owner.contact",
                "must be a bounded email address",
            ));
        }
        validate_unique_strings(&self.owner.codeowners, "$.owner.codeowners", &mut issues);
        for (index, codeowner) in self.owner.codeowners.iter().enumerate() {
            if !valid_codeowner(codeowner) {
                issues.push(ValidationIssue::new(
                    "codeowner",
                    format!("$.owner.codeowners[{index}]"),
                    "must use @user or @org/team syntax",
                ));
            }
        }

        if let Some(entrypoint) = &self.runtime.entrypoint
            && (!valid_path(entrypoint, true) || entrypoint.len() > 240)
        {
            issues.push(ValidationIssue::new(
                "runtime_entrypoint",
                "$.runtime.entrypoint",
                "contains unsupported characters or is too long",
            ));
        }
        if let Some(digest) = &self.runtime.artifact_digest
            && !valid_sha256_reference(digest)
        {
            issues.push(ValidationIssue::new(
                "artifact_digest",
                "$.runtime.artifact_digest",
                "must be sha256 followed by 64 lowercase hexadecimal characters",
            ));
        }
        match self.runtime.kind {
            RuntimeKind::InProcess | RuntimeKind::Service if self.runtime.entrypoint.is_none() => {
                issues.push(ValidationIssue::new(
                    "runtime_entrypoint_required",
                    "$.runtime.entrypoint",
                    "in-process and service modules require an entrypoint",
                ));
            }
            RuntimeKind::Wasm if self.runtime.artifact_digest.is_none() => {
                issues.push(ValidationIssue::new(
                    "runtime_digest_required",
                    "$.runtime.artifact_digest",
                    "WASM modules require an immutable artifact digest",
                ));
            }
            _ => {}
        }

        let minimum = parse_semver(&self.platform.minimum_version, "$.platform.minimum_version", &mut issues);
        let maximum = self
            .platform
            .maximum_exclusive_version
            .as_ref()
            .and_then(|value| parse_semver(value, "$.platform.maximum_exclusive_version", &mut issues));
        if let (Some(minimum), Some(maximum)) = (minimum, maximum)
            && minimum >= maximum
        {
            issues.push(ValidationIssue::new(
                "platform_range",
                "$.platform",
                "maximum_exclusive_version must be greater than minimum_version",
            ));
        }

        validate_dependencies(self, &mut issues);
        validate_contracts(&self.provides.capabilities, "$.provides.capabilities", &mut issues);
        validate_contracts(&self.provides.events, "$.provides.events", &mut issues);
        validate_contracts(&self.consumes.capabilities, "$.consumes.capabilities", &mut issues);
        validate_contracts(&self.consumes.events, "$.consumes.events", &mut issues);

        validate_namespaced_list(&self.provides.objects, "$.provides.objects", &mut issues);
        validate_namespaced_list(&self.storage.record_types, "$.storage.record_types", &mut issues);
        validate_namespaced_list(
            &self.storage.private_state_namespaces,
            "$.storage.private_state_namespaces",
            &mut issues,
        );
        validate_namespaced_list(
            &self.lifecycle.retained_record_types,
            "$.lifecycle.retained_record_types",
            &mut issues,
        );

        let owned: BTreeSet<_> = self.provides.objects.iter().collect();
        for (index, record_type) in self.storage.record_types.iter().enumerate() {
            if !owned.contains(record_type) {
                issues.push(ValidationIssue::new(
                    "storage_not_owned",
                    format!("$.storage.record_types[{index}]"),
                    format!("record type {record_type} is not owned by this module"),
                ));
            }
        }
        let stored: BTreeSet<_> = self.storage.record_types.iter().collect();
        for (index, record_type) in self.lifecycle.retained_record_types.iter().enumerate() {
            if !stored.contains(record_type) {
                issues.push(ValidationIssue::new(
                    "retained_not_stored",
                    format!("$.lifecycle.retained_record_types[{index}]"),
                    format!("record type {record_type} is not declared in storage"),
                ));
            }
        }
        for (index, namespace) in self.storage.private_state_namespaces.iter().enumerate() {
            if !namespace.starts_with(&self.module_id) {
                issues.push(ValidationIssue::new(
                    "private_namespace",
                    format!("$.storage.private_state_namespaces[{index}]"),
                    format!("must start with module_id {}", self.module_id),
                ));
            }
        }

        let mut ui_ids = BTreeSet::new();
        for (index, extension) in self.provides.ui_extensions.iter().enumerate() {
            validate_namespaced_id(&extension.id, &format!("$.provides.ui_extensions[{index}].id"), &mut issues);
            validate_semver(&extension.version, &format!("$.provides.ui_extensions[{index}].version"), &mut issues);
            validate_namespaced_id(
                &extension.permission,
                &format!("$.provides.ui_extensions[{index}].permission"),
                &mut issues,
            );
            if !ui_ids.insert(extension.id.clone()) {
                issues.push(ValidationIssue::new(
                    "duplicate_ui_extension",
                    format!("$.provides.ui_extensions[{index}].id"),
                    format!("duplicate UI extension id {}", extension.id),
                ));
            }
        }

        if duplicates(self.security.data_classes.iter().copied()).next().is_some() {
            issues.push(ValidationIssue::new(
                "duplicate_data_class",
                "$.security.data_classes",
                "data classes must be unique",
            ));
        }
        validate_unique_strings(&self.security.network_egress, "$.security.network_egress", &mut issues);
        for (index, target) in self.security.network_egress.iter().enumerate() {
            if !valid_network_target(target) {
                issues.push(ValidationIssue::new(
                    "network_egress",
                    format!("$.security.network_egress[{index}]"),
                    "must be a DNS name with an optional numeric port",
                ));
            }
        }
        validate_namespaced_list(&self.security.secret_handles, "$.security.secret_handles", &mut issues);

        for (name, value) in &self.quotas {
            validate_ascii_id(name, &format!("$.quotas.{name}"), &mut issues);
            if *value > MAX_SAFE_INTEGER {
                issues.push(ValidationIssue::new(
                    "quota_range",
                    format!("$.quotas.{name}"),
                    format!("must not exceed {MAX_SAFE_INTEGER}"),
                ));
            }
        }

        if let Some(path) = &self.lifecycle.migrations_path
            && (!valid_path(path, false) || path.len() > 240)
        {
            issues.push(ValidationIssue::new(
                "migrations_path",
                "$.lifecycle.migrations_path",
                "contains unsupported characters or is too long",
            ));
        }

        issues
    }

    pub fn canonical_json_bytes(&self) -> Result<Vec<u8>, ManifestError> {
        self.validate_or_error()?;
        let value = serde_json::to_value(self)?;
        let mut output = Vec::new();
        write_canonical_value(&value, &mut output)?;
        Ok(output)
    }

    pub fn identity(&self) -> Result<ManifestIdentity, ManifestError> {
        let canonical = self.canonical_json_bytes()?;
        let digest = Sha256::digest(canonical);
        let mut sha256 = String::with_capacity(64);
        for byte in digest {
            write!(&mut sha256, "{byte:02x}").expect("writing to a String cannot fail");
        }
        Ok(ManifestIdentity {
            profile: CANONICALIZATION_PROFILE,
            sha256,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ManifestCatalog {
    manifests: BTreeMap<String, ModuleManifest>,
    capability_owners: BTreeMap<(String, String), String>,
    event_owners: BTreeMap<(String, String), String>,
    object_owners: BTreeMap<String, String>,
}

impl ManifestCatalog {
    pub fn build(manifests: impl IntoIterator<Item = ModuleManifest>) -> Result<Self, ManifestError> {
        let mut issues = Vec::new();
        let mut by_module = BTreeMap::new();
        let mut capability_owners = BTreeMap::new();
        let mut event_owners = BTreeMap::new();
        let mut object_owners = BTreeMap::new();

        for manifest in manifests {
            issues.extend(manifest.validate());
            let module_id = manifest.module_id.clone();
            if by_module.contains_key(&module_id) {
                issues.push(ValidationIssue::new(
                    "duplicate_module",
                    "$.module_id",
                    format!("duplicate module_id {module_id}"),
                ));
                continue;
            }

            register_contract_owners(
                &manifest.provides.capabilities,
                &module_id,
                "capability",
                &mut capability_owners,
                &mut issues,
            );
            register_contract_owners(
                &manifest.provides.events,
                &module_id,
                "event",
                &mut event_owners,
                &mut issues,
            );
            for object in &manifest.provides.objects {
                if let Some(previous) = object_owners.insert(object.clone(), module_id.clone())
                    && previous != module_id
                {
                    issues.push(ValidationIssue::new(
                        "duplicate_object_owner",
                        "$.provides.objects",
                        format!("object {object} is already owned by {previous}"),
                    ));
                }
            }
            by_module.insert(module_id, manifest);
        }

        issues.extend(required_dependency_cycle_issues(&by_module));
        if !issues.is_empty() {
            return Err(ManifestError::Validation(issues));
        }

        Ok(Self {
            manifests: by_module,
            capability_owners,
            event_owners,
            object_owners,
        })
    }

    pub fn manifest(&self, module_id: &str) -> Option<&ModuleManifest> {
        self.manifests.get(module_id)
    }

    pub fn capability_owner(&self, id: &str, version: &str) -> Option<&str> {
        self.capability_owners
            .get(&(id.to_owned(), version.to_owned()))
            .map(String::as_str)
    }

    pub fn event_owner(&self, id: &str, version: &str) -> Option<&str> {
        self.event_owners
            .get(&(id.to_owned(), version.to_owned()))
            .map(String::as_str)
    }

    pub fn object_owner(&self, object_type: &str) -> Option<&str> {
        self.object_owners.get(object_type).map(String::as_str)
    }
}

fn register_contract_owners(
    contracts: &[VersionedContract],
    module_id: &str,
    kind: &str,
    owners: &mut BTreeMap<(String, String), String>,
    issues: &mut Vec<ValidationIssue>,
) {
    for contract in contracts {
        let key = (contract.id.clone(), contract.version.clone());
        if let Some(previous) = owners.insert(key, module_id.to_owned())
            && previous != module_id
        {
            issues.push(ValidationIssue::new(
                format!("duplicate_{kind}_provider").as_str(),
                format!("$.provides.{kind}s"),
                format!(
                    "{kind} {}@{} is already provided by {previous}",
                    contract.id, contract.version
                ),
            ));
        }
    }
}

fn required_dependency_cycle_issues(
    manifests: &BTreeMap<String, ModuleManifest>,
) -> Vec<ValidationIssue> {
    let known: BTreeSet<_> = manifests.keys().cloned().collect();
    let graph: BTreeMap<String, BTreeSet<String>> = manifests
        .iter()
        .map(|(module_id, manifest)| {
            let dependencies = manifest
                .dependencies
                .required
                .iter()
                .map(|dependency| dependency.module_id.clone())
                .filter(|dependency| known.contains(dependency))
                .collect();
            (module_id.clone(), dependencies)
        })
        .collect();

    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut stack = Vec::new();
    let mut issues = Vec::new();

    for node in graph.keys() {
        visit_dependency_node(
            node,
            &graph,
            &mut visiting,
            &mut visited,
            &mut stack,
            &mut issues,
        );
    }
    issues
}

fn visit_dependency_node(
    node: &str,
    graph: &BTreeMap<String, BTreeSet<String>>,
    visiting: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
    stack: &mut Vec<String>,
    issues: &mut Vec<ValidationIssue>,
) {
    if visited.contains(node) {
        return;
    }
    if visiting.contains(node) {
        let start = stack.iter().position(|entry| entry == node).unwrap_or(0);
        let mut cycle = stack[start..].to_vec();
        cycle.push(node.to_owned());
        issues.push(ValidationIssue::new(
            "dependency_cycle",
            "$.dependencies.required",
            format!("required module dependency cycle: {}", cycle.join(" -> ")),
        ));
        return;
    }

    visiting.insert(node.to_owned());
    stack.push(node.to_owned());
    if let Some(dependencies) = graph.get(node) {
        for dependency in dependencies {
            visit_dependency_node(
                dependency,
                graph,
                visiting,
                visited,
                stack,
                issues,
            );
        }
    }
    stack.pop();
    visiting.remove(node);
    visited.insert(node.to_owned());
}

fn validate_dependencies(manifest: &ModuleManifest, issues: &mut Vec<ValidationIssue>) {
    let required_ids: Vec<_> = manifest
        .dependencies
        .required
        .iter()
        .map(|dependency| dependency.module_id.clone())
        .collect();
    let optional_ids: Vec<_> = manifest
        .dependencies
        .optional
        .iter()
        .map(|dependency| dependency.module_id.clone())
        .collect();

    validate_dependency_list(
        &manifest.dependencies.required,
        "$.dependencies.required",
        issues,
    );
    validate_dependency_list(
        &manifest.dependencies.optional,
        "$.dependencies.optional",
        issues,
    );
    validate_unique_strings(&manifest.dependencies.conflicts, "$.dependencies.conflicts", issues);
    for (index, conflict) in manifest.dependencies.conflicts.iter().enumerate() {
        validate_module_id(conflict, &format!("$.dependencies.conflicts[{index}]"), issues);
    }

    let required: BTreeSet<_> = required_ids.iter().collect();
    let optional: BTreeSet<_> = optional_ids.iter().collect();
    for overlap in required.intersection(&optional) {
        issues.push(ValidationIssue::new(
            "dependency_overlap",
            "$.dependencies",
            format!("module {overlap} cannot be both required and optional"),
        ));
    }

    if required_ids
        .iter()
        .chain(optional_ids.iter())
        .chain(manifest.dependencies.conflicts.iter())
        .any(|module_id| module_id == &manifest.module_id)
    {
        issues.push(ValidationIssue::new(
            "self_dependency",
            "$.dependencies",
            "module must not depend on or conflict with itself",
        ));
    }
}

fn validate_dependency_list(
    dependencies: &[ModuleDependency],
    path: &str,
    issues: &mut Vec<ValidationIssue>,
) {
    let mut ids = BTreeSet::new();
    for (index, dependency) in dependencies.iter().enumerate() {
        validate_module_id(
            &dependency.module_id,
            &format!("{path}[{index}].module_id"),
            issues,
        );
        if VersionReq::parse(&dependency.version_range).is_err() {
            issues.push(ValidationIssue::new(
                "version_range",
                format!("{path}[{index}].version_range"),
                format!("invalid semantic version requirement {}", dependency.version_range),
            ));
        }
        if !ids.insert(dependency.module_id.clone()) {
            issues.push(ValidationIssue::new(
                "duplicate_dependency",
                format!("{path}[{index}].module_id"),
                format!("duplicate dependency {}", dependency.module_id),
            ));
        }
    }
}

fn validate_contracts(
    contracts: &[VersionedContract],
    path: &str,
    issues: &mut Vec<ValidationIssue>,
) {
    let mut keys = BTreeSet::new();
    for (index, contract) in contracts.iter().enumerate() {
        validate_namespaced_id(&contract.id, &format!("{path}[{index}].id"), issues);
        validate_semver(&contract.version, &format!("{path}[{index}].version"), issues);
        if !keys.insert((contract.id.clone(), contract.version.clone())) {
            issues.push(ValidationIssue::new(
                "duplicate_contract",
                format!("{path}[{index}]"),
                format!("duplicate contract {}@{}", contract.id, contract.version),
            ));
        }
    }
}

fn validate_namespaced_list(values: &[String], path: &str, issues: &mut Vec<ValidationIssue>) {
    validate_unique_strings(values, path, issues);
    for (index, value) in values.iter().enumerate() {
        validate_namespaced_id(value, &format!("{path}[{index}]"), issues);
    }
}

fn validate_unique_strings(values: &[String], path: &str, issues: &mut Vec<ValidationIssue>) {
    let mut seen = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        if !seen.insert(value) {
            issues.push(ValidationIssue::new(
                "duplicate_value",
                format!("{path}[{index}]"),
                format!("duplicate value {value}"),
            ));
        }
    }
}

fn duplicates<T: Ord + Copy>(values: impl IntoIterator<Item = T>) -> impl Iterator<Item = T> {
    let mut seen = BTreeSet::new();
    let mut duplicate = BTreeSet::new();
    for value in values {
        if !seen.insert(value) {
            duplicate.insert(value);
        }
    }
    duplicate.into_iter()
}

fn validate_ascii_id(value: &str, path: &str, issues: &mut Vec<ValidationIssue>) {
    if !valid_ascii_id(value) {
        issues.push(ValidationIssue::new(
            "ascii_id",
            path,
            "must match [a-z][a-z0-9_-]{1,63}",
        ));
    }
}

fn validate_module_id(value: &str, path: &str, issues: &mut Vec<ValidationIssue>) {
    if !valid_segmented_id(value, &['.', '-'], 120) {
        issues.push(ValidationIssue::new(
            "module_id",
            path,
            "must be a lowercase segmented module identifier",
        ));
    }
}

fn validate_namespaced_id(value: &str, path: &str, issues: &mut Vec<ValidationIssue>) {
    if !valid_segmented_id(value, &['.', '_', '-'], 180) {
        issues.push(ValidationIssue::new(
            "namespaced_id",
            path,
            "must be a lowercase segmented identifier",
        ));
    }
}

fn validate_semver(value: &str, path: &str, issues: &mut Vec<ValidationIssue>) {
    let _ = parse_semver(value, path, issues);
}

fn parse_semver(
    value: &str,
    path: &str,
    issues: &mut Vec<ValidationIssue>,
) -> Option<Version> {
    match Version::parse(value) {
        Ok(version) if value.len() <= 80 => Some(version),
        _ => {
            issues.push(ValidationIssue::new(
                "semver",
                path,
                format!("invalid semantic version {value}"),
            ));
            None
        }
    }
}

fn valid_ascii_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    (2..=64).contains(&bytes.len())
        && bytes[0].is_ascii_lowercase()
        && bytes[1..]
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-'))
}

fn valid_segmented_id(value: &str, separators: &[char], maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value.is_ascii()
        && value.chars().any(|character| separators.contains(&character))
        && value.split(|character| separators.contains(&character)).all(|segment| {
            !segment.is_empty()
                && segment.as_bytes()[0].is_ascii_lowercase()
                && segment
                    .as_bytes()
                    .iter()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}

fn valid_contact(value: &str) -> bool {
    value.len() <= 254
        && !value.contains(char::is_whitespace)
        && value
            .split_once('@')
            .is_some_and(|(local, domain)| !local.is_empty() && domain.contains('.') && !domain.ends_with('.'))
}

fn valid_codeowner(value: &str) -> bool {
    let Some(body) = value.strip_prefix('@') else {
        return false;
    };
    if body.is_empty() || body.matches('/').count() > 1 {
        return false;
    }
    body.split('/').all(|part| {
        !part.is_empty()
            && part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
    })
}

fn valid_path(value: &str, allow_colon: bool) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(byte, b'_' | b'.' | b'/' | b'-')
                || (allow_colon && byte == b':')
        })
}

fn valid_sha256_reference(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|digest| digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()))
}

fn valid_network_target(value: &str) -> bool {
    if value.contains("//") || value.contains('/') || value.contains('@') {
        return false;
    }
    let (host, port) = match value.rsplit_once(':') {
        Some((host, port)) if !port.is_empty() && port.bytes().all(|byte| byte.is_ascii_digit()) => {
            let Ok(port) = port.parse::<u16>() else {
                return false;
            };
            if port == 0 {
                return false;
            }
            (host, Some(port))
        }
        Some(_) => return false,
        None => (value, None),
    };
    let _ = port;
    let labels: Vec<_> = host.split('.').collect();
    labels.len() >= 2
        && labels.last().is_some_and(|label| (2..=63).contains(&label.len()) && label.bytes().all(|byte| byte.is_ascii_alphabetic()))
        && labels.iter().all(|label| {
            !label.is_empty()
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}

fn write_canonical_value(value: &Value, output: &mut Vec<u8>) -> Result<(), ManifestError> {
    match value {
        Value::Null => output.extend_from_slice(b"null"),
        Value::Bool(true) => output.extend_from_slice(b"true"),
        Value::Bool(false) => output.extend_from_slice(b"false"),
        Value::String(value) => output.extend_from_slice(serde_json::to_string(value)?.as_bytes()),
        Value::Number(number) => {
            if let Some(value) = number.as_u64() {
                if value > MAX_SAFE_INTEGER {
                    return Err(ManifestError::Canonical(format!(
                        "integer {value} exceeds the safe canonical range"
                    )));
                }
                output.extend_from_slice(value.to_string().as_bytes());
            } else if let Some(value) = number.as_i64() {
                if value.unsigned_abs() > MAX_SAFE_INTEGER {
                    return Err(ManifestError::Canonical(format!(
                        "integer {value} exceeds the safe canonical range"
                    )));
                }
                output.extend_from_slice(value.to_string().as_bytes());
            } else {
                return Err(ManifestError::Canonical(
                    "floating-point values are forbidden by crm.cjson/v1".to_owned(),
                ));
            }
        }
        Value::Array(values) => {
            output.push(b'[');
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    output.push(b',');
                }
                write_canonical_value(value, output)?;
            }
            output.push(b']');
        }
        Value::Object(values) => {
            let mut keys: Vec<_> = values.keys().collect();
            if keys.iter().any(|key| !key.is_ascii()) {
                return Err(ManifestError::Canonical(
                    "canonical object keys must be ASCII".to_owned(),
                ));
            }
            keys.sort_unstable();
            output.push(b'{');
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    output.push(b',');
                }
                output.extend_from_slice(serde_json::to_string(key)?.as_bytes());
                output.push(b':');
                write_canonical_value(&values[key], output)?;
            }
            output.push(b'}');
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SALES_MANIFEST: &str = r#"{
      "schema_version":"crm.module/v1",
      "module_id":"crm.sales",
      "version":"0.1.0",
      "display_name":"CRM Sales",
      "description":"Expert sales domain module owning leads, deals and quotes.",
      "owner":{"team":"sales-platform","contact":"crm-owner@example.com","codeowners":["@iamaman11"]},
      "runtime":{"kind":"in_process","entrypoint":"modules/crm-sales"},
      "platform":{"minimum_version":"0.1.0"},
      "dependencies":{"required":[],"optional":[],"conflicts":[]},
      "provides":{
        "capabilities":[
          {"id":"sales.deal.create","version":"1.0.0"},
          {"id":"sales.deal.update","version":"1.0.0"},
          {"id":"sales.deal.advance_stage","version":"1.0.0"}
        ],
        "events":[
          {"id":"sales.deal.created","version":"1.0.0"},
          {"id":"sales.deal.updated","version":"1.0.0"},
          {"id":"sales.deal.stage_changed","version":"1.0.0"}
        ],
        "objects":["sales.lead","sales.deal","sales.quote"],
        "ui_extensions":[
          {"id":"sales.pipeline.navigation","type":"navigation.item","version":"1.0.0","permission":"sales.deal.read","fallback":"hide"},
          {"id":"sales.deal.summary","type":"record_page.panel","version":"1.0.0","permission":"sales.deal.read","fallback":"readonly"}
        ]
      },
      "consumes":{"capabilities":[],"events":[]},
      "storage":{"record_types":["sales.lead","sales.deal","sales.quote"],"private_state_namespaces":["crm.sales"]},
      "security":{"data_classes":["internal","confidential","personal"],"network_egress":[],"secret_handles":[]},
      "quotas":{"maximum_bulk_records":10000,"maximum_workflow_actions_per_transaction":100},
      "lifecycle":{"upgrade_policy":"manual","rollback_policy":"supported","uninstall_policy":"retain_business_records","migrations_path":"modules/crm-sales/migrations","retained_record_types":["sales.lead","sales.deal","sales.quote"]}
    }"#;

    #[test]
    fn parses_and_matches_authoring_digest() {
        let manifest = ModuleManifest::from_normalized_json(SALES_MANIFEST).expect("valid manifest");
        let identity = manifest.identity().expect("canonical identity");
        assert_eq!(identity.profile, CANONICALIZATION_PROFILE);
        assert_eq!(
            identity.sha256,
            "a33cc97534dd9ee0f85dc163b499b126622eb931ca037a302114588f7a997f0f"
        );
    }

    #[test]
    fn canonical_identity_ignores_json_object_input_order() {
        let manifest = ModuleManifest::from_normalized_json(SALES_MANIFEST).expect("valid manifest");
        let pretty = serde_json::to_string_pretty(&manifest).expect("serialize manifest");
        let reparsed = ModuleManifest::from_normalized_json(&pretty).expect("reparse manifest");
        assert_eq!(manifest.identity().unwrap(), reparsed.identity().unwrap());
    }

    #[test]
    fn rejects_unknown_fields() {
        let invalid = SALES_MANIFEST.replacen(
            "\"schema_version\":\"crm.module/v1\"",
            "\"schema_version\":\"crm.module/v1\",\"unknown\":true",
            1,
        );
        assert!(matches!(
            ModuleManifest::from_normalized_json(&invalid),
            Err(ManifestError::Json(_))
        ));
    }

    #[test]
    fn rejects_self_dependency() {
        let mut manifest = ModuleManifest::from_normalized_json(SALES_MANIFEST).unwrap();
        manifest.dependencies.required.push(ModuleDependency {
            module_id: manifest.module_id.clone(),
            version_range: "^0.1".to_owned(),
        });
        assert!(manifest.validate().iter().any(|issue| issue.code == "self_dependency"));
    }

    #[test]
    fn catalog_rejects_duplicate_object_ownership() {
        let first = ModuleManifest::from_normalized_json(SALES_MANIFEST).unwrap();
        let mut second = first.clone();
        second.module_id = "crm.partner".to_owned();
        second.storage.private_state_namespaces = vec!["crm.partner".to_owned()];
        assert!(matches!(
            ManifestCatalog::build([first, second]),
            Err(ManifestError::Validation(issues))
                if issues.iter().any(|issue| issue.code == "duplicate_object_owner")
        ));
    }

    #[test]
    fn catalog_rejects_required_dependency_cycle() {
        let mut first = ModuleManifest::from_normalized_json(SALES_MANIFEST).unwrap();
        first.dependencies.required.push(ModuleDependency {
            module_id: "crm.activities".to_owned(),
            version_range: "^0.1".to_owned(),
        });

        let mut second = first.clone();
        second.module_id = "crm.activities".to_owned();
        second.dependencies.required = vec![ModuleDependency {
            module_id: "crm.sales".to_owned(),
            version_range: "^0.1".to_owned(),
        }];
        second.provides.capabilities.clear();
        second.provides.events.clear();
        second.provides.objects.clear();
        second.storage.record_types.clear();
        second.lifecycle.retained_record_types.clear();
        second.storage.private_state_namespaces = vec!["crm.activities".to_owned()];

        assert!(matches!(
            ManifestCatalog::build([first, second]),
            Err(ManifestError::Validation(issues))
                if issues.iter().any(|issue| issue.code == "dependency_cycle")
        ));
    }
}

use crate::state::{InstallationStatus, ModuleCoordinate, ModuleInstallation, TransitionError};
use crm_module_manifest::{
    ManifestError, ManifestIdentity, ModuleDependency, ModuleManifest, UninstallPolicy,
};
use crm_module_sdk::{IdentifierError, ModuleId, TenantId};
use semver::{Version, VersionReq};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryErrorCode {
    InvalidManifest,
    InvalidIdentifier,
    InvalidVersion,
    PlatformIncompatible,
    ImmutableVersionConflict,
    ModuleNotPublished,
    MissingDependency,
    DependencyConflict,
    DependencyCycle,
    InstallationNotFound,
    AlreadyInstalled,
    DependencyNotActive,
    UninstallBlocked,
    TransitionRejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryError {
    pub code: RegistryErrorCode,
    pub message: String,
    pub blockers: Vec<String>,
}

impl RegistryError {
    fn new(code: RegistryErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            blockers: Vec::new(),
        }
    }

    fn blocked(code: RegistryErrorCode, message: impl Into<String>, blockers: Vec<String>) -> Self {
        Self {
            code,
            message: message.into(),
            blockers,
        }
    }
}

impl fmt::Display for RegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for RegistryError {}

impl From<ManifestError> for RegistryError {
    fn from(value: ManifestError) -> Self {
        Self::new(RegistryErrorCode::InvalidManifest, value.to_string())
    }
}

impl From<IdentifierError> for RegistryError {
    fn from(value: IdentifierError) -> Self {
        Self::new(RegistryErrorCode::InvalidIdentifier, value.to_string())
    }
}

impl From<TransitionError> for RegistryError {
    fn from(value: TransitionError) -> Self {
        Self::new(RegistryErrorCode::TransitionRejected, value.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct PublishedModuleVersion {
    pub coordinate: ModuleCoordinate,
    pub manifest: ModuleManifest,
    pub identity: ManifestIdentity,
    pub published_at_unix_nanos: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DependencyResolution {
    pub installation_order: Vec<ModuleCoordinate>,
    pub already_satisfied: Vec<ModuleCoordinate>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImpactReport {
    pub blockers: Vec<String>,
    pub warnings: Vec<String>,
    pub affected_modules: Vec<ModuleCoordinate>,
    pub requires_approval: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishOutcome {
    pub coordinate: ModuleCoordinate,
    pub identity: ManifestIdentity,
    pub already_published: bool,
}

#[derive(Debug)]
pub struct ModuleRegistry {
    platform_version: Version,
    published: BTreeMap<ModuleId, BTreeMap<Version, PublishedModuleVersion>>,
    installations: BTreeMap<(TenantId, ModuleId), ModuleInstallation>,
    next_install_sequence: u64,
}

impl ModuleRegistry {
    pub fn new(platform_version: Version) -> Self {
        Self {
            platform_version,
            published: BTreeMap::new(),
            installations: BTreeMap::new(),
            next_install_sequence: 1,
        }
    }

    pub fn publish(
        &mut self,
        manifest: ModuleManifest,
        published_at_unix_nanos: i64,
    ) -> Result<PublishOutcome, RegistryError> {
        manifest.validate_or_error()?;
        let module_id = ModuleId::try_new(manifest.module_id.clone())?;
        let version = parse_version(&manifest.version, "module version")?;
        let identity = manifest.identity()?;
        let coordinate = ModuleCoordinate::new(module_id.clone(), version.clone());
        let versions = self.published.entry(module_id).or_default();

        if let Some(existing) = versions.get(&version) {
            if existing.identity == identity {
                return Ok(PublishOutcome {
                    coordinate,
                    identity,
                    already_published: true,
                });
            }
            return Err(RegistryError::new(
                RegistryErrorCode::ImmutableVersionConflict,
                format!(
                    "published coordinate {coordinate} already exists with a different manifest digest"
                ),
            ));
        }

        versions.insert(
            version,
            PublishedModuleVersion {
                coordinate: coordinate.clone(),
                manifest,
                identity: identity.clone(),
                published_at_unix_nanos,
            },
        );
        Ok(PublishOutcome {
            coordinate,
            identity,
            already_published: false,
        })
    }

    pub fn published_version(
        &self,
        coordinate: &ModuleCoordinate,
    ) -> Option<&PublishedModuleVersion> {
        self.published
            .get(&coordinate.module_id)
            .and_then(|versions| versions.get(&coordinate.version))
    }

    pub fn installation(
        &self,
        tenant_id: &TenantId,
        module_id: &ModuleId,
    ) -> Option<&ModuleInstallation> {
        self.installations
            .get(&(tenant_id.clone(), module_id.clone()))
    }

    pub fn resolve_dependencies(
        &self,
        tenant_id: &TenantId,
        target: &ModuleCoordinate,
    ) -> Result<DependencyResolution, RegistryError> {
        self.ensure_published_and_compatible(target)?;
        let mut state = ResolutionState::default();
        self.resolve_coordinate(tenant_id, target, None, &mut state)?;
        state.resolution.already_satisfied.sort();
        state.resolution.already_satisfied.dedup();
        Ok(state.resolution)
    }

    pub fn install(
        &mut self,
        tenant_id: TenantId,
        target: &ModuleCoordinate,
        grant_set_digests: &BTreeMap<ModuleId, [u8; 32]>,
        now_unix_nanos: i64,
    ) -> Result<Vec<ModuleInstallation>, RegistryError> {
        let resolution = self.resolve_dependencies(&tenant_id, target)?;
        let mut installed = Vec::new();

        for coordinate in resolution.installation_order {
            let key = (tenant_id.clone(), coordinate.module_id.clone());
            if let Some(existing) = self.installations.get(&key) {
                if existing.current == coordinate {
                    continue;
                }
                return Err(RegistryError::new(
                    RegistryErrorCode::AlreadyInstalled,
                    format!(
                        "{} is already installed at {}; use upgrade instead",
                        coordinate.module_id, existing.current.version
                    ),
                ));
            }
            let digest = grant_set_digests
                .get(&coordinate.module_id)
                .ok_or_else(|| {
                    RegistryError::new(
                        RegistryErrorCode::InvalidManifest,
                        format!(
                            "missing approved grant-set digest for {}",
                            coordinate.module_id
                        ),
                    )
                })?;
            let install_id = format!("module-install-{}", self.next_install_sequence);
            self.next_install_sequence += 1;
            let installation = ModuleInstallation::installed(
                install_id,
                tenant_id.clone(),
                coordinate,
                *digest,
                now_unix_nanos,
            );
            self.installations.insert(key, installation.clone());
            installed.push(installation);
        }
        Ok(installed)
    }

    pub fn activate(
        &mut self,
        tenant_id: &TenantId,
        module_id: &ModuleId,
        expected_generation: u64,
        now_unix_nanos: i64,
    ) -> Result<&ModuleInstallation, RegistryError> {
        self.ensure_required_dependencies_active(tenant_id, module_id)?;
        let installation = self.installation_mut(tenant_id, module_id)?;
        installation.activate(expected_generation, now_unix_nanos)?;
        Ok(installation)
    }

    pub fn suspend(
        &mut self,
        tenant_id: &TenantId,
        module_id: &ModuleId,
        expected_generation: u64,
        now_unix_nanos: i64,
    ) -> Result<&ModuleInstallation, RegistryError> {
        let blockers = self.active_dependents(tenant_id, module_id)?;
        if !blockers.is_empty() {
            return Err(RegistryError::blocked(
                RegistryErrorCode::DependencyConflict,
                format!("cannot suspend {module_id} while active modules depend on it"),
                blockers,
            ));
        }
        let installation = self.installation_mut(tenant_id, module_id)?;
        installation.suspend(expected_generation, now_unix_nanos)?;
        Ok(installation)
    }

    pub fn begin_upgrade(
        &mut self,
        tenant_id: &TenantId,
        target: ModuleCoordinate,
        expected_generation: u64,
        now_unix_nanos: i64,
    ) -> Result<&ModuleInstallation, RegistryError> {
        self.ensure_published_and_compatible(&target)?;
        let resolution = self.resolve_dependencies(tenant_id, &target)?;
        let missing: Vec<_> = resolution
            .installation_order
            .iter()
            .filter(|coordinate| coordinate.module_id != target.module_id)
            .map(ToString::to_string)
            .collect();
        if !missing.is_empty() {
            return Err(RegistryError::blocked(
                RegistryErrorCode::MissingDependency,
                "upgrade requires dependency installation or upgrade first",
                missing,
            ));
        }
        self.ensure_required_dependencies_active_for_coordinate(tenant_id, &target)?;
        let installation = self.installation_mut(tenant_id, &target.module_id)?;
        installation.begin_upgrade(expected_generation, target, now_unix_nanos)?;
        Ok(installation)
    }

    pub fn complete_upgrade(
        &mut self,
        tenant_id: &TenantId,
        module_id: &ModuleId,
        expected_generation: u64,
        now_unix_nanos: i64,
    ) -> Result<&ModuleInstallation, RegistryError> {
        let installation = self.installation_mut(tenant_id, module_id)?;
        installation.complete_upgrade(expected_generation, now_unix_nanos)?;
        Ok(installation)
    }

    pub fn fail_operation(
        &mut self,
        tenant_id: &TenantId,
        module_id: &ModuleId,
        expected_generation: u64,
        failure_code: impl Into<String>,
        now_unix_nanos: i64,
    ) -> Result<&ModuleInstallation, RegistryError> {
        let installation = self.installation_mut(tenant_id, module_id)?;
        installation.fail(expected_generation, failure_code, now_unix_nanos)?;
        Ok(installation)
    }

    pub fn begin_rollback(
        &mut self,
        tenant_id: &TenantId,
        module_id: &ModuleId,
        expected_generation: u64,
        now_unix_nanos: i64,
    ) -> Result<&ModuleInstallation, RegistryError> {
        let installation = self.installation_mut(tenant_id, module_id)?;
        installation.begin_rollback(expected_generation, now_unix_nanos)?;
        Ok(installation)
    }

    pub fn complete_rollback(
        &mut self,
        tenant_id: &TenantId,
        module_id: &ModuleId,
        expected_generation: u64,
        now_unix_nanos: i64,
    ) -> Result<&ModuleInstallation, RegistryError> {
        let installation = self.installation_mut(tenant_id, module_id)?;
        installation.complete_rollback(expected_generation, now_unix_nanos)?;
        Ok(installation)
    }

    pub fn uninstall_impact(
        &self,
        tenant_id: &TenantId,
        module_id: &ModuleId,
    ) -> Result<ImpactReport, RegistryError> {
        let installation = self.installation(tenant_id, module_id).ok_or_else(|| {
            RegistryError::new(
                RegistryErrorCode::InstallationNotFound,
                format!("module {module_id} is not installed for tenant {tenant_id}"),
            )
        })?;
        let published = self
            .published_version(&installation.current)
            .ok_or_else(|| {
                RegistryError::new(
                    RegistryErrorCode::ModuleNotPublished,
                    format!(
                        "installed version {} is not published",
                        installation.current
                    ),
                )
            })?;
        let mut report = ImpactReport {
            blockers: self.installed_dependents(tenant_id, module_id)?,
            ..ImpactReport::default()
        };
        report.affected_modules.push(installation.current.clone());
        if installation.status == InstallationStatus::Active {
            report
                .blockers
                .push("module must be suspended before uninstall".to_owned());
        }
        if published.manifest.lifecycle.uninstall_policy == UninstallPolicy::BlockedWhileReferenced
            && !report.blockers.is_empty()
        {
            report.requires_approval = true;
        }
        if !published
            .manifest
            .lifecycle
            .retained_record_types
            .is_empty()
        {
            report.warnings.push(format!(
                "business records will be retained: {}",
                published
                    .manifest
                    .lifecycle
                    .retained_record_types
                    .join(", ")
            ));
            report.requires_approval = true;
        }
        Ok(report)
    }

    pub fn begin_uninstall(
        &mut self,
        tenant_id: &TenantId,
        module_id: &ModuleId,
        expected_generation: u64,
        now_unix_nanos: i64,
    ) -> Result<&ModuleInstallation, RegistryError> {
        let report = self.uninstall_impact(tenant_id, module_id)?;
        if !report.blockers.is_empty() {
            return Err(RegistryError::blocked(
                RegistryErrorCode::UninstallBlocked,
                format!("cannot uninstall {module_id}"),
                report.blockers,
            ));
        }
        let installation = self.installation_mut(tenant_id, module_id)?;
        installation.begin_uninstall(expected_generation, now_unix_nanos)?;
        Ok(installation)
    }

    pub fn complete_uninstall(
        &mut self,
        tenant_id: &TenantId,
        module_id: &ModuleId,
        expected_generation: u64,
    ) -> Result<ModuleInstallation, RegistryError> {
        let key = (tenant_id.clone(), module_id.clone());
        let installation = self.installations.get(&key).ok_or_else(|| {
            RegistryError::new(
                RegistryErrorCode::InstallationNotFound,
                format!("module {module_id} is not installed for tenant {tenant_id}"),
            )
        })?;
        if installation.generation != expected_generation {
            return Err(RegistryError::new(
                RegistryErrorCode::TransitionRejected,
                format!(
                    "expected generation {expected_generation}, found {}",
                    installation.generation
                ),
            ));
        }
        installation.ensure_uninstalling()?;
        self.installations.remove(&key).ok_or_else(|| {
            RegistryError::new(
                RegistryErrorCode::InstallationNotFound,
                "installation disappeared during uninstall",
            )
        })
    }

    fn resolve_coordinate(
        &self,
        tenant_id: &TenantId,
        coordinate: &ModuleCoordinate,
        requirement: Option<&VersionReq>,
        state: &mut ResolutionState,
    ) -> Result<(), RegistryError> {
        if let Some(requirement) = requirement
            && !requirement.matches(&coordinate.version)
        {
            return Err(RegistryError::new(
                RegistryErrorCode::DependencyConflict,
                format!("selected {coordinate} does not satisfy {requirement}"),
            ));
        }
        if state.visited.contains(&coordinate.module_id) {
            let selected = state.selected.get(&coordinate.module_id).ok_or_else(|| {
                RegistryError::new(
                    RegistryErrorCode::DependencyConflict,
                    "dependency resolver lost a selected module",
                )
            })?;
            if selected != &coordinate.version {
                return Err(RegistryError::new(
                    RegistryErrorCode::DependencyConflict,
                    format!(
                        "conflicting versions selected for {}: {} and {}",
                        coordinate.module_id, selected, coordinate.version
                    ),
                ));
            }
            return Ok(());
        }
        if !state.visiting.insert(coordinate.module_id.clone()) {
            let mut cycle: Vec<_> = state.stack.iter().map(ToString::to_string).collect();
            cycle.push(coordinate.module_id.to_string());
            return Err(RegistryError::blocked(
                RegistryErrorCode::DependencyCycle,
                "module dependency cycle detected",
                vec![cycle.join(" -> ")],
            ));
        }
        state.stack.push(coordinate.module_id.clone());
        state
            .selected
            .insert(coordinate.module_id.clone(), coordinate.version.clone());

        let published = self.ensure_published_and_compatible(coordinate)?;
        self.ensure_manifest_conflicts_absent(tenant_id, &published.manifest, &state.selected)?;

        for dependency in &published.manifest.dependencies.required {
            let requirement = parse_requirement(dependency)?;
            let selected = self.select_dependency(tenant_id, dependency, &requirement)?;
            self.resolve_coordinate(tenant_id, &selected, Some(&requirement), state)?;
        }
        for dependency in &published.manifest.dependencies.optional {
            let requirement = parse_requirement(dependency)?;
            if let Some(installed) =
                self.installation(tenant_id, &ModuleId::try_new(dependency.module_id.clone())?)
                && requirement.matches(&installed.current.version)
            {
                state
                    .resolution
                    .already_satisfied
                    .push(installed.current.clone());
            } else {
                state.resolution.warnings.push(format!(
                    "optional dependency {} {} was not selected",
                    dependency.module_id, dependency.version_range
                ));
            }
        }

        state.stack.pop();
        state.visiting.remove(&coordinate.module_id);
        state.visited.insert(coordinate.module_id.clone());

        match self.installation(tenant_id, &coordinate.module_id) {
            Some(installed) if installed.current == *coordinate => {
                state.resolution.already_satisfied.push(coordinate.clone());
            }
            _ => state.resolution.installation_order.push(coordinate.clone()),
        }
        Ok(())
    }

    fn select_dependency(
        &self,
        tenant_id: &TenantId,
        dependency: &ModuleDependency,
        requirement: &VersionReq,
    ) -> Result<ModuleCoordinate, RegistryError> {
        let module_id = ModuleId::try_new(dependency.module_id.clone())?;
        if let Some(installed) = self.installation(tenant_id, &module_id)
            && requirement.matches(&installed.current.version)
        {
            self.ensure_published_and_compatible(&installed.current)?;
            return Ok(installed.current.clone());
        }

        let versions = self.published.get(&module_id).ok_or_else(|| {
            RegistryError::new(
                RegistryErrorCode::MissingDependency,
                format!("required module {module_id} has no published versions"),
            )
        })?;
        versions
            .iter()
            .rev()
            .find_map(|(version, published)| {
                (requirement.matches(version)
                    && self.is_platform_compatible(&published.manifest).is_ok())
                .then(|| published.coordinate.clone())
            })
            .ok_or_else(|| {
                RegistryError::new(
                    RegistryErrorCode::MissingDependency,
                    format!(
                        "no compatible version of {module_id} satisfies {}",
                        dependency.version_range
                    ),
                )
            })
    }

    fn ensure_manifest_conflicts_absent(
        &self,
        tenant_id: &TenantId,
        manifest: &ModuleManifest,
        selected: &BTreeMap<ModuleId, Version>,
    ) -> Result<(), RegistryError> {
        let mut conflicts = Vec::new();
        for conflict in &manifest.dependencies.conflicts {
            let conflict_id = ModuleId::try_new(conflict.clone())?;
            if selected.contains_key(&conflict_id)
                || self.installation(tenant_id, &conflict_id).is_some()
            {
                conflicts.push(format!(
                    "{} conflicts with {conflict_id}",
                    manifest.module_id
                ));
            }
        }
        if conflicts.is_empty() {
            Ok(())
        } else {
            Err(RegistryError::blocked(
                RegistryErrorCode::DependencyConflict,
                "module conflict detected",
                conflicts,
            ))
        }
    }

    fn ensure_published_and_compatible(
        &self,
        coordinate: &ModuleCoordinate,
    ) -> Result<&PublishedModuleVersion, RegistryError> {
        let published = self.published_version(coordinate).ok_or_else(|| {
            RegistryError::new(
                RegistryErrorCode::ModuleNotPublished,
                format!("module version {coordinate} is not published"),
            )
        })?;
        self.is_platform_compatible(&published.manifest)?;
        Ok(published)
    }

    fn is_platform_compatible(&self, manifest: &ModuleManifest) -> Result<(), RegistryError> {
        let minimum = parse_version(
            &manifest.platform.minimum_version,
            "minimum platform version",
        )?;
        if self.platform_version < minimum {
            return Err(RegistryError::new(
                RegistryErrorCode::PlatformIncompatible,
                format!(
                    "{} requires platform >= {}, current platform is {}",
                    manifest.module_id, minimum, self.platform_version
                ),
            ));
        }
        if let Some(maximum) = &manifest.platform.maximum_exclusive_version {
            let maximum = parse_version(maximum, "maximum platform version")?;
            if self.platform_version >= maximum {
                return Err(RegistryError::new(
                    RegistryErrorCode::PlatformIncompatible,
                    format!(
                        "{} requires platform < {}, current platform is {}",
                        manifest.module_id, maximum, self.platform_version
                    ),
                ));
            }
        }
        Ok(())
    }

    fn ensure_required_dependencies_active(
        &self,
        tenant_id: &TenantId,
        module_id: &ModuleId,
    ) -> Result<(), RegistryError> {
        let installation = self.installation(tenant_id, module_id).ok_or_else(|| {
            RegistryError::new(
                RegistryErrorCode::InstallationNotFound,
                format!("module {module_id} is not installed for tenant {tenant_id}"),
            )
        })?;
        self.ensure_required_dependencies_active_for_coordinate(tenant_id, &installation.current)
    }

    fn ensure_required_dependencies_active_for_coordinate(
        &self,
        tenant_id: &TenantId,
        coordinate: &ModuleCoordinate,
    ) -> Result<(), RegistryError> {
        let published = self.ensure_published_and_compatible(coordinate)?;
        let mut blockers = Vec::new();
        for dependency in &published.manifest.dependencies.required {
            let module_id = ModuleId::try_new(dependency.module_id.clone())?;
            let requirement = parse_requirement(dependency)?;
            match self.installation(tenant_id, &module_id) {
                Some(installed)
                    if installed.status == InstallationStatus::Active
                        && requirement.matches(&installed.current.version) => {}
                Some(installed) => blockers.push(format!(
                    "{} requires active {} {}, found {} in {:?}",
                    coordinate.module_id,
                    module_id,
                    dependency.version_range,
                    installed.current.version,
                    installed.status
                )),
                None => blockers.push(format!(
                    "{} requires active {} {}",
                    coordinate.module_id, module_id, dependency.version_range
                )),
            }
        }
        if blockers.is_empty() {
            Ok(())
        } else {
            Err(RegistryError::blocked(
                RegistryErrorCode::DependencyNotActive,
                "required dependencies are not active",
                blockers,
            ))
        }
    }

    fn active_dependents(
        &self,
        tenant_id: &TenantId,
        module_id: &ModuleId,
    ) -> Result<Vec<String>, RegistryError> {
        self.dependents(tenant_id, module_id, true)
    }

    fn installed_dependents(
        &self,
        tenant_id: &TenantId,
        module_id: &ModuleId,
    ) -> Result<Vec<String>, RegistryError> {
        self.dependents(tenant_id, module_id, false)
    }

    fn dependents(
        &self,
        tenant_id: &TenantId,
        module_id: &ModuleId,
        active_only: bool,
    ) -> Result<Vec<String>, RegistryError> {
        let mut dependents = Vec::new();
        for ((installed_tenant, _), installation) in &self.installations {
            if installed_tenant != tenant_id
                || (active_only && installation.status != InstallationStatus::Active)
            {
                continue;
            }
            let published = self.ensure_published_and_compatible(&installation.current)?;
            if published
                .manifest
                .dependencies
                .required
                .iter()
                .any(|dependency| dependency.module_id == module_id.as_str())
            {
                dependents.push(format!(
                    "{} depends on {module_id}",
                    installation.current.module_id
                ));
            }
        }
        dependents.sort();
        dependents.dedup();
        Ok(dependents)
    }

    fn installation_mut(
        &mut self,
        tenant_id: &TenantId,
        module_id: &ModuleId,
    ) -> Result<&mut ModuleInstallation, RegistryError> {
        self.installations
            .get_mut(&(tenant_id.clone(), module_id.clone()))
            .ok_or_else(|| {
                RegistryError::new(
                    RegistryErrorCode::InstallationNotFound,
                    format!("module {module_id} is not installed for tenant {tenant_id}"),
                )
            })
    }
}

#[derive(Debug, Default)]
struct ResolutionState {
    visiting: BTreeSet<ModuleId>,
    visited: BTreeSet<ModuleId>,
    stack: Vec<ModuleId>,
    selected: BTreeMap<ModuleId, Version>,
    resolution: DependencyResolution,
}

fn parse_version(value: &str, label: &str) -> Result<Version, RegistryError> {
    Version::parse(value).map_err(|error| {
        RegistryError::new(
            RegistryErrorCode::InvalidVersion,
            format!("invalid {label} {value}: {error}"),
        )
    })
}

fn parse_requirement(dependency: &ModuleDependency) -> Result<VersionReq, RegistryError> {
    VersionReq::parse(&dependency.version_range).map_err(|error| {
        RegistryError::new(
            RegistryErrorCode::InvalidVersion,
            format!(
                "invalid dependency requirement {} for {}: {error}",
                dependency.version_range, dependency.module_id
            ),
        )
    })
}

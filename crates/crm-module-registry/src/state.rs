use crm_module_sdk::{ModuleId, TenantId};
use semver::Version;
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ModuleCoordinate {
    pub module_id: ModuleId,
    pub version: Version,
}

impl ModuleCoordinate {
    pub fn new(module_id: ModuleId, version: Version) -> Self {
        Self { module_id, version }
    }
}

impl fmt::Display for ModuleCoordinate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}@{}", self.module_id, self.version)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StableInstallationStatus {
    Installed,
    Active,
    Suspended,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallationStatus {
    Installed,
    Active,
    Suspended,
    Upgrading,
    RollingBack,
    Uninstalling,
    Failed,
}

impl InstallationStatus {
    pub const fn is_stable(self) -> bool {
        matches!(Self::Installed | Self::Active | Self::Suspended, self)
    }

    pub const fn stable(self) -> Option<StableInstallationStatus> {
        match self {
            Self::Installed => Some(StableInstallationStatus::Installed),
            Self::Active => Some(StableInstallationStatus::Active),
            Self::Suspended => Some(StableInstallationStatus::Suspended),
            Self::Upgrading | Self::RollingBack | Self::Uninstalling | Self::Failed => None,
        }
    }
}

impl From<StableInstallationStatus> for InstallationStatus {
    fn from(value: StableInstallationStatus) -> Self {
        match value {
            StableInstallationStatus::Installed => Self::Installed,
            StableInstallationStatus::Active => Self::Active,
            StableInstallationStatus::Suspended => Self::Suspended,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionErrorCode {
    GenerationConflict,
    InvalidTransition,
    MissingPreviousVersion,
    MissingPendingVersion,
    SameVersion,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionError {
    pub code: TransitionErrorCode,
    pub message: String,
}

impl TransitionError {
    fn new(code: TransitionErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for TransitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for TransitionError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleInstallation {
    pub install_id: String,
    pub tenant_id: TenantId,
    pub current: ModuleCoordinate,
    pub status: InstallationStatus,
    pub previous: Option<ModuleCoordinate>,
    pub pending: Option<ModuleCoordinate>,
    pub generation: u64,
    pub failure_code: Option<String>,
    pub updated_at_unix_nanos: i64,
    pub grant_set_digest: [u8; 32],
    resume_status: Option<StableInstallationStatus>,
}

impl ModuleInstallation {
    pub fn installed(
        install_id: impl Into<String>,
        tenant_id: TenantId,
        current: ModuleCoordinate,
        grant_set_digest: [u8; 32],
        now_unix_nanos: i64,
    ) -> Self {
        Self {
            install_id: install_id.into(),
            tenant_id,
            current,
            status: InstallationStatus::Installed,
            previous: None,
            pending: None,
            generation: 1,
            failure_code: None,
            updated_at_unix_nanos: now_unix_nanos,
            grant_set_digest,
            resume_status: None,
        }
    }

    pub fn activate(
        &mut self,
        expected_generation: u64,
        now_unix_nanos: i64,
    ) -> Result<(), TransitionError> {
        self.ensure_generation(expected_generation)?;
        match self.status {
            InstallationStatus::Installed | InstallationStatus::Suspended => {
                self.status = InstallationStatus::Active;
                self.bump(now_unix_nanos);
                Ok(())
            }
            InstallationStatus::Active => Ok(()),
            status => Err(self.invalid_transition("activate", status)),
        }
    }

    pub fn suspend(
        &mut self,
        expected_generation: u64,
        now_unix_nanos: i64,
    ) -> Result<(), TransitionError> {
        self.ensure_generation(expected_generation)?;
        match self.status {
            InstallationStatus::Active => {
                self.status = InstallationStatus::Suspended;
                self.bump(now_unix_nanos);
                Ok(())
            }
            InstallationStatus::Suspended => Ok(()),
            status => Err(self.invalid_transition("suspend", status)),
        }
    }

    pub fn begin_upgrade(
        &mut self,
        expected_generation: u64,
        target: ModuleCoordinate,
        now_unix_nanos: i64,
    ) -> Result<(), TransitionError> {
        self.ensure_generation(expected_generation)?;
        let resume_status = self.status.stable().ok_or_else(|| {
            self.invalid_transition("begin upgrade", self.status)
        })?;
        if target == self.current {
            return Err(TransitionError::new(
                TransitionErrorCode::SameVersion,
                "upgrade target must differ from the current module version",
            ));
        }
        if target.module_id != self.current.module_id {
            return Err(TransitionError::new(
                TransitionErrorCode::InvalidTransition,
                "upgrade target must have the same module_id",
            ));
        }

        self.previous = Some(self.current.clone());
        self.pending = Some(target);
        self.resume_status = Some(resume_status);
        self.status = InstallationStatus::Upgrading;
        self.failure_code = None;
        self.bump(now_unix_nanos);
        Ok(())
    }

    pub fn complete_upgrade(
        &mut self,
        expected_generation: u64,
        now_unix_nanos: i64,
    ) -> Result<(), TransitionError> {
        self.ensure_generation(expected_generation)?;
        if self.status != InstallationStatus::Upgrading {
            return Err(self.invalid_transition("complete upgrade", self.status));
        }
        let pending = self.pending.take().ok_or_else(|| {
            TransitionError::new(
                TransitionErrorCode::MissingPendingVersion,
                "upgrade has no pending module version",
            )
        })?;
        let resume_status = self.resume_status.take().unwrap_or(StableInstallationStatus::Installed);
        self.current = pending;
        self.status = resume_status.into();
        self.failure_code = None;
        self.bump(now_unix_nanos);
        Ok(())
    }

    pub fn fail(
        &mut self,
        expected_generation: u64,
        failure_code: impl Into<String>,
        now_unix_nanos: i64,
    ) -> Result<(), TransitionError> {
        self.ensure_generation(expected_generation)?;
        if !matches!(
            self.status,
            InstallationStatus::Upgrading
                | InstallationStatus::RollingBack
                | InstallationStatus::Uninstalling
        ) {
            return Err(self.invalid_transition("fail operation", self.status));
        }
        self.status = InstallationStatus::Failed;
        self.failure_code = Some(failure_code.into());
        self.bump(now_unix_nanos);
        Ok(())
    }

    pub fn begin_rollback(
        &mut self,
        expected_generation: u64,
        now_unix_nanos: i64,
    ) -> Result<(), TransitionError> {
        self.ensure_generation(expected_generation)?;
        if !matches!(
            self.status,
            InstallationStatus::Installed
                | InstallationStatus::Active
                | InstallationStatus::Suspended
                | InstallationStatus::Failed
        ) {
            return Err(self.invalid_transition("begin rollback", self.status));
        }
        let previous = self.previous.clone().ok_or_else(|| {
            TransitionError::new(
                TransitionErrorCode::MissingPreviousVersion,
                "no previous module version is available for rollback",
            )
        })?;
        let resume_status = self
            .status
            .stable()
            .or(self.resume_status)
            .unwrap_or(StableInstallationStatus::Suspended);
        self.pending = Some(previous);
        self.resume_status = Some(resume_status);
        self.status = InstallationStatus::RollingBack;
        self.failure_code = None;
        self.bump(now_unix_nanos);
        Ok(())
    }

    pub fn complete_rollback(
        &mut self,
        expected_generation: u64,
        now_unix_nanos: i64,
    ) -> Result<(), TransitionError> {
        self.ensure_generation(expected_generation)?;
        if self.status != InstallationStatus::RollingBack {
            return Err(self.invalid_transition("complete rollback", self.status));
        }
        let target = self.pending.take().ok_or_else(|| {
            TransitionError::new(
                TransitionErrorCode::MissingPendingVersion,
                "rollback has no pending module version",
            )
        })?;
        let replaced = std::mem::replace(&mut self.current, target);
        self.previous = Some(replaced);
        self.status = self
            .resume_status
            .take()
            .unwrap_or(StableInstallationStatus::Suspended)
            .into();
        self.failure_code = None;
        self.bump(now_unix_nanos);
        Ok(())
    }

    pub fn begin_uninstall(
        &mut self,
        expected_generation: u64,
        now_unix_nanos: i64,
    ) -> Result<(), TransitionError> {
        self.ensure_generation(expected_generation)?;
        if !matches!(
            self.status,
            InstallationStatus::Installed
                | InstallationStatus::Suspended
                | InstallationStatus::Failed
        ) {
            return Err(self.invalid_transition("begin uninstall", self.status));
        }
        self.status = InstallationStatus::Uninstalling;
        self.failure_code = None;
        self.bump(now_unix_nanos);
        Ok(())
    }

    pub fn ensure_uninstalling(&self) -> Result<(), TransitionError> {
        if self.status == InstallationStatus::Uninstalling {
            Ok(())
        } else {
            Err(self.invalid_transition("complete uninstall", self.status))
        }
    }

    fn ensure_generation(&self, expected_generation: u64) -> Result<(), TransitionError> {
        if self.generation == expected_generation {
            Ok(())
        } else {
            Err(TransitionError::new(
                TransitionErrorCode::GenerationConflict,
                format!(
                    "expected generation {expected_generation}, found {}",
                    self.generation
                ),
            ))
        }
    }

    fn invalid_transition(
        &self,
        operation: &str,
        status: InstallationStatus,
    ) -> TransitionError {
        TransitionError::new(
            TransitionErrorCode::InvalidTransition,
            format!("cannot {operation} while installation is {status:?}"),
        )
    }

    fn bump(&mut self, now_unix_nanos: i64) {
        self.generation += 1;
        self.updated_at_unix_nanos = now_unix_nanos;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn coordinate(version: &str) -> ModuleCoordinate {
        ModuleCoordinate::new(
            ModuleId::try_new("crm.sales").unwrap(),
            Version::parse(version).unwrap(),
        )
    }

    fn installation() -> ModuleInstallation {
        ModuleInstallation::installed(
            "install-1",
            TenantId::try_new("tenant-a").unwrap(),
            coordinate("1.0.0"),
            [7; 32],
            1,
        )
    }

    #[test]
    fn generation_protects_transitions() {
        let mut installation = installation();
        let error = installation.activate(9, 2).unwrap_err();
        assert_eq!(error.code, TransitionErrorCode::GenerationConflict);
    }

    #[test]
    fn upgrade_and_rollback_preserve_versions() {
        let mut installation = installation();
        installation.activate(1, 2).unwrap();
        installation
            .begin_upgrade(2, coordinate("2.0.0"), 3)
            .unwrap();
        installation.complete_upgrade(3, 4).unwrap();
        assert_eq!(installation.current.version, Version::parse("2.0.0").unwrap());
        assert_eq!(installation.status, InstallationStatus::Active);

        installation.begin_rollback(4, 5).unwrap();
        installation.complete_rollback(5, 6).unwrap();
        assert_eq!(installation.current.version, Version::parse("1.0.0").unwrap());
        assert_eq!(installation.status, InstallationStatus::Active);
    }

    #[test]
    fn active_installation_cannot_be_uninstalled_directly() {
        let mut installation = installation();
        installation.activate(1, 2).unwrap();
        assert_eq!(
            installation.begin_uninstall(2, 3).unwrap_err().code,
            TransitionErrorCode::InvalidTransition
        );
    }
}

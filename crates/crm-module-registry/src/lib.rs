#![forbid(unsafe_code)]

//! Pure domain implementation of module publication, dependency resolution and
//! tenant installation lifecycle. Persistence and transport adapters are added
//! in later phases; this crate owns the deterministic rules.

mod registry;
mod state;

pub use registry::*;
pub use state::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crm_module_manifest::ModuleManifest;
    use crm_module_sdk::{ModuleId, TenantId};
    use semver::Version;
    use std::collections::BTreeMap;

    fn manifest(
        module_id: &str,
        version: &str,
        required: &[(&str, &str)],
        conflicts: &[&str],
    ) -> ModuleManifest {
        let required = required
            .iter()
            .map(|(module_id, version_range)| {
                format!(
                    r#"{{"module_id":"{module_id}","version_range":"{version_range}"}}"#
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let conflicts = conflicts
            .iter()
            .map(|module_id| format!(r#""{module_id}""#))
            .collect::<Vec<_>>()
            .join(",");
        let json = format!(
            r#"{{
                "schema_version":"crm.module/v1",
                "module_id":"{module_id}",
                "version":"{version}",
                "owner":{{"team":"platform","contact":"owner@example.com","codeowners":[]}},
                "runtime":{{"kind":"in_process","entrypoint":"modules/{module_id}"}},
                "platform":{{"minimum_version":"0.1.0"}},
                "dependencies":{{"required":[{required}],"optional":[],"conflicts":[{conflicts}]}},
                "provides":{{"capabilities":[],"events":[],"objects":[],"ui_extensions":[]}},
                "consumes":{{"capabilities":[],"events":[]}},
                "storage":{{"record_types":[],"private_state_namespaces":["{module_id}"]}},
                "security":{{"data_classes":[],"network_egress":[],"secret_handles":[]}},
                "lifecycle":{{
                    "upgrade_policy":"manual",
                    "rollback_policy":"supported",
                    "uninstall_policy":"blocked_while_referenced",
                    "retained_record_types":[]
                }}
            }}"#
        );
        ModuleManifest::from_normalized_json(&json).expect("test manifest must be valid")
    }

    fn coordinate(module_id: &str, version: &str) -> ModuleCoordinate {
        ModuleCoordinate::new(
            ModuleId::try_new(module_id).unwrap(),
            Version::parse(version).unwrap(),
        )
    }

    fn grants(module_ids: &[&str]) -> BTreeMap<ModuleId, [u8; 32]> {
        module_ids
            .iter()
            .enumerate()
            .map(|(index, module_id)| {
                (
                    ModuleId::try_new(*module_id).unwrap(),
                    [(index as u8) + 1; 32],
                )
            })
            .collect()
    }

    #[test]
    fn resolver_selects_highest_compatible_version_in_dependency_order() {
        let mut registry = ModuleRegistry::new(Version::parse("1.0.0").unwrap());
        registry
            .publish(manifest("crm.activities", "1.0.0", &[], &[]), 1)
            .unwrap();
        registry
            .publish(manifest("crm.activities", "1.5.0", &[], &[]), 2)
            .unwrap();
        registry
            .publish(manifest("crm.activities", "2.0.0", &[], &[]), 3)
            .unwrap();
        registry
            .publish(
                manifest(
                    "crm.sales",
                    "1.0.0",
                    &[("crm.activities", ">=1.0.0,<2.0.0")],
                    &[],
                ),
                4,
            )
            .unwrap();

        let resolution = registry
            .resolve_dependencies(
                &TenantId::try_new("tenant-a").unwrap(),
                &coordinate("crm.sales", "1.0.0"),
            )
            .unwrap();
        assert_eq!(
            resolution.installation_order,
            vec![
                coordinate("crm.activities", "1.5.0"),
                coordinate("crm.sales", "1.0.0")
            ]
        );
    }

    #[test]
    fn activation_requires_active_dependencies() {
        let tenant = TenantId::try_new("tenant-a").unwrap();
        let activities = ModuleId::try_new("crm.activities").unwrap();
        let sales = ModuleId::try_new("crm.sales").unwrap();
        let mut registry = ModuleRegistry::new(Version::parse("1.0.0").unwrap());
        registry
            .publish(manifest("crm.activities", "1.0.0", &[], &[]), 1)
            .unwrap();
        registry
            .publish(
                manifest(
                    "crm.sales",
                    "1.0.0",
                    &[("crm.activities", "^1.0")],
                    &[],
                ),
                2,
            )
            .unwrap();
        registry
            .install(
                tenant.clone(),
                &coordinate("crm.sales", "1.0.0"),
                &grants(&["crm.activities", "crm.sales"]),
                3,
            )
            .unwrap();

        let error = registry.activate(&tenant, &sales, 1, 4).unwrap_err();
        assert_eq!(error.code, RegistryErrorCode::DependencyNotActive);
        registry.activate(&tenant, &activities, 1, 5).unwrap();
        registry.activate(&tenant, &sales, 1, 6).unwrap();
        assert_eq!(
            registry.installation(&tenant, &sales).unwrap().status,
            InstallationStatus::Active
        );
    }

    #[test]
    fn publication_is_immutable_and_idempotent() {
        let mut registry = ModuleRegistry::new(Version::parse("1.0.0").unwrap());
        let original = manifest("crm.sales", "1.0.0", &[], &[]);
        let first = registry.publish(original.clone(), 1).unwrap();
        let second = registry.publish(original, 2).unwrap();
        assert!(!first.already_published);
        assert!(second.already_published);

        let mut changed = manifest("crm.sales", "1.0.0", &[], &[]);
        changed.description = Some("changed meaning".to_owned());
        assert_eq!(
            registry.publish(changed, 3).unwrap_err().code,
            RegistryErrorCode::ImmutableVersionConflict
        );
    }

    #[test]
    fn registry_upgrade_and_rollback_restore_previous_version() {
        let tenant = TenantId::try_new("tenant-a").unwrap();
        let sales = ModuleId::try_new("crm.sales").unwrap();
        let mut registry = ModuleRegistry::new(Version::parse("1.0.0").unwrap());
        registry
            .publish(manifest("crm.sales", "1.0.0", &[], &[]), 1)
            .unwrap();
        registry
            .publish(manifest("crm.sales", "2.0.0", &[], &[]), 2)
            .unwrap();
        registry
            .install(
                tenant.clone(),
                &coordinate("crm.sales", "1.0.0"),
                &grants(&["crm.sales"]),
                3,
            )
            .unwrap();
        registry.activate(&tenant, &sales, 1, 4).unwrap();
        registry
            .begin_upgrade(&tenant, coordinate("crm.sales", "2.0.0"), 2, 5)
            .unwrap();
        registry.complete_upgrade(&tenant, &sales, 3, 6).unwrap();
        assert_eq!(
            registry.installation(&tenant, &sales).unwrap().current.version,
            Version::parse("2.0.0").unwrap()
        );

        registry.begin_rollback(&tenant, &sales, 4, 7).unwrap();
        registry.complete_rollback(&tenant, &sales, 5, 8).unwrap();
        assert_eq!(
            registry.installation(&tenant, &sales).unwrap().current.version,
            Version::parse("1.0.0").unwrap()
        );
    }

    #[test]
    fn installed_dependents_block_uninstall() {
        let tenant = TenantId::try_new("tenant-a").unwrap();
        let activities = ModuleId::try_new("crm.activities").unwrap();
        let sales = ModuleId::try_new("crm.sales").unwrap();
        let mut registry = ModuleRegistry::new(Version::parse("1.0.0").unwrap());
        registry
            .publish(manifest("crm.activities", "1.0.0", &[], &[]), 1)
            .unwrap();
        registry
            .publish(
                manifest(
                    "crm.sales",
                    "1.0.0",
                    &[("crm.activities", "^1.0")],
                    &[],
                ),
                2,
            )
            .unwrap();
        registry
            .install(
                tenant.clone(),
                &coordinate("crm.sales", "1.0.0"),
                &grants(&["crm.activities", "crm.sales"]),
                3,
            )
            .unwrap();
        registry.activate(&tenant, &activities, 1, 4).unwrap();
        registry.activate(&tenant, &sales, 1, 5).unwrap();
        registry.suspend(&tenant, &sales, 2, 6).unwrap();
        registry.suspend(&tenant, &activities, 2, 7).unwrap();

        let impact = registry.uninstall_impact(&tenant, &activities).unwrap();
        assert!(impact.blockers.iter().any(|value| value.contains("crm.sales")));
        assert_eq!(
            registry
                .begin_uninstall(&tenant, &activities, 3, 8)
                .unwrap_err()
                .code,
            RegistryErrorCode::UninstallBlocked
        );
    }

    #[test]
    fn dependency_cycles_are_rejected() {
        let tenant = TenantId::try_new("tenant-a").unwrap();
        let mut registry = ModuleRegistry::new(Version::parse("1.0.0").unwrap());
        registry
            .publish(
                manifest("crm.alpha", "1.0.0", &[("crm.beta", "^1.0")], &[]),
                1,
            )
            .unwrap();
        registry
            .publish(
                manifest("crm.beta", "1.0.0", &[("crm.alpha", "^1.0")], &[]),
                2,
            )
            .unwrap();

        assert_eq!(
            registry
                .resolve_dependencies(&tenant, &coordinate("crm.alpha", "1.0.0"))
                .unwrap_err()
                .code,
            RegistryErrorCode::DependencyCycle
        );
    }
}

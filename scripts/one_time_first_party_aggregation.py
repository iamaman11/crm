#!/usr/bin/env python3
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def replace_exact(path: Path, old: str, new: str, count: int = 1) -> None:
    content = path.read_text(encoding="utf-8")
    if content.count(old) != count:
        raise SystemExit(
            f"expected {count} matches in {path}, found {content.count(old)}: {old!r}"
        )
    path.write_text(content.replace(old, new), encoding="utf-8")


root_cargo = ROOT / "Cargo.toml"
replace_exact(
    root_cargo,
    '  "crates/crm-application-composition",\n',
    '  "crates/crm-application-composition",\n'
    '  "crates/crm-first-party-modules",\n',
)

crate_root = ROOT / "crates/crm-first-party-modules"
(crate_root / "src").mkdir(parents=True, exist_ok=True)
(crate_root / "Cargo.toml").write_text(
    """[package]
name = "crm-first-party-modules"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
crm-application-composition = { path = "../crm-application-composition" }
crm-core-data = { path = "../crm-core-data" }
crm-customer-accounts-capability-composition = { path = "../crm-customer-accounts-capability-composition" }
crm-consents-capability-composition = { path = "../crm-consents-capability-composition" }
crm-module-sdk = { path = "../crm-module-sdk" }
crm-party-reference-composition = { path = "../crm-party-reference-composition" }
crm-query-runtime = { path = "../crm-query-runtime" }
""",
    encoding="utf-8",
)
(crate_root / "src/lib.rs").write_text(
    """#![forbid(unsafe_code)]

//! Mechanically narrow aggregation of proven first-party owner contributions.
//!
//! This crate contains no route catalog and no business dispatch. Exact routes
//! remain defined and built by owner packages; this boundary only combines
//! their contribution entry points for the generic process host.

use crm_application_composition::{
    ModuleActivationPort, ModuleContributionSet,
};
use crm_consents_capability_composition::{
    ConsentsProductionDependencies, build_contribution as build_consents_contribution,
};
use crm_core_data::PostgresDataStore;
use crm_customer_accounts_capability_composition::{
    CustomerAccountsProductionDependencies,
    build_contribution as build_customer_accounts_contribution,
};
use crm_module_sdk::SdkError;
use crm_party_reference_composition::PartyReferenceReader;
use crm_query_runtime::QueryVisibilityAuthorizer;
use std::sync::Arc;

#[derive(Clone)]
pub struct FirstPartyProductionDependencies {
    pub store: PostgresDataStore,
    pub parties: Arc<dyn PartyReferenceReader>,
    pub activation: Arc<dyn ModuleActivationPort>,
    pub visibility_authorizer: Arc<dyn QueryVisibilityAuthorizer>,
    pub cursor_key: [u8; 32],
}

/// Builds all owner contributions that have completed the two-owner proof.
///
/// No module identifiers or route coordinates are repeated here. Owner
/// packages remain the only source of those definitions, and final application
/// assembly retains duplicate, owner and route-kind validation.
pub fn build_all(
    dependencies: FirstPartyProductionDependencies,
) -> Result<ModuleContributionSet, SdkError> {
    let FirstPartyProductionDependencies {
        store,
        parties,
        activation,
        visibility_authorizer,
        cursor_key,
    } = dependencies;
    let mut contributions = ModuleContributionSet::new();

    contributions.merge(build_customer_accounts_contribution(
        CustomerAccountsProductionDependencies {
            store: store.clone(),
            parties,
            activation: activation.clone(),
            visibility_authorizer: visibility_authorizer.clone(),
            cursor_key,
        },
    )?);
    contributions.merge(build_consents_contribution(
        ConsentsProductionDependencies {
            store,
            activation,
            visibility_authorizer,
            cursor_key,
        },
    )?);

    Ok(contributions)
}

pub const CRATE_NAME: &str = "crm-first-party-modules";
""",
    encoding="utf-8",
)

runtime_cargo = ROOT / "crates/crm-application-runtime/Cargo.toml"
replace_exact(
    runtime_cargo,
    'crm-application-composition = { path = "../crm-application-composition" }\n',
    'crm-application-composition = { path = "../crm-application-composition" }\n'
    'crm-first-party-modules = { path = "../crm-first-party-modules" }\n',
)
replace_exact(
    runtime_cargo,
    'crm-consents-capability-composition = { path = "../crm-consents-capability-composition" }\n',
    "",
)
replace_exact(
    runtime_cargo,
    'crm-customer-accounts-capability-composition = { path = "../crm-customer-accounts-capability-composition" }\n',
    "",
)

native = ROOT / "crates/crm-application-runtime/src/native_composition.rs"
replace_exact(
    native,
    "use crm_consents_capability_composition::{\n"
    "    ConsentsProductionDependencies, build_contribution as build_consents_contribution,\n"
    "};\n",
    "",
)
replace_exact(
    native,
    "use crm_customer_accounts_capability_composition::{\n"
    "    CustomerAccountsProductionDependencies,\n"
    "    build_contribution as build_customer_accounts_contribution,\n"
    "};\n",
    "use crm_first_party_modules::{\n"
    "    FirstPartyProductionDependencies, build_all as build_first_party_modules,\n"
    "};\n",
)
replace_exact(
    native,
    "    contributions.merge(build_customer_accounts_contribution(\n"
    "        CustomerAccountsProductionDependencies {\n"
    "            store: store.clone(),\n"
    "            parties: parties.clone(),\n"
    "            activation: activation.clone(),\n"
    "            visibility_authorizer: visibility_authorizer.clone(),\n"
    "            cursor_key,\n"
    "        },\n"
    "    )?);\n",
    "    contributions.merge(build_first_party_modules(\n"
    "        FirstPartyProductionDependencies {\n"
    "            store: store.clone(),\n"
    "            parties: parties.clone(),\n"
    "            activation: activation.clone(),\n"
    "            visibility_authorizer: visibility_authorizer.clone(),\n"
    "            cursor_key,\n"
    "        },\n"
    "    )?);\n",
)
replace_exact(
    native,
    "    contributions.merge(build_consents_contribution(\n"
    "        ConsentsProductionDependencies {\n"
    "            store: store.clone(),\n"
    "            activation: activation.clone(),\n"
    "            visibility_authorizer: visibility_authorizer.clone(),\n"
    "            cursor_key,\n"
    "        },\n"
    "    )?);\n\n",
    "",
)

guard = ROOT / "scripts/check_native_module_composition.py"
replace_exact(
    guard,
    "    LegacyMarker(\n"
    "        \"crates/crm-application-runtime/src/native_composition.rs\",\n"
    "        \"let consent_queries = Arc::new(ConsentQueryAdapter::new(\",\n"
    "        \"Consents route registration returned to the generic process host\",\n"
    "    ),\n"
    ")\n",
    "    LegacyMarker(\n"
    "        \"crates/crm-application-runtime/src/native_composition.rs\",\n"
    "        \"let consent_queries = Arc::new(ConsentQueryAdapter::new(\",\n"
    "        \"Consents route registration returned to the generic process host\",\n"
    "    ),\n"
    "    LegacyMarker(\n"
    "        \"crates/crm-application-runtime/src/native_composition.rs\",\n"
    "        \"CustomerAccountsProductionDependencies\",\n"
    "        \"Customer Accounts owner contribution bypassed the first-party aggregate\",\n"
    "    ),\n"
    "    LegacyMarker(\n"
    "        \"crates/crm-application-runtime/src/native_composition.rs\",\n"
    "        \"build_customer_accounts_contribution\",\n"
    "        \"Customer Accounts owner builder bypassed the first-party aggregate\",\n"
    "    ),\n"
    "    LegacyMarker(\n"
    "        \"crates/crm-application-runtime/src/native_composition.rs\",\n"
    "        \"ConsentsProductionDependencies\",\n"
    "        \"Consents owner contribution bypassed the first-party aggregate\",\n"
    "    ),\n"
    "    LegacyMarker(\n"
    "        \"crates/crm-application-runtime/src/native_composition.rs\",\n"
    "        \"build_consents_contribution\",\n"
    "        \"Consents owner builder bypassed the first-party aggregate\",\n"
    "    ),\n"
    ")\n",
)

governance = ROOT / ".github/workflows/governance.yml"
replace_exact(
    governance,
    '      - "crates/crm-application-composition/**"\n'
    '      - "crates/crm-application-runtime/**"\n',
    '      - "crates/crm-application-composition/**"\n'
    '      - "crates/crm-first-party-modules/**"\n'
    '      - "crates/crm-application-runtime/**"\n',
    count=2,
)

application_ci = ROOT / ".github/workflows/application-runtime.yml"
replace_exact(
    application_ci,
    '      - "crates/crm-application-runtime/**"\n'
    '      - "crates/crm-capability-adapters/**"\n',
    '      - "crates/crm-application-runtime/**"\n'
    '      - "crates/crm-first-party-modules/**"\n'
    '      - "crates/crm-capability-adapters/**"\n',
    count=2,
)

doc = ROOT / "docs/FIRST_PARTY_MODULE_AGGREGATION.md"
doc.write_text(
    """# First-Party Module Contribution Aggregation

Status: bounded Phase C/D aggregation candidate

Customer Accounts and Consents have completed two contrasting module-owned production-contribution proofs. This packet introduces the narrow aggregate required to keep the generic process host independent from a growing list of owner packages.

## New crate justification

- **Protected boundary:** the generic application runtime depends on one first-party aggregate rather than concrete owner composition packages.
- **Isolated dependencies:** owner production-package dependencies remain behind `crm-first-party-modules`.
- **Expected consumers:** `crm-application-runtime` is the first consumer; process-test composition, packaging and future extraction tooling may consume the same stable boundary.
- **Why an internal module is insufficient:** an internal runtime module would not mechanically prevent the process host crate from importing each owner directly.
- **Lifecycle/extraction seam:** the architecture complexity plan explicitly requires a first-party module bundle before affected-scope CI and broader owner migration.
- **Build/test fan-out:** the runtime replaces two direct owner composition dependencies with one aggregate dependency; future migrated owners should change the aggregate rather than generic runtime imports.

## Source-of-truth rule

The aggregate stores no module identifier list and no capability coordinates. It calls owner-provided `build_contribution` functions and merges their `ModuleContributionSet` values. Owner definitions remain authoritative, and final `ApplicationComposition` assembly still rejects duplicates, owner mismatch and route-kind mismatch.

## Initial proven owners

- Customer Accounts — simple Party-reference validation plus mutation/query routes.
- Consents — multi-record Party/Contact Point validation, owner-specific executor wrapping and mutation/query routes.

Other owners remain in their current production wiring until migrated by separately proven packets. Adding a capability to either migrated owner must not require editing the generic application runtime.

## Acceptance boundary

This packet changes composition only. It introduces no public route, worker, contract, migration or business behavior. Application Runtime process acceptance and full unchanged exact-head acceptance remain mandatory.
""",
    encoding="utf-8",
)

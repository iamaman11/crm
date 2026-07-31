#!/usr/bin/env python3
"""Fail closed while production still depends on legacy central module wiring."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]


@dataclass(frozen=True)
class LegacyMarker:
    path: str
    needle: str
    reason: str


LEGACY_MARKERS: tuple[LegacyMarker, ...] = (
    LegacyMarker(
        "crates/crm-application-runtime/src/governed_metadata.rs",
        "pub struct ApplicationAggregatePlannerRouter",
        "mutation planners are still selected by a central capability switch",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/governed_metadata.rs",
        "pub struct ApplicationCapabilityExecutorRouter",
        "mutation executors are still selected by a central capability switch",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/governed_metadata.rs",
        "pub struct ApplicationQueryRouter",
        "query validators and executors are still selected by a central capability switch",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/platform.rs",
        "pub struct ContractBoundMutationSemanticValidator",
        "production mutation semantic validation is still a process-wide no-op boundary",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/runtime.rs",
        "ModuleRegistry::new(Version::new(",
        "the runtime constructs an empty in-memory registry instead of an active-route authority",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/runtime.rs",
        "match definition.owner_module_id.as_str()",
        "bootstrap visibility still branches centrally by module owner",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/runtime.rs",
        "pub link_processor:",
        "background work is still represented by fixed process fields",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/runtime.rs",
        "pub projection_worker:",
        "background work is still represented by fixed process fields",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/runtime.rs",
        "pub customer_360_worker:",
        "background work is still represented by fixed process fields",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/runtime.rs",
        "pub search_worker:",
        "background work is still represented by fixed process fields",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/runtime.rs",
        "pub import_execution_worker:",
        "background work is still represented by fixed process fields",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/runtime.rs",
        "pub export_selection_worker:",
        "background work is still represented by fixed process fields",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "CustomerAccountCapabilityPlanner",
        "Customer Accounts mutation construction returned to the generic process host",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "AccountPartyReferenceSemanticValidator",
        "Customer Accounts semantic validation returned to the generic process host",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "AccountQueryAdapter::new",
        "Customer Accounts query construction returned to the generic process host",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "ConsentCapabilityPlanner",
        "Consents mutation construction returned to the generic process host",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "ConsentCapabilitySemanticValidator",
        "Consents semantic validation returned to the generic process host",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "PostgresConsentReferenceReader",
        "Consents reference-reader construction returned to the generic process host",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "let consent_queries = Arc::new(ConsentQueryAdapter::new(",
        "Consents route registration returned to the generic process host",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "CustomerAccountsProductionDependencies",
        "Customer Accounts owner contribution bypassed the first-party aggregate",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "build_customer_accounts_contribution",
        "Customer Accounts owner builder bypassed the first-party aggregate",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "ConsentsProductionDependencies",
        "Consents owner contribution bypassed the first-party aggregate",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "build_consents_contribution",
        "Consents owner builder bypassed the first-party aggregate",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "use crm_customer_accounts_capability_adapter",
        "Customer Accounts mutation inventory bypassed the first-party aggregate",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "use crm_customer_accounts_query_adapter",
        "Customer Accounts query inventory bypassed the first-party aggregate",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "use crm_consents_capability_adapter",
        "Consents inventory bypassed the first-party aggregate",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "ContactPointCapabilityPlanner",
        "Contact Points mutation construction returned to the generic process host",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "ContactPointPartyReferenceSemanticValidator",
        "Contact Points semantic validation returned to the generic process host",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "ContactPointQueryAdapter::new",
        "Contact Points query construction returned to the generic process host",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "PartyRelationshipCapabilityPlanner",
        "Party Relationships mutation construction returned to the generic process host",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "PartyRelationshipReferenceSemanticValidator",
        "Party Relationships semantic validation returned to the generic process host",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "PartyRelationshipQueryAdapter::new",
        "Party Relationships query construction returned to the generic process host",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "add_activated_mutations(\n"
        "        &mut contributions,\n"
        "        party_capability_definitions()?",
        "Parties mutation registration returned to the generic process host",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "add_activated_queries(\n"
        "        &mut contributions,\n"
        "        party_query_capability_definitions()?",
        "Parties query registration returned to the generic process host",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "IdentityResolutionCapabilityPlanner",
        "Identity Resolution mutation construction returned to the generic process host",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "IdentityResolutionQueryAdapter::new",
        "Identity Resolution query construction returned to the generic process host",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "CustomerDataOperationsCapabilityPlanner",
        "Customer Data Operations mutation construction returned to the generic process host",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "CustomerDataOperationsQueryAdapter::new",
        "Customer Data Operations query construction returned to the generic process host",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "DataQualityCapabilityExecutor::new",
        "Data Quality mutation construction returned to the generic process host",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "DataQualityQueryAdapter::new",
        "Data Quality query construction returned to the generic process host",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "IdentityResolutionProductionDependencies",
        "Identity Resolution owner contribution bypassed the first-party aggregate",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "CustomerDataOperationsProductionDependencies",
        "Customer Data Operations owner contribution bypassed the first-party aggregate",
    ),
    LegacyMarker(
        "crates/crm-application-runtime/src/native_composition.rs",
        "DataQualityProductionDependencies",
        "Data Quality owner contribution bypassed the first-party aggregate",
    ),
)


def find_legacy_composition_violations(root: Path = ROOT) -> list[str]:
    violations: list[str] = []
    for marker in LEGACY_MARKERS:
        path = root / marker.path
        if not path.exists():
            continue
        try:
            content = path.read_text(encoding="utf-8")
        except OSError as exc:
            violations.append(f"{marker.path}: could not inspect file: {exc}")
            continue
        if marker.needle in content:
            violations.append(
                f"{marker.path}: {marker.reason} (legacy marker: {marker.needle})"
            )
    return violations


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=ROOT)
    args = parser.parse_args(argv)
    violations = find_legacy_composition_violations(args.root.resolve())
    if violations:
        print("Native module composition readiness FAILED:")
        for violation in violations:
            print(f"- {violation}")
        print(
            "Production feature expansion remains blocked until every legacy marker is removed "
            "through module-owned contributions and deterministic generic registries."
        )
        return 1
    print("Native module composition readiness passed: no legacy central wiring remains")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

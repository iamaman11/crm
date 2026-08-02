#!/usr/bin/env python3
"""One-shot deterministic source patch for the Step 17 telemetry packet."""

from __future__ import annotations

import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
BASELINE = "a9d6a3c58dc0418343a8919ae731aa5c8b3f92e8"
PACKET_ID = "repository-step-17-contract-usage-telemetry"


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, content: str) -> None:
    target = ROOT / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(content, encoding="utf-8")


def replace_once(path: str, old: str, new: str) -> None:
    content = read(path)
    count = content.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: expected one replacement target, found {count}: {old[:80]!r}")
    write(path, content.replace(old, new, 1))


def replace_all_exact(path: str, old: str, new: str, expected: int) -> None:
    content = read(path)
    count = content.count(old)
    if count != expected:
        raise RuntimeError(f"{path}: expected {expected} targets, found {count}: {old[:80]!r}")
    write(path, content.replace(old, new))


def replace_allowed_set(path: str, items: list[str]) -> None:
    content = read(path)
    pattern = re.compile(
        r'(set\(packet\["allowed_paths"\]\),\n\s*)\{.*?\}(,?\n\s*\),)',
        re.DOTALL,
    )
    body = "{\n" + "".join(f'                "{item}",\n' for item in items) + "            }"
    replaced, count = pattern.subn(lambda match: match.group(1) + body + match.group(2), content, count=1)
    if count != 1:
        raise RuntimeError(f"{path}: could not replace allowed path assertion")
    write(path, replaced)


def replace_required_checks(path: str, checks: list[str]) -> None:
    content = read(path)
    pattern = re.compile(
        r'(self\.assertEqual\(\n\s*(?:self\.)?packet\["required_checks"\],\n\s*)\[.*?\](,\n\s*\))',
        re.DOTALL,
    )
    body = "[\n" + "".join(f'                "{check}",\n' for check in checks) + "            ]"
    replaced, count = pattern.subn(lambda match: match.group(1) + body + match.group(2), content, count=1)
    if count != 1:
        raise RuntimeError(f"{path}: could not replace required check assertion")
    write(path, replaced)


TELEMETRY_MODULE = r'''use crate::generated_contract_telemetry::DEPRECATED_CONTRACTS;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRegistryPort};
use crm_module_sdk::{CapabilityId, CapabilityVersion, PortFuture, SdkError};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ContractUsageSurface {
    Mutation,
    Query,
}

impl ContractUsageSurface {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Mutation => "mutation",
            Self::Query => "query",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DeprecatedContractTelemetry {
    pub capability_id: &'static str,
    pub capability_version: &'static str,
    pub owner_module_id: &'static str,
    pub metric: &'static str,
    pub lookback_days: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Coordinate {
    capability_id: String,
    capability_version: String,
}

impl Coordinate {
    fn from_definition(definition: &CapabilityDefinition) -> Self {
        Self {
            capability_id: definition.capability_id.as_str().to_owned(),
            capability_version: definition.capability_version.as_str().to_owned(),
        }
    }

    fn from_catalog(entry: &DeprecatedContractTelemetry) -> Self {
        Self {
            capability_id: entry.capability_id.to_owned(),
            capability_version: entry.capability_version.to_owned(),
        }
    }

    fn display(&self) -> String {
        format!("{}@{}", self.capability_id, self.capability_version)
    }
}

#[derive(Debug, Clone)]
struct PublishedDefinition {
    owner_module_id: String,
    surface: ContractUsageSurface,
}

#[derive(Debug)]
struct MetricSeries {
    owner_module_id: &'static str,
    metric: &'static str,
    lookback_days: u32,
    surface: ContractUsageSurface,
    count: AtomicU64,
}

#[derive(Debug, Default)]
pub struct ContractUsageMetrics {
    series: BTreeMap<Coordinate, MetricSeries>,
}

impl ContractUsageMetrics {
    pub fn production(
        mutation_definitions: &[CapabilityDefinition],
        query_definitions: &[CapabilityDefinition],
    ) -> Result<Self, ContractUsageTelemetryError> {
        Self::from_catalog(
            DEPRECATED_CONTRACTS,
            mutation_definitions,
            query_definitions,
        )
    }

    fn from_catalog(
        catalog: &'static [DeprecatedContractTelemetry],
        mutation_definitions: &[CapabilityDefinition],
        query_definitions: &[CapabilityDefinition],
    ) -> Result<Self, ContractUsageTelemetryError> {
        let mut published = BTreeMap::new();
        add_published_definitions(
            &mut published,
            mutation_definitions,
            ContractUsageSurface::Mutation,
        )?;
        add_published_definitions(
            &mut published,
            query_definitions,
            ContractUsageSurface::Query,
        )?;

        let mut series = BTreeMap::new();
        for entry in catalog {
            let coordinate = Coordinate::from_catalog(entry);
            let definition = published.get(&coordinate).ok_or_else(|| {
                ContractUsageTelemetryError::UnpublishedContract(coordinate.display())
            })?;
            if definition.owner_module_id != entry.owner_module_id {
                return Err(ContractUsageTelemetryError::OwnerMismatch {
                    coordinate: coordinate.display(),
                    expected: entry.owner_module_id.to_owned(),
                    actual: definition.owner_module_id.clone(),
                });
            }
            let metric_series = MetricSeries {
                owner_module_id: entry.owner_module_id,
                metric: entry.metric,
                lookback_days: entry.lookback_days,
                surface: definition.surface,
                count: AtomicU64::new(0),
            };
            if series.insert(coordinate.clone(), metric_series).is_some() {
                return Err(ContractUsageTelemetryError::DuplicateContract(
                    coordinate.display(),
                ));
            }
        }
        Ok(Self { series })
    }

    fn record_resolved(
        &self,
        definition: &CapabilityDefinition,
        surface: ContractUsageSurface,
    ) {
        let coordinate = Coordinate::from_definition(definition);
        let Some(series) = self.series.get(&coordinate) else {
            return;
        };
        if series.surface != surface
            || series.owner_module_id != definition.owner_module_id.as_str()
        {
            return;
        }
        let _ = series.count.fetch_update(
            Ordering::Relaxed,
            Ordering::Relaxed,
            |current| current.checked_add(1),
        );
    }

    pub fn render_prometheus(&self) -> String {
        let metrics: BTreeSet<_> = self.series.values().map(|series| series.metric).collect();
        let mut output = String::new();
        for metric in metrics {
            output.push_str("# HELP ");
            output.push_str(metric);
            output.push_str(
                " Resolved requests for deprecated exact-version capability contracts.\n",
            );
            output.push_str("# TYPE ");
            output.push_str(metric);
            output.push_str(" counter\n");
            for (coordinate, series) in &self.series {
                if series.metric != metric {
                    continue;
                }
                output.push_str(metric);
                output.push_str("{capability_id=\"");
                output.push_str(&coordinate.capability_id);
                output.push_str("\",capability_version=\"");
                output.push_str(&coordinate.capability_version);
                output.push_str("\",owner_module_id=\"");
                output.push_str(series.owner_module_id);
                output.push_str("\",surface=\"");
                output.push_str(series.surface.as_str());
                output.push_str("\",lookback_days=\"");
                output.push_str(&series.lookback_days.to_string());
                output.push_str("\"} ");
                output.push_str(&series.count.load(Ordering::Relaxed).to_string());
                output.push('\n');
            }
        }
        output
    }

    #[cfg(test)]
    fn count(&self, capability_id: &str, capability_version: &str) -> Option<u64> {
        self.series
            .get(&Coordinate {
                capability_id: capability_id.to_owned(),
                capability_version: capability_version.to_owned(),
            })
            .map(|series| series.count.load(Ordering::Relaxed))
    }
}

fn add_published_definitions(
    published: &mut BTreeMap<Coordinate, PublishedDefinition>,
    definitions: &[CapabilityDefinition],
    surface: ContractUsageSurface,
) -> Result<(), ContractUsageTelemetryError> {
    for definition in definitions {
        let coordinate = Coordinate::from_definition(definition);
        let entry = PublishedDefinition {
            owner_module_id: definition.owner_module_id.as_str().to_owned(),
            surface,
        };
        if published.insert(coordinate.clone(), entry).is_some() {
            return Err(ContractUsageTelemetryError::DuplicatePublishedContract(
                coordinate.display(),
            ));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractUsageTelemetryError {
    DuplicatePublishedContract(String),
    DuplicateContract(String),
    UnpublishedContract(String),
    OwnerMismatch {
        coordinate: String,
        expected: String,
        actual: String,
    },
}

impl fmt::Display for ContractUsageTelemetryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicatePublishedContract(coordinate) => {
                write!(formatter, "published contract is duplicated: {coordinate}")
            }
            Self::DuplicateContract(coordinate) => {
                write!(formatter, "deprecated telemetry contract is duplicated: {coordinate}")
            }
            Self::UnpublishedContract(coordinate) => {
                write!(formatter, "deprecated telemetry contract is not published: {coordinate}")
            }
            Self::OwnerMismatch {
                coordinate,
                expected,
                actual,
            } => write!(
                formatter,
                "deprecated telemetry owner mismatch for {coordinate}: expected {expected}, got {actual}"
            ),
        }
    }
}

impl std::error::Error for ContractUsageTelemetryError {}

#[derive(Clone)]
pub struct MeteredCapabilityRegistry {
    inner: Arc<dyn CapabilityRegistryPort>,
    metrics: Arc<ContractUsageMetrics>,
    surface: ContractUsageSurface,
}

impl fmt::Debug for MeteredCapabilityRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MeteredCapabilityRegistry")
            .field("inner", &"dyn CapabilityRegistryPort")
            .field("surface", &self.surface)
            .finish()
    }
}

impl MeteredCapabilityRegistry {
    pub fn new(
        inner: Arc<dyn CapabilityRegistryPort>,
        metrics: Arc<ContractUsageMetrics>,
        surface: ContractUsageSurface,
    ) -> Self {
        Self {
            inner,
            metrics,
            surface,
        }
    }
}

impl CapabilityRegistryPort for MeteredCapabilityRegistry {
    fn resolve<'a>(
        &'a self,
        capability_id: &'a CapabilityId,
        capability_version: &'a CapabilityVersion,
    ) -> PortFuture<'a, Result<Option<CapabilityDefinition>, SdkError>> {
        Box::pin(async move {
            let definition = self
                .inner
                .resolve(capability_id, capability_version)
                .await?;
            if let Some(definition) = definition.as_ref() {
                self.metrics.record_resolved(definition, self.surface);
            }
            Ok(definition)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_capability_runtime::{CapabilityRisk, PayloadContract};
    use crm_module_sdk::{
        DataClass, ModuleId, PayloadEncoding, SchemaId, SchemaVersion,
    };

    static CATALOG: &[DeprecatedContractTelemetry] = &[DeprecatedContractTelemetry {
        capability_id: "test.record.create",
        capability_version: "1.0.0",
        owner_module_id: "crm.test",
        metric: "crm_contract_invocations_total",
        lookback_days: 30,
    }];

    #[derive(Debug)]
    struct Registry {
        definition: CapabilityDefinition,
    }

    impl CapabilityRegistryPort for Registry {
        fn resolve<'a>(
            &'a self,
            capability_id: &'a CapabilityId,
            capability_version: &'a CapabilityVersion,
        ) -> PortFuture<'a, Result<Option<CapabilityDefinition>, SdkError>> {
            Box::pin(async move {
                if &self.definition.capability_id == capability_id
                    && &self.definition.capability_version == capability_version
                {
                    Ok(Some(self.definition.clone()))
                } else {
                    Ok(None)
                }
            })
        }
    }

    #[tokio::test]
    async fn exact_deprecated_resolution_increments_before_later_gateway_checks() {
        let definition = definition(true, "crm.test");
        let metrics = Arc::new(
            ContractUsageMetrics::from_catalog(CATALOG, &[definition.clone()], &[]).unwrap(),
        );
        let registry = MeteredCapabilityRegistry::new(
            Arc::new(Registry {
                definition: definition.clone(),
            }),
            Arc::clone(&metrics),
            ContractUsageSurface::Mutation,
        );

        assert_eq!(metrics.count("test.record.create", "1.0.0"), Some(0));
        assert!(
            registry
                .resolve(&definition.capability_id, &definition.capability_version)
                .await
                .unwrap()
                .is_some()
        );
        assert_eq!(metrics.count("test.record.create", "1.0.0"), Some(1));
    }

    #[tokio::test]
    async fn unknown_or_wrong_surface_resolution_does_not_increment() {
        let definition = definition(true, "crm.test");
        let metrics = Arc::new(
            ContractUsageMetrics::from_catalog(CATALOG, &[definition.clone()], &[]).unwrap(),
        );
        let registry = MeteredCapabilityRegistry::new(
            Arc::new(Registry {
                definition: definition.clone(),
            }),
            Arc::clone(&metrics),
            ContractUsageSurface::Query,
        );
        let missing = CapabilityId::try_new("test.record.missing").unwrap();

        assert!(
            registry
                .resolve(&missing, &definition.capability_version)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            registry
                .resolve(&definition.capability_id, &definition.capability_version)
                .await
                .unwrap()
                .is_some()
        );
        assert_eq!(metrics.count("test.record.create", "1.0.0"), Some(0));
    }

    #[test]
    fn catalog_is_fail_closed_against_live_owner_and_coordinate() {
        let wrong_owner = definition(true, "crm.other");
        let owner_error =
            ContractUsageMetrics::from_catalog(CATALOG, &[wrong_owner], &[]).unwrap_err();
        assert!(matches!(
            owner_error,
            ContractUsageTelemetryError::OwnerMismatch { .. }
        ));

        let missing_error = ContractUsageMetrics::from_catalog(CATALOG, &[], &[]).unwrap_err();
        assert!(matches!(
            missing_error,
            ContractUsageTelemetryError::UnpublishedContract(_)
        ));
    }

    #[test]
    fn prometheus_output_is_deterministic_zero_seeded_and_pii_free() {
        let definition = definition(true, "crm.test");
        let metrics =
            ContractUsageMetrics::from_catalog(CATALOG, &[definition], &[]).unwrap();
        let first = metrics.render_prometheus();
        let second = metrics.render_prometheus();

        assert_eq!(first, second);
        assert!(first.contains("crm_contract_invocations_total"));
        assert!(first.contains("capability_id=\"test.record.create\""));
        assert!(first.contains("surface=\"mutation\""));
        assert!(first.ends_with(" 0\n"));
        assert!(!first.contains("tenant"));
        assert!(!first.contains("actor"));
        assert!(!first.contains("request_id"));
    }

    fn definition(mutation: bool, owner: &str) -> CapabilityDefinition {
        let contract = PayloadContract {
            owner: ModuleId::try_new(owner).unwrap(),
            schema_id: SchemaId::try_new("crm.test.v1.RecordRequest").unwrap(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
            descriptor_hash: [0x51; 32],
            allowed_data_classes: vec![DataClass::Internal],
            allowed_encodings: vec![PayloadEncoding::Protobuf],
            maximum_size_bytes: 1024,
        };
        CapabilityDefinition {
            capability_id: CapabilityId::try_new("test.record.create").unwrap(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            owner_module_id: ModuleId::try_new(owner).unwrap(),
            input_contract: contract.clone(),
            output_contract: Some(contract),
            risk: CapabilityRisk::Low,
            mutation,
            requires_idempotency: mutation,
            requires_approval: false,
            authorization_policy_id: "test.record.create".to_owned(),
            rate_limit_policy_id: None,
        }
    }
}
'''

GENERATOR = r'''#!/usr/bin/env python3
"""Generate the Rust catalog consumed by production deprecated-contract telemetry."""

from __future__ import annotations

import argparse
import difflib
import json
from pathlib import Path
import re
import sys
from typing import Any

POLICY_SCHEMA_VERSION = "crm.contract-lifecycle-policy/v1"
SAFE_TEXT = re.compile(r"^[A-Za-z0-9_.:+-]+$")


def load_policy(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read lifecycle policy {path}: {error}") from error
    if not isinstance(value, dict):
        raise ValueError("lifecycle policy must contain an object")
    return value


def required_text(value: dict[str, Any], key: str, location: str) -> str:
    candidate = value.get(key)
    if not isinstance(candidate, str) or not candidate or not SAFE_TEXT.fullmatch(candidate):
        raise ValueError(f"{location}.{key} must be safe non-empty ASCII text")
    return candidate


def deprecated_capabilities(policy: dict[str, Any]) -> list[dict[str, Any]]:
    if policy.get("schema_version") != POLICY_SCHEMA_VERSION:
        raise ValueError(f"lifecycle policy must use {POLICY_SCHEMA_VERSION}")
    contracts = policy.get("contracts")
    if not isinstance(contracts, list):
        raise ValueError("lifecycle policy contracts must be a list")
    entries: list[dict[str, Any]] = []
    seen: set[tuple[str, str]] = set()
    for index, item in enumerate(contracts):
        location = f"contracts[{index}]"
        if not isinstance(item, dict):
            raise ValueError(f"{location} must be an object")
        if item.get("kind") != "capability" or item.get("state") != "deprecated":
            continue
        capability_id = required_text(item, "id", location)
        version = required_text(item, "version", location)
        owner = required_text(item, "owner", location)
        telemetry = item.get("telemetry")
        if not isinstance(telemetry, dict):
            raise ValueError(f"{location}.telemetry must be an object")
        metric = required_text(telemetry, "metric", f"{location}.telemetry")
        lookback_days = telemetry.get("lookback_days")
        if isinstance(lookback_days, bool) or not isinstance(lookback_days, int) or lookback_days <= 0:
            raise ValueError(f"{location}.telemetry.lookback_days must be positive")
        coordinate = (capability_id, version)
        if coordinate in seen:
            raise ValueError(f"duplicate deprecated capability {capability_id}@{version}")
        seen.add(coordinate)
        entries.append(
            {
                "capability_id": capability_id,
                "capability_version": version,
                "owner_module_id": owner,
                "metric": metric,
                "lookback_days": lookback_days,
            }
        )
    entries.sort(key=lambda entry: (entry["capability_id"], entry["capability_version"]))
    return entries


def render(entries: list[dict[str, Any]]) -> bytes:
    lines = [
        "// @generated by scripts/generate_contract_telemetry_catalog.py; do not edit.\n",
        "use crate::contract_usage_telemetry::DeprecatedContractTelemetry;\n\n",
        "pub(crate) const DEPRECATED_CONTRACTS: &[DeprecatedContractTelemetry] = &[\n",
    ]
    for entry in entries:
        lines.extend(
            [
                "    DeprecatedContractTelemetry {\n",
                f'        capability_id: "{entry["capability_id"]}",\n',
                f'        capability_version: "{entry["capability_version"]}",\n',
                f'        owner_module_id: "{entry["owner_module_id"]}",\n',
                f'        metric: "{entry["metric"]}",\n',
                f'        lookback_days: {entry["lookback_days"]},\n',
                "    },\n",
            ]
        )
    lines.append("];\n")
    return "".join(lines).encode("utf-8")


def write_atomic(path: Path, content: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(path.name + ".tmp")
    temporary.write_bytes(content)
    temporary.replace(path)


def check_exact(path: Path, expected: bytes) -> list[str]:
    try:
        actual = path.read_bytes()
    except OSError as error:
        return [f"cannot read generated telemetry catalog {path}: {error}"]
    if actual == expected:
        return []
    diff = "".join(
        difflib.unified_diff(
            actual.decode("utf-8", errors="replace").splitlines(keepends=True),
            expected.decode("utf-8").splitlines(keepends=True),
            fromfile=str(path),
            tofile=f"{path} (generated)",
        )
    )
    return [
        f"{path} is stale; run python scripts/generate_contract_telemetry_catalog.py --write",
        diff.rstrip(),
    ]


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true")
    mode.add_argument("--write", action="store_true")
    parser.add_argument(
        "--policy",
        type=Path,
        default=Path("contracts/contract-lifecycle-policy.json"),
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=Path("crates/crm-application-runtime/src/generated_contract_telemetry.rs"),
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        expected = render(deprecated_capabilities(load_policy(args.policy)))
    except ValueError as error:
        print(f"contract telemetry catalog generation failed: {error}", file=sys.stderr)
        return 1
    if args.write:
        write_atomic(args.output, expected)
        print(f"wrote contract telemetry catalog: {args.output}")
        return 0
    errors = check_exact(args.output, expected)
    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1
    print("contract telemetry catalog is synchronized")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
'''

GENERATOR_TEST = r'''from __future__ import annotations

from pathlib import Path
import tempfile
import unittest

from scripts.generate_contract_telemetry_catalog import deprecated_capabilities, load_policy, render


ROOT = Path(__file__).resolve().parents[1]


class ContractTelemetryCatalogTests(unittest.TestCase):
    def test_committed_catalog_is_current(self) -> None:
        policy = load_policy(ROOT / "contracts/contract-lifecycle-policy.json")
        expected = render(deprecated_capabilities(policy))
        actual = (
            ROOT
            / "crates/crm-application-runtime/src/generated_contract_telemetry.rs"
        ).read_bytes()
        self.assertEqual(actual, expected)

    def test_only_deprecated_capabilities_are_sorted_and_rendered(self) -> None:
        policy = {
            "schema_version": "crm.contract-lifecycle-policy/v1",
            "contracts": [
                {
                    "kind": "event",
                    "id": "test.created",
                    "version": "1.0.0",
                    "state": "deprecated",
                    "owner": "crm.test",
                    "telemetry": {
                        "metric": "crm_contract_invocations_total",
                        "lookback_days": 30,
                    },
                },
                {
                    "kind": "capability",
                    "id": "zeta.create",
                    "version": "1.0.0",
                    "state": "active",
                    "owner": "crm.zeta",
                    "telemetry": {
                        "metric": "crm_contract_invocations_total",
                        "lookback_days": 30,
                    },
                },
                {
                    "kind": "capability",
                    "id": "alpha.create",
                    "version": "1.0.0",
                    "state": "deprecated",
                    "owner": "crm.alpha",
                    "telemetry": {
                        "metric": "crm_contract_invocations_total",
                        "lookback_days": 45,
                    },
                },
            ],
        }
        entries = deprecated_capabilities(policy)
        self.assertEqual([entry["capability_id"] for entry in entries], ["alpha.create"])
        generated = render(entries).decode("utf-8")
        self.assertIn('capability_id: "alpha.create"', generated)
        self.assertIn("lookback_days: 45", generated)
        self.assertNotIn("test.created", generated)
        self.assertNotIn("zeta.create", generated)

    def test_duplicate_and_invalid_telemetry_fail_closed(self) -> None:
        entry = {
            "kind": "capability",
            "id": "test.create",
            "version": "1.0.0",
            "state": "deprecated",
            "owner": "crm.test",
            "telemetry": {
                "metric": "crm_contract_invocations_total",
                "lookback_days": 30,
            },
        }
        policy = {
            "schema_version": "crm.contract-lifecycle-policy/v1",
            "contracts": [entry, dict(entry)],
        }
        with self.assertRaisesRegex(ValueError, "duplicate deprecated capability"):
            deprecated_capabilities(policy)
        policy["contracts"] = [dict(entry, telemetry={"metric": "bad metric", "lookback_days": 0})]
        with self.assertRaises(ValueError):
            deprecated_capabilities(policy)

    def test_policy_loader_rejects_non_object(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "policy.json"
            path.write_text("[]", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "must contain an object"):
                load_policy(path)


if __name__ == "__main__":
    unittest.main()
'''

GENERATED_EMPTY = '''// @generated by scripts/generate_contract_telemetry_catalog.py; do not edit.\nuse crate::contract_usage_telemetry::DeprecatedContractTelemetry;\n\npub(crate) const DEPRECATED_CONTRACTS: &[DeprecatedContractTelemetry] = &[\n];\n'''


# New production and generation files.
write("crates/crm-application-runtime/src/contract_usage_telemetry.rs", TELEMETRY_MODULE)
write("crates/crm-application-runtime/src/generated_contract_telemetry.rs", GENERATED_EMPTY)
write("scripts/generate_contract_telemetry_catalog.py", GENERATOR)
write("tests/test_contract_telemetry_catalog.py", GENERATOR_TEST)

# Wire the application-runtime module without changing generic runtime APIs.
replace_once(
    "crates/crm-application-runtime/src/lib.rs",
    "mod config;\n",
    "mod config;\nmod contract_usage_telemetry;\nmod generated_contract_telemetry;\n",
)
replace_once(
    "crates/crm-application-runtime/src/lib.rs",
    "pub use config::*;\n",
    "pub use config::*;\npub use contract_usage_telemetry::*;\n",
)

runtime = "crates/crm-application-runtime/src/runtime.rs"
replace_once(
    runtime,
    "    ProductionBackgroundWorkerDependencies, ProductionCompositionDependencies, SystemClock,\n",
    "    ContractUsageMetrics, ContractUsageSurface, MeteredCapabilityRegistry,\n"
    "    ProductionBackgroundWorkerDependencies, ProductionCompositionDependencies, SystemClock,\n",
)
replace_once(
    runtime,
    "    pub export_artifact_download: Arc<PartyExportArtifactDownloadService>,\n",
    "    pub export_artifact_download: Arc<PartyExportArtifactDownloadService>,\n"
    "    pub contract_usage_metrics: Arc<ContractUsageMetrics>,\n",
)
replace_once(
    runtime,
    "        let query_definitions = composition.query_definitions().to_vec();\n",
    "        let query_definitions = composition.query_definitions().to_vec();\n"
    "        let contract_usage_metrics = Arc::new(\n"
    "            ContractUsageMetrics::production(&mutation_definitions, &query_definitions)\n"
    "                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,\n"
    "        );\n",
)
replace_once(
    runtime,
    "            composition.mutation_registry(),\n",
    "            Arc::new(MeteredCapabilityRegistry::new(\n"
    "                composition.mutation_registry(),\n"
    "                Arc::clone(&contract_usage_metrics),\n"
    "                ContractUsageSurface::Mutation,\n"
    "            )),\n",
)
replace_once(
    runtime,
    "            composition.query_registry(),\n",
    "            Arc::new(MeteredCapabilityRegistry::new(\n"
    "                composition.query_registry(),\n"
    "                Arc::clone(&contract_usage_metrics),\n"
    "                ContractUsageSurface::Query,\n"
    "            )),\n",
)
replace_once(
    runtime,
    "            export_artifact_download,\n            readiness: Arc::new(AtomicBool::new(false)),\n",
    "            export_artifact_download,\n"
    "            contract_usage_metrics,\n"
    "            readiness: Arc::new(AtomicBool::new(false)),\n",
)
replace_once(
    runtime,
    "        .route(\"/readyz\", get(ready))\n",
    "        .route(\"/readyz\", get(ready))\n        .route(\"/metrics\", get(metrics))\n",
)
replace_once(
    runtime,
    "async fn ready(State(state): State<HttpState>) -> impl IntoResponse {\n",
    "async fn metrics(State(state): State<HttpState>) -> impl IntoResponse {\n"
    "    (\n"
    "        StatusCode::OK,\n"
    "        state.components.contract_usage_metrics.render_prometheus(),\n"
    "    )\n"
    "}\n\n"
    "async fn ready(State(state): State<HttpState>) -> impl IntoResponse {\n",
)

# Contract CI owns deterministic catalog generation.
contracts_workflow = ".github/workflows/contracts.yml"
replace_all_exact(
    contracts_workflow,
    '      - "scripts/generate_contract_lifecycle.py"\n',
    '      - "scripts/generate_contract_lifecycle.py"\n'
    '      - "scripts/generate_contract_telemetry_catalog.py"\n',
    2,
)
replace_all_exact(
    contracts_workflow,
    '      - "tests/test_contract_lifecycle_transitions.py"\n',
    '      - "tests/test_contract_lifecycle_transitions.py"\n'
    '      - "tests/test_contract_telemetry_catalog.py"\n',
    2,
)
replace_once(
    contracts_workflow,
    "          python -m unittest tests/test_contract_bindings.py tests/test_contract_lifecycle.py tests/test_contract_lifecycle_transitions.py \\\n",
    "          python -m unittest tests/test_contract_bindings.py tests/test_contract_lifecycle.py tests/test_contract_lifecycle_transitions.py tests/test_contract_telemetry_catalog.py \\\n",
)

# Affected-scope ownership binds the generator and its runtime output to Contract CI.
policy_path = ROOT / "affected-scope-policy.json"
affected = json.loads(policy_path.read_text(encoding="utf-8"))
contracts_scope = next(scope for scope in affected["scopes"] if scope["id"] == "contracts")
for item in (
    "scripts/generate_contract_telemetry_catalog.py",
    "crates/crm-application-runtime/src/generated_contract_telemetry.rs",
):
    if item not in contracts_scope["path_patterns"]:
        contracts_scope["path_patterns"].append(item)
contracts_scope["path_patterns"].sort()
policy_path.write_text(json.dumps(affected, indent=2) + "\n", encoding="utf-8")

allowed_paths = [
    ".github/workflows/contracts.yml",
    "affected-scope-policy.json",
    "crates/crm-application-runtime/src/contract_usage_telemetry.rs",
    "crates/crm-application-runtime/src/generated_contract_telemetry.rs",
    "crates/crm-application-runtime/src/lib.rs",
    "crates/crm-application-runtime/src/runtime.rs",
    "docs/ACTIVE_PACKET.md",
    "repository-packet.json",
    "scripts/generate_contract_telemetry_catalog.py",
    "tests/test_architecture_documentation_consistency.py",
    "tests/test_contract_telemetry_catalog.py",
    "tests/test_repository_navigation.py",
]
required_checks = [
    "Affected Scope CI",
    "Application Runtime CI",
    "Contract CI",
    "Complexity Baseline CI",
    "Governance CI",
    "Rust Generated Sync",
    "Rust CI",
]
packet = {
    "schema_version": "crm.repository-packet/v1",
    "packet_id": PACKET_ID,
    "title": "Instrument deprecated capability usage in production",
    "status": "active",
    "baseline": {"ref": "main", "sha": BASELINE},
    "tracking_issues": [126, 194],
    "objective": (
        "Continue Repository Step 17 with deterministic, production-shaped usage telemetry for "
        "deprecated exact-version capability contracts. Generate a typed runtime catalog from the "
        "governed lifecycle policy, wrap the existing mutation and query registries without changing "
        "generic gateway APIs, count every successfully resolved deprecated coordinate before later "
        "semantic or authorization decisions, preseed zero series for retirement evidence and expose "
        "low-cardinality Prometheus text without tenant, actor, request or payload data."
    ),
    "allowed_paths": allowed_paths,
    "forbidden_paths": [
        "Cargo.lock",
        "Cargo.toml",
        "apps/**",
        "database/**",
        "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
        "docs/IMPLEMENTATION_ROADMAP.md",
        "docs/MODULE_CATALOG.md",
        "docs/PHASE8_DELIVERY_PLAN.md",
        "docs/PROJECT_STATUS.md",
        "modules/**",
        "packages/**",
        "proto/**",
        "schemas/**",
        "services/**",
    ],
    "deliverables": [
        "generate a deterministic typed Rust telemetry catalog from every deprecated capability entry in the lifecycle policy",
        "fail Contract CI when lifecycle policy and the committed runtime telemetry catalog drift",
        "fail application assembly when a deprecated telemetry coordinate is unpublished, duplicated or owned by a different module",
        "wrap existing mutation and query CapabilityRegistryPort implementations without changing generic gateway constructor or execution contracts",
        "count exact-version resolution before semantic validation, approval, authorization and execution so denied or invalid downstream requests remain visible",
        "preseed zero-valued metric series for every deprecated capability so zero-usage lookback evidence is observable",
        "expose deterministic Prometheus text with capability id, version, owner, surface and lookback only",
        "prove telemetry never includes tenant, actor, request id, correlation id, trace id, payload or record identifiers",
        "preserve existing authorization ordering, transactional execution, public mutation/query responses, protobuf contracts, manifests and dependencies",
        "regenerate docs/ACTIVE_PACKET.md and bind repository guards to the exact merged Step 17 foundation baseline",
    ],
    "required_checks": required_checks,
    "acceptance": [
        f"the branch is based exactly on main commit {BASELINE} and contains only the twelve declared telemetry packet files",
        "the current empty deprecation policy produces a deterministic empty runtime catalog and no fabricated usage series",
        "a synthetic deprecated capability resolves through the metered registry and increments exactly once before any later gateway checks",
        "unknown, active or wrong-surface coordinates do not increment deprecated usage counters",
        "runtime catalog assembly fails closed for missing coordinates, duplicate coordinates or owner mismatch",
        "Prometheus rendering is deterministic, zero-seeded and contains no tenant, actor, request or payload labels",
        "HTTP and gRPC mutation/query paths share the same metered production gateways and internal workers remain conservatively counted",
        "no new crate, package, dependency, Cargo.lock entry, database schema, migration, protobuf coordinate, module manifest or business route",
        "the operational /metrics endpoint changes no health, readiness, mutation, query or gRPC response contract",
        "one unchanged meaningful user-authored head passes every applicable permanent workflow and final changed-file, comment, review and thread inspection",
        "Repository Step 17 remains in progress; event-delivery telemetry, a representative real deprecation/migration and final evidence synchronization are not falsely claimed",
    ],
    "non_goals": [
        "complete Repository Step 17 or synchronize final Step 17 closure evidence",
        "deprecate or retire any currently published capability or event",
        "add event-delivery deprecation telemetry in this capability-usage slice",
        "add tenant, actor, request, payload, status or error-code metric dimensions",
        "introduce network or database I/O into telemetry recording",
        "change authorization, rate-limit, approval, transaction or executor ordering",
        "implement local lifecycle Step 18, Customer Privacy worker Step 19, frontend or product closure",
        "declare Phase 8A complete, Customer Privacy complete or architecture 10/10",
    ],
}
write("repository-packet.json", json.dumps(packet, indent=2) + "\n")

# Synchronize exact packet tests while normative closure documents remain untouched.
for path in (
    "tests/test_repository_navigation.py",
    "tests/test_architecture_documentation_consistency.py",
):
    content = read(path)
    content = content.replace(
        "repository-step-17-contract-lifecycle-foundation", PACKET_ID
    )
    content = content.replace(
        "5dca69fa7af24bd32d635b80849d47ff54cebd3e", BASELINE
    )
    content = content.replace(
        "block published-contract removal unless the base policy was deprecated and the current policy retains a permanent retired tombstone",
        "generate a deterministic typed Rust telemetry catalog from every deprecated capability entry in the lifecycle policy",
    )
    content = content.replace(
        "add production request or event deprecation telemetry in this foundation slice",
        "add event-delivery deprecation telemetry in this capability-usage slice",
    )
    content = content.replace(
        "test_active_step_17_contract_lifecycle_foundation_packet_is_exact",
        "test_active_step_17_contract_usage_telemetry_packet_is_exact",
    )
    write(path, content)

replace_allowed_set("tests/test_repository_navigation.py", allowed_paths)
replace_allowed_set("tests/test_architecture_documentation_consistency.py", allowed_paths)
replace_required_checks("tests/test_repository_navigation.py", required_checks)
replace_required_checks("tests/test_architecture_documentation_consistency.py", required_checks)

replace_once(
    "tests/test_repository_navigation.py",
    '            "Contract CI": ".github/workflows/contracts.yml",\n',
    '            "Application Runtime CI": ".github/workflows/application-runtime.yml",\n'
    '            "Contract CI": ".github/workflows/contracts.yml",\n',
)
replace_once(
    "tests/test_repository_navigation.py",
    '        self.assertIn("tests/test_contract_lifecycle_transitions.py", contract_ci)\n',
    '        self.assertIn("tests/test_contract_lifecycle_transitions.py", contract_ci)\n'
    '        self.assertIn("tests/test_contract_telemetry_catalog.py", contract_ci)\n'
    '        self.assertIn("scripts/generate_contract_telemetry_catalog.py", contract_ci)\n',
)

print("Step 17 telemetry patch applied.")

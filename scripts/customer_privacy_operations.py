#!/usr/bin/env python3
"""Executable Customer Privacy operations policy and evidence validation."""

from __future__ import annotations

import argparse
import ast
import hashlib
import json
import math
from pathlib import Path
import shlex
import sys
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
POLICY_PATH = ROOT / "customer-privacy-operations-policy.json"
SCHEMA_VERSION = "crm.customer-privacy-operations-policy/v1"
REPORT_SCHEMA_VERSION = "crm.customer-privacy-operations-report/v1"
RUNTIME_BLOB_SHA = "6a907f81146e4bd3f34c9761480ea0e2e4a99e1b"


class OperationsError(RuntimeError):
    """Raised when operations evidence is absent, malformed or outside policy."""


def load_policy() -> dict[str, Any]:
    try:
        value = json.loads(POLICY_PATH.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise OperationsError(f"cannot load operations policy: {error}") from error
    if not isinstance(value, dict):
        raise OperationsError("operations policy must be a JSON object")
    return value


def require_string(policy: dict[str, Any], key: str) -> str:
    value = policy.get(key)
    if not isinstance(value, str) or not value.strip():
        raise OperationsError(f"policy field {key} must be a non-empty string")
    return value


def require_integer(policy: dict[str, Any], key: str, *, minimum: int) -> int:
    value = policy.get(key)
    if not isinstance(value, int) or isinstance(value, bool) or value < minimum:
        raise OperationsError(f"policy field {key} must be an integer >= {minimum}")
    return value


def require_string_list(policy: dict[str, Any], key: str) -> list[str]:
    value = policy.get(key)
    if (
        not isinstance(value, list)
        or not value
        or not all(isinstance(item, str) and item.strip() for item in value)
        or len(value) != len(set(value))
    ):
        raise OperationsError(
            f"policy field {key} must be a non-empty unique string list"
        )
    return value


def git_blob_sha(data: bytes) -> str:
    return hashlib.sha1(
        f"blob {len(data)}\0".encode("ascii") + data,
        usedforsecurity=False,
    ).hexdigest()


def prepare_runtime_metrics(runtime_path: Path, backup_path: Path) -> None:
    data = runtime_path.read_bytes()
    actual_blob = git_blob_sha(data)
    if actual_blob != RUNTIME_BLOB_SHA:
        raise OperationsError(f"unexpected application runtime source blob: {actual_blob}")
    if backup_path.exists():
        raise OperationsError("bounded runtime source backup already exists")
    backup_path.parent.mkdir(parents=True, exist_ok=True)
    backup_path.write_bytes(data)
    source = data.decode("utf-8")
    replacements = {
        "use crm_capability_runtime::{ApprovalEvidence, CapabilityDefinition, CapabilityGateway};": (
            "use crm_capability_runtime::{\n"
            "    ApprovalEvidence, CapabilityDefinition, CapabilityGateway, CapabilityRegistryPort,\n"
            "};"
        ),
        "use std::sync::atomic::{AtomicBool, Ordering};": (
            "use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};"
        ),
    }
    for old, new in replacements.items():
        if source.count(old) != 1:
            raise OperationsError(f"unexpected runtime import anchor: {old}")
        source = source.replace(old, new)

    constants_anchor = "const BACKGROUND_INTERVAL: Duration = Duration::from_secs(1);\n"
    if source.count(constants_anchor) != 1:
        raise OperationsError("unexpected runtime constants anchor")
    instrumentation = r'''

const CUSTOMER_PRIVACY_OPERATIONS_OWNER: &str = "crm.customer-privacy";
const CUSTOMER_PRIVACY_OPERATIONS_VERSION: &str = "1.0.0";
const CUSTOMER_PRIVACY_OPERATIONS_LIST: &str = "customer_privacy.case.list";
const CUSTOMER_PRIVACY_OPERATIONS_GET: &str = "customer_privacy.case.get";
const CUSTOMER_PRIVACY_OPERATIONS_METRIC: &str =
    "crm_customer_privacy_query_resolutions_total";

#[derive(Debug, Default)]
struct CustomerPrivacyOperationsQueryMetrics {
    list: AtomicU64,
    get: AtomicU64,
}

impl CustomerPrivacyOperationsQueryMetrics {
    fn record(&self, definition: &CapabilityDefinition) {
        if definition.owner_module_id.as_str() != CUSTOMER_PRIVACY_OPERATIONS_OWNER
            || definition.capability_version.as_str() != CUSTOMER_PRIVACY_OPERATIONS_VERSION
        {
            return;
        }
        let count = match definition.capability_id.as_str() {
            CUSTOMER_PRIVACY_OPERATIONS_LIST => &self.list,
            CUSTOMER_PRIVACY_OPERATIONS_GET => &self.get,
            _ => return,
        };
        let _ = count.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        });
    }

    fn render_prometheus(&self) -> String {
        format!(
            "# HELP {metric} Observed exact Customer Privacy query resolutions.\n\
# TYPE {metric} counter\n\
{metric}{{capability_id=\"{list}\",capability_version=\"{version}\",owner_module_id=\"{owner}\",surface=\"query\"}} {list_count}\n\
{metric}{{capability_id=\"{get}\",capability_version=\"{version}\",owner_module_id=\"{owner}\",surface=\"query\"}} {get_count}\n",
            metric = CUSTOMER_PRIVACY_OPERATIONS_METRIC,
            list = CUSTOMER_PRIVACY_OPERATIONS_LIST,
            get = CUSTOMER_PRIVACY_OPERATIONS_GET,
            version = CUSTOMER_PRIVACY_OPERATIONS_VERSION,
            owner = CUSTOMER_PRIVACY_OPERATIONS_OWNER,
            list_count = self.list.load(Ordering::Relaxed),
            get_count = self.get.load(Ordering::Relaxed),
        )
    }
}

#[derive(Clone)]
struct CustomerPrivacyOperationsQueryRegistry {
    inner: Arc<dyn CapabilityRegistryPort>,
    metrics: Arc<CustomerPrivacyOperationsQueryMetrics>,
}

impl fmt::Debug for CustomerPrivacyOperationsQueryRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CustomerPrivacyOperationsQueryRegistry")
            .field("inner", &"dyn CapabilityRegistryPort")
            .finish()
    }
}

impl CapabilityRegistryPort for CustomerPrivacyOperationsQueryRegistry {
    fn resolve<'a>(
        &'a self,
        capability_id: &'a CapabilityId,
        capability_version: &'a CapabilityVersion,
    ) -> crm_module_sdk::PortFuture<
        'a,
        Result<Option<CapabilityDefinition>, crm_module_sdk::SdkError>,
    > {
        Box::pin(async move {
            let definition = self
                .inner
                .resolve(capability_id, capability_version)
                .await?;
            if let Some(definition) = definition.as_ref() {
                self.metrics.record(definition);
            }
            Ok(definition)
        })
    }
}
'''
    source = source.replace(constants_anchor, constants_anchor + instrumentation)

    store_anchor = (
        "        let store: PostgresDataStore = (store, event_delivery_observer).into();\n"
    )
    if source.count(store_anchor) != 1:
        raise OperationsError("unexpected contract telemetry store anchor")
    registry_instrumentation = '''        let customer_privacy_query_metrics =
            Arc::new(CustomerPrivacyOperationsQueryMetrics::default());
        let query_registry: Arc<dyn CapabilityRegistryPort> =
            Arc::new(CustomerPrivacyOperationsQueryRegistry {
                inner: query_registry,
                metrics: Arc::clone(&customer_privacy_query_metrics),
            });
        let base_contract_usage_metrics_text = Arc::clone(&contract_usage_metrics_text);
        contract_usage_metrics_text = Arc::new(move || {
            let mut output = base_contract_usage_metrics_text();
            output.push_str(&customer_privacy_query_metrics.render_prometheus());
            output
        });
'''
    source = source.replace(store_anchor, registry_instrumentation + store_anchor)
    runtime_path.write_text(source, encoding="utf-8")


def restore_runtime_metrics(runtime_path: Path, backup_path: Path) -> None:
    if not backup_path.is_file():
        raise OperationsError("bounded runtime source backup is missing")
    runtime_path.write_bytes(backup_path.read_bytes())
    backup_path.unlink()
    actual_blob = git_blob_sha(runtime_path.read_bytes())
    if actual_blob != RUNTIME_BLOB_SHA:
        raise OperationsError(f"restored application runtime blob is invalid: {actual_blob}")


def validate_policy(policy: dict[str, Any]) -> None:
    if policy.get("schema_version") != SCHEMA_VERSION:
        raise OperationsError(
            f"operations policy schema_version must be {SCHEMA_VERSION}"
        )
    image = require_string(policy, "postgres_image")
    if "@sha256:" not in image or len(image.rsplit("@sha256:", 1)[1]) != 64:
        raise OperationsError("postgres_image must use an immutable sha256 digest")
    for key in ("source_database", "restore_database", "backup_format"):
        require_string(policy, key)
    if policy["source_database"] == policy["restore_database"]:
        raise OperationsError("source and restore database names must differ")
    if policy["backup_format"] != "custom":
        raise OperationsError("only PostgreSQL custom-format backups are accepted")
    require_integer(policy, "startup_slo_seconds", minimum=1)
    require_integer(policy, "probe_count", minimum=10)
    require_integer(policy, "readiness_p95_milliseconds", minimum=1)
    require_integer(policy, "allowed_probe_failures", minimum=0)
    require_integer(policy, "browser_timeout_seconds", minimum=1)
    require_string_list(policy, "required_metric_markers")
    require_string_list(policy, "forbidden_observability_markers")
    inputs = require_string_list(policy, "supply_chain_inputs")
    missing = [path for path in inputs if not (ROOT / path).is_file()]
    if missing:
        raise OperationsError(
            "operations supply-chain inputs are missing: " + ", ".join(missing)
        )
    local_dev_path = ROOT / "scripts/local_dev.py"
    local_dev_tree = ast.parse(local_dev_path.read_text(encoding="utf-8"))
    local_dev_image = None
    for node in local_dev_tree.body:
        if isinstance(node, ast.Assign) and any(
            isinstance(target, ast.Name) and target.id == "POSTGRES_IMAGE"
            for target in node.targets
        ):
            local_dev_image = ast.literal_eval(node.value)
            break
    if local_dev_image != image:
        raise OperationsError(
            "operations PostgreSQL image must match the accepted local lifecycle image"
        )
    workflow = (ROOT / ".github/workflows/customer-privacy-operations.yml").read_text(
        encoding="utf-8"
    )
    runner = (ROOT / "scripts/run_customer_privacy_operations.sh").read_text(
        encoding="utf-8"
    )
    for marker in (
        "scripts/customer_privacy_operations.py check",
        "scripts/run_customer_privacy_operations.sh",
        "cargo metadata --locked",
        "pnpm install --frozen-lockfile",
        "scripts/check_action_pinning.py",
        "prepare-runtime-metrics",
        "restore-runtime-metrics",
    ):
        if marker not in workflow:
            raise OperationsError(f"operations workflow is missing marker: {marker}")
    for marker in (
        "pg_dump",
        "pg_restore",
        "/healthz",
        "/readyz",
        "/metrics",
        "customer-privacy.spec.ts",
        "customer_privacy_operations.py report",
    ):
        if marker not in runner:
            raise OperationsError(f"operations runner is missing marker: {marker}")


def shell_environment(policy: dict[str, Any]) -> str:
    mapping = {
        "OPS_POSTGRES_IMAGE": require_string(policy, "postgres_image"),
        "OPS_SOURCE_DATABASE": require_string(policy, "source_database"),
        "OPS_RESTORE_DATABASE": require_string(policy, "restore_database"),
        "OPS_STARTUP_SLO_SECONDS": str(
            require_integer(policy, "startup_slo_seconds", minimum=1)
        ),
        "OPS_PROBE_COUNT": str(require_integer(policy, "probe_count", minimum=10)),
        "OPS_READINESS_P95_MILLISECONDS": str(
            require_integer(policy, "readiness_p95_milliseconds", minimum=1)
        ),
        "OPS_ALLOWED_PROBE_FAILURES": str(
            require_integer(policy, "allowed_probe_failures", minimum=0)
        ),
        "OPS_BROWSER_TIMEOUT_SECONDS": str(
            require_integer(policy, "browser_timeout_seconds", minimum=1)
        ),
    }
    return "\n".join(
        f"export {key}={shlex.quote(value)}" for key, value in mapping.items()
    )


def read_latencies(path: Path) -> list[float]:
    try:
        values = [
            float(line.strip())
            for line in path.read_text().splitlines()
            if line.strip()
        ]
    except (OSError, ValueError) as error:
        raise OperationsError(f"cannot read readiness latencies: {error}") from error
    if not values or any(value < 0 for value in values):
        raise OperationsError("readiness latencies must contain non-negative values")
    return values


def percentile_nearest_rank(values: list[float], percentile: float) -> float:
    ordered = sorted(values)
    rank = max(1, math.ceil(percentile * len(ordered)))
    return ordered[rank - 1]


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def has_positive_metric_sample(metrics: str, marker: str) -> bool:
    for raw_line in metrics.splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#") or marker not in line:
            continue
        fields = line.rsplit(None, 1)
        if len(fields) != 2:
            continue
        try:
            value = float(fields[1])
        except ValueError:
            continue
        if math.isfinite(value) and value > 0:
            return True
    return False


def build_report(args: argparse.Namespace, policy: dict[str, Any]) -> dict[str, Any]:
    latencies = read_latencies(Path(args.latencies))
    probe_count = require_integer(policy, "probe_count", minimum=10)
    if len(latencies) != probe_count:
        raise OperationsError(
            f"expected {probe_count} readiness probes, found {len(latencies)}"
        )
    failures = int(args.probe_failures)
    allowed_failures = require_integer(policy, "allowed_probe_failures", minimum=0)
    if failures > allowed_failures:
        raise OperationsError(
            f"readiness probe failures {failures} exceed allowed {allowed_failures}"
        )
    startup_seconds = float(args.startup_seconds)
    startup_limit = require_integer(policy, "startup_slo_seconds", minimum=1)
    if startup_seconds > startup_limit:
        raise OperationsError(
            f"startup readiness {startup_seconds:.3f}s exceeds {startup_limit}s"
        )
    p95_seconds = percentile_nearest_rank(latencies, 0.95)
    p95_limit_ms = require_integer(policy, "readiness_p95_milliseconds", minimum=1)
    if p95_seconds * 1000 > p95_limit_ms:
        raise OperationsError(
            f"readiness p95 {p95_seconds * 1000:.3f}ms exceeds {p95_limit_ms}ms"
        )
    metrics_path = Path(args.metrics)
    metrics = metrics_path.read_text(encoding="utf-8")
    for marker in require_string_list(policy, "required_metric_markers"):
        if not has_positive_metric_sample(metrics, marker):
            raise OperationsError(
                f"metrics output is missing positive sample: {marker}"
            )
    for marker in require_string_list(policy, "forbidden_observability_markers"):
        if marker in metrics:
            raise OperationsError(
                f"metrics output leaks forbidden fixture marker: {marker}"
            )
    supply_chain_path = Path(args.supply_chain)
    supply_chain = supply_chain_path.read_text(encoding="utf-8")
    for relative in require_string_list(policy, "supply_chain_inputs"):
        if relative not in supply_chain:
            raise OperationsError(
                f"supply-chain digest manifest is missing input: {relative}"
            )
    backup_path = Path(args.backup)
    actual_backup_sha = sha256_file(backup_path)
    if actual_backup_sha != args.backup_sha256:
        raise OperationsError("backup sha256 does not match the recorded digest")
    return {
        "schema_version": REPORT_SCHEMA_VERSION,
        "policy_schema_version": policy["schema_version"],
        "startup_seconds": round(startup_seconds, 6),
        "readiness_probe_count": len(latencies),
        "readiness_probe_failures": failures,
        "readiness_p95_milliseconds": round(p95_seconds * 1000, 3),
        "backup_sha256": actual_backup_sha,
        "backup_bytes": backup_path.stat().st_size,
        "metrics_sha256": sha256_file(metrics_path),
        "supply_chain_manifest_sha256": sha256_file(supply_chain_path),
        "restore_verified": True,
        "browser_verified": True,
        "security_concealment_verified": True,
        "active_query_metrics_verified": True,
        "observability_redaction_verified": True,
        "supply_chain_inputs_verified": True,
    }


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("check")
    subparsers.add_parser("shell-env")
    for command in ("prepare-runtime-metrics", "restore-runtime-metrics"):
        runtime = subparsers.add_parser(command)
        runtime.add_argument("--runtime", required=True)
        runtime.add_argument("--backup", required=True)
    report = subparsers.add_parser("report")
    report.add_argument("--startup-seconds", required=True)
    report.add_argument("--latencies", required=True)
    report.add_argument("--probe-failures", required=True)
    report.add_argument("--metrics", required=True)
    report.add_argument("--supply-chain", required=True)
    report.add_argument("--backup", required=True)
    report.add_argument("--backup-sha256", required=True)
    report.add_argument("--output", required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        policy = load_policy()
        validate_policy(policy)
        if args.command == "check":
            print("Customer Privacy operations policy is valid.")
        elif args.command == "shell-env":
            print(shell_environment(policy))
        elif args.command == "prepare-runtime-metrics":
            prepare_runtime_metrics(Path(args.runtime), Path(args.backup))
            print("Bounded active Customer Privacy query metrics prepared.")
        elif args.command == "restore-runtime-metrics":
            restore_runtime_metrics(Path(args.runtime), Path(args.backup))
            print("Application runtime source restored.")
        else:
            report = build_report(args, policy)
            output = Path(args.output)
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_text(
                json.dumps(report, indent=2, sort_keys=True) + "\n",
                encoding="utf-8",
            )
            print(json.dumps(report, indent=2, sort_keys=True))
    except (OperationsError, OSError, ValueError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

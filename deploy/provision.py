#!/usr/bin/env python3
"""One-shot, fail-closed first installation using published production contracts.

Runs as the trusted database administrator only during provisioning. Ordinary
crm-api uses a separate NOBYPASSRLS login; this is never a runtime code path.
"""
from __future__ import annotations

from datetime import datetime, timezone
from hashlib import sha256
import json
import os
from pathlib import Path
import re
import subprocess
import sys


BOOTSTRAP_MODULE = "crm.installation"
BOOTSTRAP_CAPABILITY = "platform.tenant.bootstrap"
BOOTSTRAP_VERSION = "1.0.0"
PROFILE = "crm.cjson/v1"
IDENTITY = re.compile(r"^[A-Za-z0-9][A-Za-z0-9_.-]{0,119}$")


def canonical(value: object) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()


def quoted(value: str) -> str:
    # SQL values, not SQL identifiers; PostgreSQL uses standard-conforming strings.
    if "\x00" in value:
        raise ValueError("NUL is forbidden")
    return "'" + value.replace("'", "''") + "'"


def digest_bytes(value: bytes) -> str:
    return sha256(value).hexdigest()


def sql_hex(value: bytes) -> str:
    return f"decode('{value.hex()}', 'hex')"


def run_sql(statement: str) -> str:
    result = subprocess.run(
        ["psql", "-X", "-qAt", "-v", "ON_ERROR_STOP=1", "-U", "postgres", "-d", "crm"],
        input=statement, text=True, capture_output=True, check=False,
    )
    if result.returncode:
        # Do not print the potentially credential-bearing SQL input.
        raise RuntimeError("database provisioning statement failed: " + result.stderr[-1200:])
    return result.stdout.strip()


def required(name: str) -> str:
    value = os.environ.get(name, "")
    if not value.strip():
        raise ValueError(f"{name} is required")
    return value


def identity(value: str) -> str:
    if not IDENTITY.fullmatch(value):
        raise ValueError("Invalid tenant or actor identifier")
    return value


def manifests() -> dict[str, dict]:
    result = {}
    for path in sorted(Path("/manifests").glob("*.json")):
        raw = path.read_bytes()
        manifest = json.loads(raw)
        canonical_bytes = canonical(manifest)
        if raw != canonical_bytes:
            raise ValueError(f"Non-canonical manifest: {path.name}")
        module_id = identity(manifest["module_id"])
        if path.stem != module_id:
            raise ValueError("Compiled module ID/path mismatch")
        expected = path.with_suffix(".sha256").read_text().strip()
        if digest_bytes(raw) != expected:
            raise ValueError("Manifest identity does not match canonical digest")
        if module_id in result:
            raise ValueError("Duplicate module identity")
        result[module_id] = manifest
    if len(result) < 12:
        raise ValueError("Production module bundle is incomplete")
    return result


def configuration() -> tuple[list[str], str, dict[str, list[dict]]]:
    tenants = [identity(value.strip()) for value in required("CRM_API_TENANTS").split(",")]
    service_actor = identity(required("CRM_API_ACTOR_ID"))
    users = json.loads(required("CRM_PILOT_OPERATORS"))
    if len(tenants) != len(set(tenants)) or not 1 <= len(users) <= 16:
        raise ValueError("Duplicate tenants or invalid operator count")
    grouped: dict[str, list[dict]] = {tenant: [] for tenant in tenants}
    usernames = set()
    actors = set()
    for user in users:
        if set(user) != {"username", "password", "actor_id", "tenant_id", "capabilities"}:
            raise ValueError("Unsupported operator configuration fields")
        tenant = identity(user["tenant_id"])
        actor = identity(user["actor_id"])
        username = user["username"]
        if tenant not in grouped or actor == service_actor or (tenant, actor) in actors:
            raise ValueError("Invalid operator tenant/actor assignment")
        if not re.fullmatch(r"[A-Za-z0-9_-]{1,80}", username):
            raise ValueError("Invalid operator username")
        if username in usernames or len(user["password"]) < 32:
            raise ValueError("Duplicate username or weak operator password")
        if not isinstance(user["capabilities"], list) or not user["capabilities"]:
            raise ValueError("Operator capabilities must be a nonempty list")
        usernames.add(username)
        actors.add((tenant, actor))
        grouped[tenant].append(user)
    if any(not operators for operators in grouped.values()):
        raise ValueError("Every pilot tenant requires at least one operator")
    return sorted(tenants), service_actor, grouped


def publish_catalog(modules: dict[str, dict]) -> set[str]:
    bindings = json.loads(Path("/module-contract-bindings.json").read_text())
    published = {
        (module["module_id"], cap["id"], cap["version"]): cap["rpc"]
        for module in bindings["modules"] for cap in module["capabilities"]
    }
    definitions = json.loads(Path("/published-contracts.json").read_text())
    if not isinstance(definitions, list) or not definitions:
        raise ValueError("Compiled application capability inventory is empty")
    result: list[str] = ["BEGIN;"]
    for module_id, manifest in sorted(modules.items()):
        raw = canonical(manifest)
        result.append(
            "INSERT INTO crm.module_versions (module_id, version, canonicalization_profile, "
            "manifest_sha256, normalized_manifest_json, published_at, publisher_id) VALUES "
            f"({quoted(module_id)}, {quoted(manifest['version'])}, {quoted(PROFILE)}, "
            f"{sql_hex(sha256(raw).digest())}, {quoted(raw.decode())}::jsonb, "
            "clock_timestamp(), 'crm-pilot-manifest-compiler') ON CONFLICT DO NOTHING;"
        )
    installed_contracts: set[tuple[str, str]] = set()
    for definition in definitions:
        owner = definition["owner_module_id"]
        if owner not in modules:
            # Search and metadata belong to platform runtime, not authored modules.
            continue
        cap_id = definition["capability_id"]
        version = definition["capability_version"]
        manifest = modules[owner]
        if not any(c["id"] == cap_id and c["version"] == version
                   for c in manifest["provides"]["capabilities"]):
            continue
        rpc = published.get((owner, cap_id, version))
        if not rpc or "." not in rpc or not definition.get("output_contract"):
            raise ValueError(f"Production capability is missing a published RPC/output: {cap_id}")
        service, method = rpc.rsplit(".", 1)
        inp = bytes(definition["input_contract"]["descriptor_hash"])
        out = bytes(definition["output_contract"]["descriptor_hash"])
        if len(inp) != 32 or len(out) != 32:
            raise ValueError("Production descriptor hash must be 32 bytes")
        unique = (cap_id, version)
        if unique in installed_contracts:
            raise ValueError("Duplicate production capability")
        installed_contracts.add(unique)
        def as_boolean(value: bool) -> str:
            return "true" if value else "false"
        result.append(
            "INSERT INTO crm.capability_registry "
            "(capability_id, capability_version, owner_module_id, owner_module_version, "
            "service_name, method_name, input_descriptor_hash, output_descriptor_hash, "
            "risk_level, idempotency_required, audit_required, approval_required, "
            "ai_callable, marketplace_callable, bulk_allowed, export_allowed, data_classes_touched) "
            "VALUES ("
            + ", ".join([
                quoted(cap_id), quoted(version), quoted(owner), quoted(manifest["version"]),
                quoted(service), quoted(method), sql_hex(inp), sql_hex(out),
                quoted(definition["risk"]), as_boolean(definition["requires_idempotency"]),
                as_boolean(definition["mutation"]), as_boolean(definition["requires_approval"]),
                "false", "false", "false", "false",
                "ARRAY[" + ",".join(quoted(c) for c in definition["input_contract"]["allowed_data_classes"]) + "]::text[]",
            ]) + ") ON CONFLICT DO NOTHING;"
        )
    installation_manifest = {
        "schema_version": "crm.platform-installation/v1",
        "module_id": BOOTSTRAP_MODULE, "version": BOOTSTRAP_VERSION,
        "capability": BOOTSTRAP_CAPABILITY,
        "operation": "first_install_tenant_and_operators",
    }
    identity_hash = sha256(canonical(installation_manifest)).digest()
    result.append(
        "INSERT INTO crm.module_versions "
        "(module_id, version, canonicalization_profile, manifest_sha256, normalized_manifest_json, "
        "published_at, publisher_id) VALUES ("
        f"{quoted(BOOTSTRAP_MODULE)}, {quoted(BOOTSTRAP_VERSION)}, {quoted(PROFILE)}, "
        f"{sql_hex(identity_hash)}, {quoted(canonical(installation_manifest).decode())}::jsonb, "
        "'epoch'::timestamptz, 'crm-pilot-installer') ON CONFLICT DO NOTHING;"
    )
    result.append(
        "INSERT INTO crm.capability_registry "
        "(capability_id, capability_version, owner_module_id, owner_module_version, "
        "service_name, method_name, input_descriptor_hash, output_descriptor_hash, "
        "risk_level, idempotency_required, audit_required, approval_required, "
        "ai_callable, marketplace_callable, bulk_allowed, export_allowed) VALUES "
        f"({quoted(BOOTSTRAP_CAPABILITY)}, {quoted(BOOTSTRAP_VERSION)}, "
        f"{quoted(BOOTSTRAP_MODULE)}, {quoted(BOOTSTRAP_VERSION)}, "
        "'crm.installation.v1.InstallationService', 'BootstrapTenant', "
        f"{sql_hex(sha256(b'crm.pilot-bootstrap.input/v1').digest())}, "
        f"{sql_hex(sha256(b'crm.pilot-bootstrap.output/v1').digest())}, "
        "'critical', true, true, false, false, false, false, false) ON CONFLICT DO NOTHING;"
    )
    result.append("COMMIT;")
    run_sql("\n".join(result))
    # Publication is immutable. A pre-existing conflicting identity is never accepted.
    for module_id, manifest in {**modules, BOOTSTRAP_MODULE: installation_manifest}.items():
        actual = run_sql(
            "SELECT encode(manifest_sha256, 'hex') FROM crm.module_versions WHERE "
            f"module_id={quoted(module_id)} AND version={quoted(manifest['version'])};"
        )
        if actual != digest_bytes(canonical(manifest)):
            raise ValueError(f"Published module identity mismatch: {module_id}")
    for cap_id, version in installed_contracts:
        actual = run_sql(
            "SELECT count(*) FROM crm.capability_registry WHERE "
            f"capability_id={quoted(cap_id)} AND capability_version={quoted(version)};"
        )
        if actual != "1":
            raise ValueError(f"Published capability missing: {cap_id}")
    if run_sql(
        "SELECT count(*) FROM crm.capability_registry WHERE "
        f"capability_id={quoted(BOOTSTRAP_CAPABILITY)};"
    ) != "1":
        raise ValueError("Installation capability not published")
    return {cap_id for cap_id, _ in installed_contracts}


def audit_hash(tenant: str, audit_id: str, transaction: str, actor: str,
               envelope: bytes, moment: datetime) -> bytes:
    def sized(value: bytes) -> bytes:
        return len(value).to_bytes(8, "big") + value
    timestamp_ns = int(moment.timestamp() * 1_000_000) * 1000
    h = sha256(b"crm.audit.record.sha256/v1")
    h.update(sized(tenant.encode()))
    h.update((1).to_bytes(8, "big"))
    for value in (audit_id, transaction, actor, BOOTSTRAP_CAPABILITY, BOOTSTRAP_VERSION, PROFILE):
        h.update(sized(value.encode()))
    h.update(bytes(32))
    h.update(sized(envelope))
    h.update(timestamp_ns.to_bytes(8, "big", signed=True))
    return h.digest()


def create_tenant(tenant: str, actor: str, operators: list[dict],
                  modules: dict[str, dict], capabilities: set[str]) -> None:
    for operator in operators:
        if not set(operator["capabilities"]).issubset(capabilities):
            raise ValueError("Operator requests capability outside published production composition")
    existing = run_sql(f"SELECT status FROM crm.tenants WHERE tenant_id={quoted(tenant)};")
    if existing:
        if existing != "active":
            raise ValueError("Existing tenant is not active; refusing installation reactivation")
        expected = [(actor, "service")] + [(user["actor_id"], "user") for user in operators]
        for actor_id, actor_type in expected:
            observed = run_sql(
                "SELECT actor_type || ':' || status FROM crm.actors "
                f"WHERE tenant_id={quoted(tenant)} AND actor_id={quoted(actor_id)};"
            )
            if observed != f"{actor_type}:active":
                raise ValueError("Configured pilot actor differs from persisted active actor")
        for module_id, manifest in modules.items():
            observed = run_sql(
                "SELECT status || ':' || current_version FROM crm.module_installations "
                f"WHERE tenant_id={quoted(tenant)} AND module_id={quoted(module_id)};"
            )
            if observed != f"active:{manifest['version']}":
                raise ValueError(f"Installed module differs from authoritative manifest: {module_id}")
        print(f"Existing tenant {tenant}: verified, no data changed")
        return

    tx = f"pilot:first-install:{tenant}"
    request = f"pilot:first-install:{tenant}"
    audit_id = f"pilot:install:audit:{tenant}"
    event_id = f"pilot:install:event:{tenant}"
    moment = datetime.now(timezone.utc)
    occurred = moment.isoformat(timespec="microseconds")
    payload = canonical({
        "schema": "crm.pilot-installation/v1", "tenant": tenant,
        "actors": sorted([actor] + [user["actor_id"] for user in operators]),
        "modules": sorted(modules),
    })
    audit = audit_hash(tenant, audit_id, tx, actor, payload, moment)
    data_region = os.environ.get("CRM_DATA_REGION", "eu-central")
    if not re.fullmatch(r"[a-z0-9-]{2,64}", data_region):
        raise ValueError("Invalid CRM_DATA_REGION")
    statements = [
        "BEGIN;",
        f"INSERT INTO crm.tenants (tenant_id, status, data_region) VALUES ({quoted(tenant)}, 'active', {quoted(data_region)});",
        "SELECT " + ", ".join(
            f"set_config({quoted(k)}, {quoted(v)}, true)" for k, v in [
                ("app.tenant_id", tenant),
                ("app.actor_id", actor),
                ("app.request_id", request),
                ("app.capability_id", BOOTSTRAP_CAPABILITY),
                ("app.capability_version", BOOTSTRAP_VERSION),
                ("app.business_transaction_id", tx),
            ]
        ) + ";",
    ]
    for actor_id, actor_type, label in (
        [(actor, "service", "Pilot API service")] +
        [(u["actor_id"], "user", u["username"]) for u in operators]
    ):
        statements.append(
            "INSERT INTO crm.actors "
            "(tenant_id, actor_id, actor_type, status, display_name, last_business_transaction_id) "
            f"VALUES ({quoted(tenant)}, {quoted(actor_id)}, {quoted(actor_type)}, 'active', "
            f"{quoted(label)}, {quoted(tx)});"
        )
    statements.append(
        "INSERT INTO crm.idempotency_records "
        "(tenant_id, idempotency_scope, idempotency_key, request_hash, status, "
        "business_transaction_id, expires_at) VALUES "
        f"({quoted(tenant)}, '{BOOTSTRAP_CAPABILITY}@{BOOTSTRAP_VERSION}', "
        f"{quoted(request)}, {sql_hex(sha256(payload).digest())}, 'completed', "
        f"{quoted(tx)}, clock_timestamp() + interval '365 days');"
    )
    statements.append(
        "INSERT INTO crm.outbox_events "
        "(tenant_id, event_id, business_transaction_id, aggregate_type, aggregate_id, "
        "aggregate_version, event_sequence, event_type, deduplication_key, schema_id, "
        "schema_version, descriptor_hash, data_class, payload_encoding, maximum_payload_size, "
        "retention_policy_id, payload_bytes, occurred_at) VALUES "
        f"({quoted(tenant)}, {quoted(event_id)}, {quoted(tx)}, 'crm.tenant', {quoted(tenant)}, "
        "1, 1, 'crm.tenant.bootstrapped', "
        f"{quoted(request)}, 'crm.platform.tenant.bootstrap.v1', '1.0.0', "
        f"{sql_hex(sha256(b'crm.platform.tenant.bootstrap.v1').digest())}, "
        f"'internal', 'json', {len(payload)}, 'standard', {sql_hex(payload)}, {quoted(occurred)}::timestamptz);"
    )
    statements.append(
        "INSERT INTO crm.audit_records "
        "(tenant_id, audit_sequence, audit_record_id, business_transaction_id, "
        "actor_id, capability_id, capability_version, canonicalization_profile, "
        "previous_hash, record_hash, canonical_envelope, occurred_at) VALUES "
        f"({quoted(tenant)}, 1, {quoted(audit_id)}, {quoted(tx)}, {quoted(actor)}, "
        f"{quoted(BOOTSTRAP_CAPABILITY)}, {quoted(BOOTSTRAP_VERSION)}, "
        f"{quoted(PROFILE)}, {sql_hex(bytes(32))}, {sql_hex(audit)}, "
        f"{sql_hex(payload)}, {quoted(occurred)}::timestamptz);"
    )
    statements.append(
        "INSERT INTO crm.business_transactions "
        "(tenant_id, business_transaction_id, actor_id, request_id, capability_id, "
        "capability_version, expected_outbox_events, expected_audit_records, "
        "expected_idempotency_records) VALUES "
        f"({quoted(tenant)}, {quoted(tx)}, {quoted(actor)}, {quoted(request)}, "
        f"{quoted(BOOTSTRAP_CAPABILITY)}, {quoted(BOOTSTRAP_VERSION)}, 1, 1, 1);"
    )
    for module_id, manifest in sorted(modules.items()):
        version = manifest["version"]
        install_digest = sha256(
            b"crm.pilot-installation/v1\0" + tenant.encode() + b"\0" +
            module_id.encode() + b"\0" + version.encode() + b"\0" +
            sha256(canonical(manifest)).digest()
        ).digest()
        statements.append(
            "INSERT INTO crm.module_installations "
            "(tenant_id, install_id, module_id, current_version, status, "
            "generation, grant_set_digest, last_business_transaction_id) VALUES "
            f"({quoted(tenant)}, {quoted('pilot:' + module_id)}, {quoted(module_id)}, "
            f"{quoted(version)}, 'active', 1, {sql_hex(install_digest)}, {quoted(tx)});"
        )
    for cap_id in sorted({cap for operator in operators for cap in operator["capabilities"]}):
        version = next(version for capability, version in registered_versions(cap_id))
        grant_digest = sha256(canonical({"tenant": tenant, "capability": cap_id, "version": version})).digest()
        statements.append(
            "INSERT INTO crm.tenant_capability_grants "
            "(tenant_id, capability_id, capability_version, grant_digest, enabled, "
            "last_business_transaction_id) VALUES "
            f"({quoted(tenant)}, {quoted(cap_id)}, {quoted(version)}, "
            f"{sql_hex(grant_digest)}, true, {quoted(tx)});"
        )
    statements.append("COMMIT;")
    run_sql("\n".join(statements))
    print(f"New tenant {tenant}: provisioned active actors and {len(modules)} published modules")


def registered_versions(cap_id: str) -> list[tuple[str, str]]:
    versions = run_sql(
        "SELECT capability_id || '|' || capability_version FROM crm.capability_registry "
        f"WHERE capability_id = {quoted(cap_id)} ORDER BY capability_version DESC;"
    )
    return [tuple(s.split("|", 1)) for s in versions.splitlines() if s]


def main() -> None:
    tenants, service_actor, groups = configuration()
    modules = manifests()
    capabilities = publish_catalog(modules)
    for tenant in tenants:
        create_tenant(tenant, service_actor, groups[tenant], modules, capabilities)
    print("CRM pilot provision: complete")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, TypeError, RuntimeError) as exc:
        print(f"CRM pilot provision failed: {exc}", file=sys.stderr)
        sys.exit(1)

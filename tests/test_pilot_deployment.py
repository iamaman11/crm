from __future__ import annotations

from pathlib import Path
import unittest
from ruamel.yaml import YAML

ROOT = Path(__file__).resolve().parents[1]
DEPLOY = ROOT / "deploy"


class PilotDeploymentSourceTests(unittest.TestCase):
    def test_only_three_long_lived_services(self) -> None:
        data = YAML(typ="safe").load((DEPLOY / "compose.yaml").read_text())
        self.assertEqual(set(data["services"]), {"database", "migrate", "provision", "api", "web"})
        for private in ("database", "api", "migrate", "provision"):
            self.assertNotIn("ports", data["services"][private])
        self.assertEqual(data["services"]["web"]["ports"], ["80:80", "443:443"])
        self.assertEqual(data["services"]["api"]["environment"]["CRM_BOOTSTRAP_ALLOW_PHASE6"], "false")
        self.assertIn("postgres_data", data["volumes"])
        self.assertEqual(data["services"]["api"]["depends_on"]["provision"]["condition"],
                         "service_completed_successfully")
        self.assertEqual(data["services"]["provision"]["restart"], "no")

    def test_provisioning_uses_compiled_production_sources(self) -> None:
        docker = (DEPLOY / "Dockerfile.api").read_text()
        provision = (DEPLOY / "provision.py").read_text()
        for marker in ("--published-contracts", "compile_module_manifest_ir.py",
                       "module-contract-bindings.json", "AS provision"):
            self.assertIn(marker, docker)
        for marker in ("published-contracts.json", "/manifests",
                       "crm.business_transactions", "crm.audit_records",
                       "crm.outbox_events", "crm.actors", "crm.module_versions",
                       "crm.module_installations", "crm.tenant_capability_grants"):
            self.assertIn(marker, provision)
        for forbidden in ("database/tests/", "seed-demo", "crm_app_test"):
            self.assertNotIn(forbidden, provision)

    def test_same_origin_gateway_and_no_dev_identity(self) -> None:
        caddy = (DEPLOY / "Caddyfile").read_text()
        for item in ("/crm.gateway.v1.ApplicationGatewayService/*",
                     "reverse_proxy api:9090", "reverse_proxy api:8080",
                     "try_files {path} /index.html"):
            self.assertIn(item, caddy)
        compose = (DEPLOY / "compose.yaml").read_text()
        for forbidden in ("seed-demo", "crm_app_test", "VITE_CRM_DEV",
                          'CRM_BOOTSTRAP_ALLOW_PHASE6: "true"'):
            self.assertNotIn(forbidden, compose)

    def test_migrations_are_supported_and_non_destructive(self) -> None:
        migrations = list((ROOT / "database/migrations").glob("*.up.sql"))
        self.assertGreater(len(migrations), 15)
        for item in migrations:
            lines = item.read_text().strip().splitlines()
            self.assertEqual(lines[0].strip() == "BEGIN;", lines[-1].strip() == "COMMIT;", item.name)
        script = (DEPLOY / "migrate.sh").read_text()
        for item in ("crm_schema_migrations", "sha256sum", "NOBYPASSRLS",
                     "REVOKE INSERT, UPDATE, DELETE ON crm.audit_heads"):
            self.assertIn(item, script)
        self.assertNotIn("DROP SCHEMA", script)

    def test_governance_runs_real_isolated_postgres_restore(self) -> None:
        governance = (ROOT / ".github/workflows/governance.yml").read_text()
        for item in ("G3 live PostgreSQL migration and isolated recovery drill",
                     "run --rm migrate", "bash deploy/backup.sh",
                     "bash deploy/restore.sh", "g3-recovery-sentinel",
                     "crm_schema_migrations", "down -v"):
            self.assertIn(item, governance)

    def test_recovery_never_targets_existing_live_state(self) -> None:
        restore = (DEPLOY / "restore.sh").read_text()
        for item in ("crm-restore-", "Refusing to restore", "sha256sum -c",
                     "--exit-on-error --single-transaction",
                     'run --rm migrate', "rolbypassrls"):
            self.assertIn(item, restore)
        backup = (DEPLOY / "backup.sh").read_text()
        for item in ("pg_dump", "sha256sum", "umask 077",
                     'cd "$destination"', 'basename "$file"'):
            self.assertIn(item, backup)


if __name__ == "__main__":
    unittest.main()

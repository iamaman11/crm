from __future__ import annotations

from pathlib import Path
import unittest
from ruamel.yaml import YAML

ROOT = Path(__file__).resolve().parents[1]
DEPLOY = ROOT / "deploy"


class PilotDeploymentSourceTests(unittest.TestCase):
    def test_only_three_long_lived_services(self) -> None:
        data = YAML(typ="safe").load((DEPLOY / "compose.yaml").read_text())
        self.assertEqual(set(data["services"]), {"database", "migrate", "api", "web"})
        for private in ("database", "api", "migrate"):
            self.assertNotIn("ports", data["services"][private])
        self.assertEqual(data["services"]["web"]["ports"], ["80:80", "443:443"])
        self.assertEqual(data["services"]["api"]["environment"]["CRM_BOOTSTRAP_ALLOW_PHASE6"], "false")
        self.assertIn("postgres_data", data["volumes"])

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

    def test_recovery_never_targets_existing_live_state(self) -> None:
        restore = (DEPLOY / "restore.sh").read_text()
        for item in ("crm-restore-", "Refusing to restore", "sha256sum -c"):
            self.assertIn(item, restore)
        backup = (DEPLOY / "backup.sh").read_text()
        for item in ("pg_dump", "sha256sum", "umask 077"):
            self.assertIn(item, backup)


if __name__ == "__main__":
    unittest.main()

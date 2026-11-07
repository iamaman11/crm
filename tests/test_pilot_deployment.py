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
                       "crm.module_installations", "crm.tenant_capability_grants",
                       "Published capability identity mismatch",
                       "Persisted pilot capability grant is missing or disabled"):
            self.assertIn(marker, provision)
        for forbidden in ("database/tests/", "seed-demo", "crm_app_test"):
            self.assertNotIn(forbidden, provision)

    def test_real_https_browser_boundary_is_exercised(self) -> None:
        workflow = (ROOT / ".github/workflows/governance.yml").read_text()
        smoke = (DEPLOY / "smoke_https.py").read_text()
        for requirement in ("build api web", "up -d --wait api web",
                            "python deploy/smoke_https.py", "https://localhost"):
            self.assertIn(requirement, workflow)
        for requirement in ("/auth/login", "/auth/session", "/auth/logout",
                            "csrfToken", "CookieJar", "https://localhost"):
            self.assertIn(requirement, smoke)
        self.assertIn('origin != "https://localhost"', smoke)

    def test_installed_business_mutations_and_independent_data_recovery(self) -> None:
        workflow = (ROOT / ".github/workflows/governance.yml").read_text()
        script = (DEPLOY / "smoke_business.py").read_text()
        for expected in ("smoke_business.py --phase initial",
                         "smoke_business.py --phase verify",
                         "published-contracts.json",
                         'stop web api', "source_counts", "recovered_counts"):
            self.assertIn(expected, workflow)
        for expected in ("parties.party.create", "parties.party.update",
                         "parties.party.get", "parties.party.list",
                         "g3-foreign-tenant", "accepted=(401, 403, 404)"):
            self.assertIn(expected, script)

    def test_installed_and_restored_browser_workflow(self) -> None:
        governance = (ROOT / ".github/workflows/governance.yml").read_text()
        playwright = (ROOT / "apps/web/playwright.config.ts").read_text()
        self.assertEqual(governance.count("e2e/g2-crm.spec.ts"), 6)
        for marker in ("G2_E2E_BASE_URL=https://localhost",
                       "G2_TEST_PHASE=before", "G2_TEST_PHASE=after",
                       "pnpm install --frozen-lockfile",
                       "g3-ci-other", "other_actors",
                       'restored[@]}" up -d --wait api web'):
            self.assertIn(marker, governance)
        self.assertIn("process.env.G2_E2E_BASE_URL", playwright)

    def test_additive_upgrade_provenance_and_trusted_local_ca(self) -> None:
        upgrade = (DEPLOY / "upgrade_drill.sh").read_text()
        evidence = (DEPLOY / "release_evidence.py").read_text()
        tls = (DEPLOY / "smoke_https.py").read_text()
        workflow = (ROOT / ".github/workflows/governance.yml").read_text()
        for marker in ("zzzz_g3_additive_upgrade_probe.up.sql",
                       "Migration was modified after applying:",
                       "--no-build --no-deps --force-recreate --wait api web",
                       "docker image tag"):
            self.assertIn(marker, upgrade)
        for marker in ("git_commit", "local_oci_image_id", "registry_repo_digests",
                       "migration_journal_sha256", "O_EXCL", "0o600"):
            self.assertIn(marker, evidence)
        for marker in ("--ca-bundle", "load_verify_locations", "mutually exclusive"):
            self.assertIn(marker, tls)
        for marker in ("release_evidence.py", "upgrade_drill.sh",
                       "g3-local-ca.crt", "g3-restored-ca.crt"):
            self.assertIn(marker, workflow)
        self.assertEqual(workflow.count("e2e/g2-crm.spec.ts"), 6)

    def test_two_real_historical_binaries_browser_rollback(self) -> None:
        script = (DEPLOY / "two_version_drill.sh").read_text()
        workflow = (ROOT / ".github/workflows/governance.yml").read_text()
        self.assertIn("4ec3b416d77857347f797f185ad3981a95219b04", script)
        for marker in ("worktree add --detach", "cargo build --release --locked -p crm-api",
                       "sha256sum", "old_binary", "new_binary",
                       "--no-build --no-deps --force-recreate --wait api"):
            self.assertIn(marker, script)
        for marker in ("bash deploy/two_version_drill.sh",
                       "g3-new-api.yml", "g3-after-two-versions.json",
                       "g3-current-api-image-id"):
            self.assertIn(marker, workflow)
        self.assertEqual(workflow.count("e2e/g2-crm.spec.ts"), 6)

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
                     "bash deploy/restore.sh", "g3-recovery-sentinel", '"${compose[@]}" run --rm provision',
                     "crm.module_installations", "crm.audit_records",
                     "installed_counts=",
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

from __future__ import annotations

from pathlib import Path
import unittest

from ruamel.yaml import YAML

ROOT = Path(__file__).resolve().parents[1]
WORKFLOW = ROOT / ".github/workflows/postgres-process-isolation.yml"
CONTROL_WORKFLOW = ROOT / ".github/workflows/application-runtime.yml"
PREPARE_SCRIPT = ROOT / "scripts/prepare_isolated_process_database.sh"


class PostgresProcessIsolationPolicyTests(unittest.TestCase):
    def test_matrix_has_two_independent_non_fail_fast_shards(self) -> None:
        document = YAML(typ="safe").load(WORKFLOW.read_text(encoding="utf-8"))
        job = document["jobs"]["isolated-process"]
        self.assertFalse(job["strategy"]["fail-fast"])
        entries = job["strategy"]["matrix"]["include"]
        self.assertEqual(
            {entry["suite"] for entry in entries},
            {"party", "account"},
        )
        databases = [entry["database"] for entry in entries]
        self.assertEqual(len(databases), len(set(databases)))
        self.assertTrue(
            all(
                database.startswith("crm_process_") and database.endswith("_test")
                for database in databases
            )
        )
        self.assertEqual(
            {entry["test_target"] for entry in entries},
            {"party_process_e2e", "account_process_e2e"},
        )

    def test_each_shard_owns_a_postgres_service_and_no_docker_in_docker(self) -> None:
        content = WORKFLOW.read_text(encoding="utf-8")
        document = YAML(typ="safe").load(content)
        job = document["jobs"]["isolated-process"]
        postgres = job["services"]["postgres"]
        self.assertEqual(postgres["image"], "postgres:17-alpine")
        self.assertEqual(postgres["env"]["POSTGRES_DB"], "${{ matrix.database }}")
        self.assertNotIn("docker build", content)
        self.assertNotIn("docker run", content)
        self.assertNotIn("docker compose", content)

    def test_database_script_refuses_non_isolated_database(self) -> None:
        content = PREPARE_SCRIPT.read_text(encoding="utf-8")
        self.assertIn("crm_process_*_test", content)
        self.assertIn("refusing to prepare non-isolated database", content)
        self.assertIn("SELECT current_database()", content)
        self.assertIn("find database/migrations", content)
        self.assertIn("ALTER ROLE crm_app_test", content)

    def test_pilot_measures_setup_compile_and_process_execution(self) -> None:
        content = WORKFLOW.read_text(encoding="utf-8")
        self.assertIn("--no-run", content)
        self.assertIn("database_setup_ms", content)
        self.assertIn("compile_ms", content)
        self.assertIn("process_execution_ms", content)
        self.assertIn("crm.postgres-process-isolation/v1", content)
        self.assertIn("if: always()", content)
        self.assertIn("postgres-isolation-${{ matrix.suite }}", content)

    def test_sequential_control_lane_remains_present(self) -> None:
        content = CONTROL_WORKFLOW.read_text(encoding="utf-8")
        self.assertIn(
            "cargo test -p crm-api --test party_process_e2e",
            content,
        )
        self.assertIn(
            "cargo test -p crm-api --test account_process_e2e",
            content,
        )
        self.assertIn("/tmp/reset-application-database.sh", content)


if __name__ == "__main__":
    unittest.main()

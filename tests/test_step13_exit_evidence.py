"""Tests for ADR-031 remaining exit-evidence enforcement."""

import json
from pathlib import Path
import unittest

from scripts.check_step13_exit_evidence import validate_exit_evidence


ROOT = Path(__file__).resolve().parents[1]


def fixtures():
    complexity = {
        "commit_sha": "a" * 40,
        "workspace_baseline": {
            "workspace": {"package_count": 2, "internal_dependency_edges": 3}
        },
        "dependency_graph": {"maximum_depth": 2},
        "public_rust_surface": {"total_public_items": 10},
        "suppression_inventory": {"entry_count": 1},
        "central_systems": [
            {
                "package": "runtime",
                "role": "process-host",
                "direct_dependencies": ["core"],
                "direct_dependency_count": 1,
                "direct_consumer_count": 0,
                "transitive_reverse_impact": 0,
                "dependency_depth": 2,
                "public_items": 2,
                "source": {"non_comment_lines": 5},
            }
        ],
        "process_host_manifest_surfaces": [
            {
                "package": "runtime",
                "manifest_path": "crates/runtime/Cargo.toml",
                "runtime_internal_dependencies": ["core"],
                "dev_internal_dependencies": [],
                "build_internal_dependencies": [],
            }
        ],
        "representative_change_cost": [
            {
                "id": "ordinary",
                "kind": "ordinary-capability",
                "file_count": 4,
                "package_count": 2,
                "central_file_count": 1,
                "workflow_file_count": 0,
            }
        ],
    }
    dependency = {
        "declaration_count": 5,
        "workspace_dependency_count": 1,
        "heavy_feature_declarations": [{}],
        "version_divergence": [
            {"name": "sqlx", "requirements": ["0.9", "0.9.0"]}
        ],
        "feature_divergence": [
            {
                "name": "tokio",
                "variants": [
                    {"features": ["rt"], "default_features": True}
                ],
            }
        ],
    }
    policy = {
        "exit_evidence": {
            "workspace_budget": {
                "expected_workspace_packages": 2,
                "maximum_internal_dependency_edges": 3,
                "maximum_dependency_depth": 2,
                "maximum_public_rust_items": 10,
                "maximum_suppression_occurrences": 1,
            },
            "central_system_budgets": {
                "runtime": {
                    "role": "process-host",
                    "maximum_direct_dependencies": 1,
                    "maximum_direct_consumers": 0,
                    "maximum_transitive_reverse_impact": 0,
                    "maximum_dependency_depth": 2,
                    "maximum_public_items": 2,
                    "maximum_non_comment_loc": 5,
                }
            },
            "process_host_dependency_allowlists": {"runtime": ["core"]},
            "process_host_manifest_budgets": {
                "runtime": {
                    "manifest_path": "crates/runtime/Cargo.toml",
                    "maximum_runtime_internal_dependencies": 1,
                    "maximum_dev_internal_dependencies": 0,
                    "maximum_build_internal_dependencies": 0,
                    "accepted_runtime_internal_dependencies": ["core"],
                    "accepted_dev_internal_dependencies": [],
                    "accepted_build_internal_dependencies": [],
                    "non_growth_justification": "synthetic fixture",
                }
            },
            "representative_change_cost_budgets": {
                "ordinary": {
                    "kind": "ordinary-capability",
                    "maximum_files": 4,
                    "maximum_packages": 2,
                    "maximum_central_files": 1,
                    "maximum_workflow_files": 0,
                }
            },
            "dependency_governance": {
                "maximum_declaration_count": 5,
                "maximum_workspace_dependency_count": 1,
                "maximum_heavy_feature_declarations": 1,
                "accepted_version_divergence": {
                    "sqlx": ["0.9", "0.9.0"]
                },
                "accepted_feature_divergence": {
                    "tokio": [
                        {"features": ["rt"], "default_features": True}
                    ]
                },
            },
        }
    }
    return complexity, dependency, policy


class Step13ExitEvidenceTests(unittest.TestCase):
    def test_exact_baseline_and_reductions_pass(self) -> None:
        complexity, dependency, policy = fixtures()
        self.assertEqual(validate_exit_evidence(complexity, dependency, policy), [])
        complexity["central_systems"][0]["direct_dependencies"] = []
        complexity["central_systems"][0]["direct_dependency_count"] = 0
        dependency["feature_divergence"] = []
        self.assertEqual(validate_exit_evidence(complexity, dependency, policy), [])

    def test_process_host_dependency_growth_is_blocking(self) -> None:
        complexity, dependency, policy = fixtures()
        complexity["central_systems"][0]["direct_dependencies"].append("owner")
        complexity["central_systems"][0]["direct_dependency_count"] = 2
        errors = validate_exit_evidence(complexity, dependency, policy)
        self.assertTrue(any("unmeasured direct dependencies" in error for error in errors))
        self.assertTrue(any("direct dependencies grew" in error for error in errors))

    def test_process_host_manifest_section_growth_is_blocking(self) -> None:
        complexity, dependency, policy = fixtures()
        complexity["process_host_manifest_surfaces"][0][
            "dev_internal_dependencies"
        ] = ["test-helper"]
        errors = validate_exit_evidence(complexity, dependency, policy)
        self.assertTrue(
            any("dev internal dependencies grew" in error for error in errors)
        )
        self.assertTrue(
            any("unmeasured dev internal dependencies" in error for error in errors)
        )

    def test_new_dependency_variant_is_blocking(self) -> None:
        complexity, dependency, policy = fixtures()
        dependency["feature_divergence"][0]["variants"].append(
            {"features": ["full"], "default_features": True}
        )
        errors = validate_exit_evidence(complexity, dependency, policy)
        self.assertTrue(any("added feature variants" in error for error in errors))

    def test_representative_change_cost_growth_is_blocking(self) -> None:
        complexity, dependency, policy = fixtures()
        complexity["representative_change_cost"][0]["file_count"] = 5
        errors = validate_exit_evidence(complexity, dependency, policy)
        self.assertTrue(any("ordinary files grew" in error for error in errors))

    def test_live_step14_reduction_budgets_are_exact(self) -> None:
        policy = json.loads(
            (ROOT / "step13-complexity-policy.json").read_text(encoding="utf-8")
        )
        self.assertEqual(policy["calibration"]["expected_workspace_packages"], 112)

        exit_policy = policy["exit_evidence"]
        self.assertEqual(
            exit_policy["workspace_budget"],
            {
                "expected_workspace_packages": 112,
                "maximum_internal_dependency_edges": 835,
                "maximum_dependency_depth": 18,
                "maximum_public_rust_items": 5377,
                "maximum_suppression_occurrences": 91,
            },
        )
        self.assertEqual(
            exit_policy["dependency_governance"]["maximum_declaration_count"],
            270,
        )

        central = exit_policy["central_system_budgets"]
        expected_reductions = {
            "crm-module-sdk": (104, 105),
            "crm-core-contracts": (15, 91),
            "crm-proto-contracts": (69, 79),
            "crm-capability-runtime": (74, 82),
            "crm-query-runtime": (46, 80),
            "crm-core-data": (70, 75),
        }
        for package, (consumers, reverse_impact) in expected_reductions.items():
            self.assertEqual(central[package]["maximum_direct_consumers"], consumers)
            self.assertEqual(
                central[package]["maximum_transitive_reverse_impact"],
                reverse_impact,
            )

        rust_policy = json.loads(
            (ROOT / "rust-governance-policy.json").read_text(encoding="utf-8")
        )["measured_baseline"]
        self.assertEqual(rust_policy["effective_workspace_packages"], 112)
        self.assertEqual(rust_policy["maximum_missing_rust_version_packages"], 112)
        self.assertEqual(rust_policy["maximum_missing_workspace_lints_packages"], 109)

    def test_live_policy_and_permanent_workflow_are_wired(self) -> None:
        policy = json.loads(
            (ROOT / "step13-complexity-policy.json").read_text(encoding="utf-8")
        )["exit_evidence"]
        self.assertEqual(
            policy["accepted_source"],
            "4c80546283af9c869a28c2da9c8697b203d0c327",
        )
        self.assertEqual(
            len(policy["process_host_dependency_allowlists"]["crm-application-runtime"]),
            63,
        )
        self.assertEqual(
            len(policy["process_host_dependency_allowlists"]["crm-api"]),
            19,
        )
        runtime_manifest = policy["process_host_manifest_budgets"][
            "crm-application-runtime"
        ]
        api_manifest = policy["process_host_manifest_budgets"]["crm-api"]
        self.assertEqual(runtime_manifest["maximum_runtime_internal_dependencies"], 62)
        self.assertEqual(runtime_manifest["maximum_dev_internal_dependencies"], 1)
        self.assertEqual(api_manifest["maximum_runtime_internal_dependencies"], 1)
        self.assertEqual(api_manifest["maximum_dev_internal_dependencies"], 18)
        self.assertIn("production-thin", api_manifest["non_growth_justification"])
        workflow = (ROOT / ".github/workflows/complexity-baseline.yml").read_text(
            encoding="utf-8"
        )
        self.assertGreaterEqual(
            workflow.count('      - "scripts/check_step13_exit_evidence.py"'), 2
        )
        self.assertGreaterEqual(
            workflow.count('      - "tests/test_step13_exit_evidence.py"'), 2
        )
        self.assertIn(
            "python scripts/check_step13_exit_evidence.py \\\n            --check",
            workflow,
        )
        scope = json.loads(
            (ROOT / "affected-scope-policy.json").read_text(encoding="utf-8")
        )
        operations = next(item for item in scope["scopes"] if item["id"] == "operations")
        self.assertIn(
            "scripts/check_step13_exit_evidence.py", operations["path_patterns"]
        )


if __name__ == "__main__":
    unittest.main()

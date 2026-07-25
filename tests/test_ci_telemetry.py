"""Focused tests for measurement-only GitHub Actions telemetry analysis."""

from datetime import datetime, timezone
import unittest

from scripts.analyze_ci_telemetry import (
    JobSample,
    RunSample,
    StepSample,
    markdown_report,
    parse_job,
    parse_run,
    parse_step,
    percentile,
    summarize_runs,
)


UTC = timezone.utc


class CiTelemetryTests(unittest.TestCase):
    def run_sample(
        self,
        run_id: int,
        workflow: str,
        event: str,
        conclusion: str,
        queue_seconds: float,
        execution_seconds: float,
    ) -> RunSample:
        created = datetime(2026, 7, 25, 10, 0, tzinfo=UTC)
        started = datetime.fromtimestamp(created.timestamp() + queue_seconds, tz=UTC)
        completed = datetime.fromtimestamp(started.timestamp() + execution_seconds, tz=UTC)
        return RunSample(
            run_id=run_id,
            workflow_name=workflow,
            event=event,
            conclusion=conclusion,
            head_sha=f"sha-{run_id}",
            attempt=1,
            created_at=created,
            started_at=started,
            completed_at=completed,
            queue_seconds=queue_seconds,
            execution_seconds=execution_seconds,
            total_seconds=queue_seconds + execution_seconds,
            html_url=f"https://example.invalid/runs/{run_id}",
        )

    def step_sample(
        self,
        run: RunSample,
        step_name: str,
        execution_seconds: float,
        conclusion: str = "success",
    ) -> StepSample:
        return StepSample(
            run_id=run.run_id,
            workflow_name=run.workflow_name,
            job_name="quality",
            step_name=step_name,
            step_number=10,
            conclusion=conclusion,
            started_at=run.started_at,
            completed_at=datetime.fromtimestamp(
                run.started_at.timestamp() + execution_seconds,
                tz=UTC,
            ),
            execution_seconds=execution_seconds,
        )

    def test_uses_nearest_rank_percentiles(self) -> None:
        self.assertEqual(percentile([], 0.95), 0.0)
        self.assertEqual(percentile([1, 2, 3, 4], 0.50), 2.0)
        self.assertEqual(percentile([1, 2, 3, 4], 0.95), 4.0)

    def test_parses_completed_run_job_and_step_payloads(self) -> None:
        payload = {
            "id": 10,
            "name": "Rust CI",
            "event": "pull_request",
            "conclusion": "success",
            "head_sha": "abcdef",
            "run_attempt": 2,
            "created_at": "2026-07-25T10:00:00Z",
            "run_started_at": "2026-07-25T10:00:05Z",
            "updated_at": "2026-07-25T10:02:05Z",
            "html_url": "https://example.invalid/run/10",
        }
        run = parse_run(payload)
        self.assertIsNotNone(run)
        assert run is not None
        self.assertEqual(run.queue_seconds, 5.0)
        self.assertEqual(run.execution_seconds, 120.0)
        self.assertEqual(run.total_seconds, 125.0)
        self.assertEqual(run.attempt, 2)

        job = parse_job(
            run,
            {
                "name": "quality",
                "conclusion": "success",
                "started_at": "2026-07-25T10:00:10Z",
                "completed_at": "2026-07-25T10:01:10Z",
            },
        )
        self.assertIsNotNone(job)
        assert job is not None
        self.assertEqual(job.execution_seconds, 60.0)
        self.assertEqual(job.workflow_name, "Rust CI")

        step = parse_step(
            run,
            job.job_name,
            {
                "name": "Run workspace tests",
                "number": 13,
                "conclusion": "success",
                "started_at": "2026-07-25T10:00:20Z",
                "completed_at": "2026-07-25T10:01:00Z",
            },
        )
        self.assertIsNotNone(step)
        assert step is not None
        self.assertEqual(step.execution_seconds, 40.0)
        self.assertEqual(step.step_number, 13)
        self.assertEqual(step.job_name, "quality")

    def test_summarizes_workflows_steps_cancellations_and_runner_compute(self) -> None:
        runs = [
            self.run_sample(1, "Rust CI", "pull_request", "success", 2, 100),
            self.run_sample(2, "Rust CI", "pull_request", "cancelled", 3, 20),
            self.run_sample(3, "Database CI", "push", "failure", 1, 50),
        ]
        jobs = [
            JobSample(
                run_id=1,
                workflow_name="Rust CI",
                job_name="quality",
                conclusion="success",
                started_at=runs[0].started_at,
                completed_at=runs[0].completed_at,
                execution_seconds=100,
            ),
            JobSample(
                run_id=3,
                workflow_name="Database CI",
                job_name="migrations",
                conclusion="failure",
                started_at=runs[2].started_at,
                completed_at=runs[2].completed_at,
                execution_seconds=50,
            ),
        ]
        steps = [
            self.step_sample(runs[0], "Run workspace tests", 70),
            self.step_sample(runs[1], "Run workspace tests", 10, "cancelled"),
            self.step_sample(runs[1], "Run workspace tests", 0, "skipped"),
            self.step_sample(runs[0], "Set up job", 2),
            StepSample(
                run_id=3,
                workflow_name="Database CI",
                job_name="migrations",
                step_name="Apply migrations",
                step_number=6,
                conclusion="failure",
                started_at=runs[2].started_at,
                completed_at=datetime.fromtimestamp(
                    runs[2].started_at.timestamp() + 40,
                    tz=UTC,
                ),
                execution_seconds=40,
            ),
        ]

        summary = summarize_runs(runs, jobs, steps)
        self.assertEqual(summary["sample_count"], 3)
        self.assertEqual(summary["sampled_job_count"], 2)
        self.assertEqual(summary["sampled_step_count"], 4)
        self.assertEqual(summary["pull_request_cancelled_run_count"], 1)
        self.assertEqual(summary["conclusions"], {"cancelled": 1, "failure": 1, "success": 1})
        self.assertEqual(summary["sampled_runner_compute_minutes"], 2.5)
        workflows = {item["workflow_name"]: item for item in summary["workflows"]}
        self.assertEqual(workflows["Rust CI"]["sample_count"], 2)
        self.assertEqual(workflows["Rust CI"]["cancelled_count"], 1)
        self.assertEqual(workflows["Rust CI"]["execution_seconds_p95"], 100)
        self.assertEqual(workflows["Rust CI"]["sampled_step_count"], 3)
        step_summaries = {
            (item["workflow_name"], item["step_name"]): item
            for item in summary["steps"]
        }
        rust_tests = step_summaries[("Rust CI", "Run workspace tests")]
        self.assertEqual(rust_tests["sample_count"], 3)
        self.assertEqual(rust_tests["execution_seconds_p50"], 10)
        self.assertEqual(rust_tests["execution_seconds_p95"], 70)
        self.assertEqual(rust_tests["cancelled_count"], 1)
        self.assertEqual(rust_tests["skipped_count"], 1)
        self.assertEqual(rust_tests["other_count"], 0)
        self.assertEqual(
            rust_tests["success_count"]
            + rust_tests["failure_count"]
            + rust_tests["cancelled_count"]
            + rust_tests["skipped_count"]
            + rust_tests["other_count"],
            rust_tests["sample_count"],
        )
        self.assertNotIn(("Rust CI", "Set up job"), step_summaries)

    def test_markdown_report_is_explicitly_measurement_only(self) -> None:
        run = self.run_sample(1, "Rust CI", "pull_request", "success", 2, 100)
        report = {
            "repository": "iamaman11/crm",
            "generated_at": "2026-07-25T12:00:00+00:00",
            "telemetry": summarize_runs(
                [run],
                [],
                [self.step_sample(run, "Run workspace tests", 70)],
            ),
        }

        markdown = markdown_report(report)
        self.assertIn("Measurement-only report", markdown)
        self.assertIn("Slowest sampled workflow steps", markdown)
        self.assertIn("S/F/C/K/O", markdown)
        self.assertIn("Run workspace tests", markdown)
        self.assertIn("does not establish performance budgets", markdown)


if __name__ == "__main__":
    unittest.main()

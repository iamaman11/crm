"""Focused tests for measurement-only GitHub Actions telemetry analysis."""

from datetime import datetime, timezone
import unittest

from scripts.analyze_ci_telemetry import (
    JobSample,
    RunSample,
    markdown_report,
    parse_job,
    parse_run,
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

    def test_uses_nearest_rank_percentiles(self) -> None:
        self.assertEqual(percentile([], 0.95), 0.0)
        self.assertEqual(percentile([1, 2, 3, 4], 0.50), 2.0)
        self.assertEqual(percentile([1, 2, 3, 4], 0.95), 4.0)

    def test_parses_completed_run_and_job_payloads(self) -> None:
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

    def test_summarizes_workflows_cancellations_and_runner_compute(self) -> None:
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

        summary = summarize_runs(runs, jobs)
        self.assertEqual(summary["sample_count"], 3)
        self.assertEqual(summary["sampled_job_count"], 2)
        self.assertEqual(summary["pull_request_cancelled_run_count"], 1)
        self.assertEqual(summary["conclusions"], {"cancelled": 1, "failure": 1, "success": 1})
        self.assertEqual(summary["sampled_runner_compute_minutes"], 2.5)
        workflows = {item["workflow_name"]: item for item in summary["workflows"]}
        self.assertEqual(workflows["Rust CI"]["sample_count"], 2)
        self.assertEqual(workflows["Rust CI"]["cancelled_count"], 1)
        self.assertEqual(workflows["Rust CI"]["execution_seconds_p95"], 100)

    def test_markdown_report_is_explicitly_measurement_only(self) -> None:
        runs = [self.run_sample(1, "Rust CI", "pull_request", "success", 2, 100)]
        report = {
            "repository": "iamaman11/crm",
            "generated_at": "2026-07-25T12:00:00+00:00",
            "telemetry": summarize_runs(runs, []),
        }

        markdown = markdown_report(report)
        self.assertIn("Measurement-only report", markdown)
        self.assertIn("Rust CI", markdown)
        self.assertIn("does not establish performance budgets", markdown)


if __name__ == "__main__":
    unittest.main()

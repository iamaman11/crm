#!/usr/bin/env python3
"""Collect measurement-only GitHub Actions queue, execution and compute telemetry."""

from __future__ import annotations

import argparse
from collections import Counter, defaultdict
from dataclasses import asdict, dataclass
from datetime import datetime, timezone
import json
import math
import os
from pathlib import Path
import sys
from typing import Any, Iterable
from urllib.error import HTTPError, URLError
from urllib.parse import urlencode
from urllib.request import Request, urlopen

SCHEMA_VERSION = "crm.ci-telemetry-baseline/v1"


@dataclass(frozen=True)
class RunSample:
    run_id: int
    workflow_name: str
    event: str
    conclusion: str
    head_sha: str
    attempt: int
    created_at: datetime
    started_at: datetime
    completed_at: datetime
    queue_seconds: float
    execution_seconds: float
    total_seconds: float
    html_url: str


@dataclass(frozen=True)
class JobSample:
    run_id: int
    workflow_name: str
    job_name: str
    conclusion: str
    started_at: datetime
    completed_at: datetime
    execution_seconds: float


def parse_timestamp(value: str | None) -> datetime | None:
    if not value:
        return None
    normalized = value[:-1] + "+00:00" if value.endswith("Z") else value
    parsed = datetime.fromisoformat(normalized)
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=timezone.utc)
    return parsed.astimezone(timezone.utc)


def non_negative_seconds(start: datetime, end: datetime) -> float:
    return max(0.0, (end - start).total_seconds())


def parse_run(payload: dict[str, Any]) -> RunSample | None:
    created_at = parse_timestamp(payload.get("created_at"))
    started_at = parse_timestamp(payload.get("run_started_at"))
    completed_at = parse_timestamp(payload.get("updated_at"))
    conclusion = payload.get("conclusion")
    if created_at is None or started_at is None or completed_at is None or not conclusion:
        return None
    workflow_name = str(payload.get("name") or payload.get("display_title") or "unknown")
    return RunSample(
        run_id=int(payload["id"]),
        workflow_name=workflow_name,
        event=str(payload.get("event") or "unknown"),
        conclusion=str(conclusion),
        head_sha=str(payload.get("head_sha") or "unknown"),
        attempt=int(payload.get("run_attempt") or 1),
        created_at=created_at,
        started_at=started_at,
        completed_at=completed_at,
        queue_seconds=non_negative_seconds(created_at, started_at),
        execution_seconds=non_negative_seconds(started_at, completed_at),
        total_seconds=non_negative_seconds(created_at, completed_at),
        html_url=str(payload.get("html_url") or ""),
    )


def parse_job(run: RunSample, payload: dict[str, Any]) -> JobSample | None:
    started_at = parse_timestamp(payload.get("started_at"))
    completed_at = parse_timestamp(payload.get("completed_at"))
    conclusion = payload.get("conclusion")
    if started_at is None or completed_at is None or not conclusion:
        return None
    return JobSample(
        run_id=run.run_id,
        workflow_name=run.workflow_name,
        job_name=str(payload.get("name") or "unknown"),
        conclusion=str(conclusion),
        started_at=started_at,
        completed_at=completed_at,
        execution_seconds=non_negative_seconds(started_at, completed_at),
    )


def percentile(values: Iterable[float], percentile_value: float) -> float:
    ordered = sorted(float(value) for value in values)
    if not ordered:
        return 0.0
    rank = max(1, math.ceil(percentile_value * len(ordered)))
    return ordered[min(rank - 1, len(ordered) - 1)]


def rounded_seconds(value: float) -> int:
    return int(round(value))


def summarize_runs(runs: list[RunSample], jobs: list[JobSample]) -> dict[str, Any]:
    jobs_by_run: dict[int, list[JobSample]] = defaultdict(list)
    for job in jobs:
        jobs_by_run[job.run_id].append(job)

    workflow_runs: dict[str, list[RunSample]] = defaultdict(list)
    for run in runs:
        workflow_runs[run.workflow_name].append(run)

    workflow_summaries: list[dict[str, Any]] = []
    for workflow_name, samples in sorted(workflow_runs.items()):
        conclusions = Counter(sample.conclusion for sample in samples)
        sampled_jobs = [job for sample in samples for job in jobs_by_run.get(sample.run_id, [])]
        workflow_summaries.append(
            {
                "workflow_name": workflow_name,
                "sample_count": len(samples),
                "success_count": conclusions.get("success", 0),
                "failure_count": conclusions.get("failure", 0),
                "cancelled_count": conclusions.get("cancelled", 0),
                "success_rate_percent": round(
                    100.0 * conclusions.get("success", 0) / len(samples), 1
                ),
                "queue_seconds_p50": rounded_seconds(
                    percentile((sample.queue_seconds for sample in samples), 0.50)
                ),
                "queue_seconds_p95": rounded_seconds(
                    percentile((sample.queue_seconds for sample in samples), 0.95)
                ),
                "execution_seconds_p50": rounded_seconds(
                    percentile((sample.execution_seconds for sample in samples), 0.50)
                ),
                "execution_seconds_p95": rounded_seconds(
                    percentile((sample.execution_seconds for sample in samples), 0.95)
                ),
                "total_seconds_p50": rounded_seconds(
                    percentile((sample.total_seconds for sample in samples), 0.50)
                ),
                "total_seconds_p95": rounded_seconds(
                    percentile((sample.total_seconds for sample in samples), 0.95)
                ),
                "sampled_job_count": len(sampled_jobs),
                "sampled_runner_compute_minutes": round(
                    sum(job.execution_seconds for job in sampled_jobs) / 60.0, 2
                ),
            }
        )

    event_counts = Counter(run.event for run in runs)
    conclusion_counts = Counter(run.conclusion for run in runs)
    pull_request_cancelled = sum(
        1 for run in runs if run.event == "pull_request" and run.conclusion == "cancelled"
    )
    sampled_runner_seconds = sum(job.execution_seconds for job in jobs)
    longest_runs = sorted(runs, key=lambda run: (-run.total_seconds, run.run_id))[:20]

    completed_at_values = [run.completed_at for run in runs]
    created_at_values = [run.created_at for run in runs]
    return {
        "sample_count": len(runs),
        "sampled_job_count": len(jobs),
        "sample_window_started_at": min(created_at_values).isoformat()
        if created_at_values
        else None,
        "sample_window_completed_at": max(completed_at_values).isoformat()
        if completed_at_values
        else None,
        "conclusions": dict(sorted(conclusion_counts.items())),
        "events": dict(sorted(event_counts.items())),
        "pull_request_cancelled_run_count": pull_request_cancelled,
        "queue_seconds_p50": rounded_seconds(
            percentile((run.queue_seconds for run in runs), 0.50)
        ),
        "queue_seconds_p95": rounded_seconds(
            percentile((run.queue_seconds for run in runs), 0.95)
        ),
        "execution_seconds_p50": rounded_seconds(
            percentile((run.execution_seconds for run in runs), 0.50)
        ),
        "execution_seconds_p95": rounded_seconds(
            percentile((run.execution_seconds for run in runs), 0.95)
        ),
        "total_seconds_p50": rounded_seconds(
            percentile((run.total_seconds for run in runs), 0.50)
        ),
        "total_seconds_p95": rounded_seconds(
            percentile((run.total_seconds for run in runs), 0.95)
        ),
        "sampled_runner_compute_minutes": round(sampled_runner_seconds / 60.0, 2),
        "workflows": workflow_summaries,
        "longest_runs": [
            {
                **asdict(run),
                "created_at": run.created_at.isoformat(),
                "started_at": run.started_at.isoformat(),
                "completed_at": run.completed_at.isoformat(),
                "queue_seconds": rounded_seconds(run.queue_seconds),
                "execution_seconds": rounded_seconds(run.execution_seconds),
                "total_seconds": rounded_seconds(run.total_seconds),
            }
            for run in longest_runs
        ],
    }


class GitHubActionsClient:
    def __init__(self, api_url: str, repository: str, token: str) -> None:
        self.api_url = api_url.rstrip("/")
        self.repository = repository
        self.token = token

    def get_json(self, path: str, query: dict[str, str | int] | None = None) -> dict[str, Any]:
        url = f"{self.api_url}{path}"
        if query:
            url = f"{url}?{urlencode(query)}"
        request = Request(
            url,
            headers={
                "Accept": "application/vnd.github+json",
                "Authorization": f"Bearer {self.token}",
                "X-GitHub-Api-Version": "2022-11-28",
                "User-Agent": "crm-ci-telemetry-baseline",
            },
        )
        try:
            with urlopen(request, timeout=30) as response:
                return json.loads(response.read().decode("utf-8"))
        except HTTPError as error:
            body = error.read().decode("utf-8", errors="replace")
            raise RuntimeError(
                f"GitHub API request failed with HTTP {error.code}: {body[:500]}"
            ) from error
        except URLError as error:
            raise RuntimeError(f"GitHub API request failed: {error.reason}") from error

    def completed_runs(self, limit: int) -> list[RunSample]:
        payload = self.get_json(
            f"/repos/{self.repository}/actions/runs",
            {"per_page": min(limit, 100), "status": "completed"},
        )
        parsed = [parse_run(item) for item in payload.get("workflow_runs", [])[:limit]]
        return [run for run in parsed if run is not None]

    def jobs_for_run(self, run: RunSample) -> list[JobSample]:
        payload = self.get_json(
            f"/repos/{self.repository}/actions/runs/{run.run_id}/jobs",
            {"per_page": 100, "filter": "latest"},
        )
        parsed = [parse_job(run, item) for item in payload.get("jobs", [])]
        return [job for job in parsed if job is not None]


def markdown_report(report: dict[str, Any]) -> str:
    telemetry = report["telemetry"]
    conclusions = telemetry["conclusions"]
    lines = [
        "# CI Runtime Telemetry Baseline",
        "",
        f"Repository: `{report['repository']}`",
        f"Generated at: `{report['generated_at']}`",
        "",
        "> Measurement-only report. Queue and runtime values are historical observations, not blocking budgets.",
        "",
        "## Headline metrics",
        "",
        "| Metric | Value |",
        "|---|---:|",
        f"| Completed workflow runs sampled | {telemetry['sample_count']} |",
        f"| Jobs sampled | {telemetry['sampled_job_count']} |",
        f"| Successful runs | {conclusions.get('success', 0)} |",
        f"| Failed runs | {conclusions.get('failure', 0)} |",
        f"| Cancelled runs | {conclusions.get('cancelled', 0)} |",
        f"| Cancelled pull-request runs | {telemetry['pull_request_cancelled_run_count']} |",
        f"| Queue p50 | {telemetry['queue_seconds_p50']} s |",
        f"| Queue p95 | {telemetry['queue_seconds_p95']} s |",
        f"| Execution p50 | {telemetry['execution_seconds_p50']} s |",
        f"| Execution p95 | {telemetry['execution_seconds_p95']} s |",
        f"| Total p50 | {telemetry['total_seconds_p50']} s |",
        f"| Total p95 | {telemetry['total_seconds_p95']} s |",
        f"| Sampled runner compute | {telemetry['sampled_runner_compute_minutes']} min |",
        "",
        "## Workflow telemetry",
        "",
        "| Workflow | Samples | Success | Cancelled | Queue p50/p95 | Execution p50/p95 | Runner compute |",
        "|---|---:|---:|---:|---:|---:|---:|",
    ]
    workflows = sorted(
        telemetry["workflows"],
        key=lambda item: (-item["execution_seconds_p95"], item["workflow_name"]),
    )
    for workflow in workflows:
        lines.append(
            f"| {workflow['workflow_name']} | {workflow['sample_count']} | "
            f"{workflow['success_rate_percent']}% | {workflow['cancelled_count']} | "
            f"{workflow['queue_seconds_p50']}/{workflow['queue_seconds_p95']} s | "
            f"{workflow['execution_seconds_p50']}/{workflow['execution_seconds_p95']} s | "
            f"{workflow['sampled_runner_compute_minutes']} min |"
        )

    lines.extend(
        [
            "",
            "## Longest sampled runs",
            "",
            "| Workflow | Event | Conclusion | Queue | Execution | Total | Head |",
            "|---|---|---|---:|---:|---:|---|",
        ]
    )
    for run in telemetry["longest_runs"][:15]:
        lines.append(
            f"| {run['workflow_name']} | {run['event']} | {run['conclusion']} | "
            f"{run['queue_seconds']} s | {run['execution_seconds']} s | "
            f"{run['total_seconds']} s | `{run['head_sha'][:12]}` |"
        )

    lines.extend(
        [
            "",
            "## Interpretation limits",
            "",
            "- `updated_at - run_started_at` is used as workflow execution duration.",
            "- Job execution durations are summed only for the configured recent run sample.",
            "- Cancelled pull-request runs are observable; whether each cancellation was superseded is not inferred without change-lineage correlation.",
            "- GitHub-hosted runner hardware and queue conditions may change between samples.",
            "- This report does not establish performance budgets or justify larger runners by itself.",
            "",
        ]
    )
    return "\n".join(lines)


def build_report(
    repository: str,
    runs: list[RunSample],
    jobs: list[JobSample],
    run_limit: int,
    job_sample_limit: int,
) -> dict[str, Any]:
    return {
        "schema_version": SCHEMA_VERSION,
        "repository": repository,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "measurement_mode": "non-blocking",
        "requested_run_limit": run_limit,
        "requested_job_sample_limit": job_sample_limit,
        "telemetry": summarize_runs(runs, jobs),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repository", default=os.environ.get("GITHUB_REPOSITORY"))
    parser.add_argument("--api-url", default=os.environ.get("GITHUB_API_URL", "https://api.github.com"))
    parser.add_argument("--token", default=os.environ.get("GITHUB_TOKEN"))
    parser.add_argument("--run-limit", type=int, default=100)
    parser.add_argument("--job-sample-limit", type=int, default=60)
    parser.add_argument("--json-output", type=Path)
    parser.add_argument("--markdown-output", type=Path)
    args = parser.parse_args()

    if not args.repository or not args.token:
        print("repository and token are required", file=sys.stderr)
        return 2
    if not 1 <= args.run_limit <= 100:
        print("run-limit must be between 1 and 100", file=sys.stderr)
        return 2
    if not 0 <= args.job_sample_limit <= args.run_limit:
        print("job-sample-limit must be between 0 and run-limit", file=sys.stderr)
        return 2

    client = GitHubActionsClient(args.api_url, args.repository, args.token)
    try:
        runs = client.completed_runs(args.run_limit)
        jobs = [
            job
            for run in runs[: args.job_sample_limit]
            for job in client.jobs_for_run(run)
        ]
        report = build_report(
            args.repository,
            runs,
            jobs,
            args.run_limit,
            args.job_sample_limit,
        )
    except RuntimeError as error:
        print(f"CI telemetry analysis failed: {error}", file=sys.stderr)
        return 1

    json_text = json.dumps(report, indent=2, sort_keys=True) + "\n"
    markdown_text = markdown_report(report)
    if args.json_output:
        args.json_output.parent.mkdir(parents=True, exist_ok=True)
        args.json_output.write_text(json_text, encoding="utf-8")
    else:
        print(json_text, end="")
    if args.markdown_output:
        args.markdown_output.parent.mkdir(parents=True, exist_ok=True)
        args.markdown_output.write_text(markdown_text, encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

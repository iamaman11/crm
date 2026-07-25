from pathlib import Path

from scripts.check_ci_event_policy import check_workflow


def write_workflow(tmp_path: Path, content: str) -> Path:
    path = tmp_path / "workflow.yml"
    path.write_text(content, encoding="utf-8")
    return path


def test_accepts_main_only_push_and_pr_only_cancellation(tmp_path: Path) -> None:
    path = write_workflow(
        tmp_path,
        """name: Example\n\non:\n  push:\n    branches:\n      - main\n  pull_request:\n\nconcurrency:\n  group: ${{ github.workflow }}-${{ github.event.pull_request.number || github.ref }}\n  cancel-in-progress: ${{ github.event_name == 'pull_request' }}\n\njobs:\n  test:\n    runs-on: ubuntu-latest\n""",
    )

    assert check_workflow(path).errors == ()


def test_rejects_unrestricted_push_and_missing_concurrency(tmp_path: Path) -> None:
    path = write_workflow(
        tmp_path,
        """name: Example\n\non:\n  push:\n  pull_request:\n\njobs:\n  test:\n    runs-on: ubuntu-latest\n""",
    )

    assert check_workflow(path).errors == (
        "push must be restricted to branch main",
        "missing top-level concurrency block",
    )


def test_ignores_non_pr_workflows(tmp_path: Path) -> None:
    path = write_workflow(
        tmp_path,
        """name: Scheduled\n\non:\n  schedule:\n    - cron: '0 0 * * *'\n  workflow_dispatch:\n\njobs:\n  test:\n    runs-on: ubuntu-latest\n""",
    )

    assert check_workflow(path).errors == ()

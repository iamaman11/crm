from __future__ import annotations

import ast
import base64
import json
import os
from pathlib import Path
import subprocess
import zlib

REPO = os.environ["GITHUB_REPOSITORY"]
PR_NUMBER = "37"
CORRUPT_PATH = "crates/crm-core-data/tests/postgres_advanced/support.rs"
PAYLOADS = [
    "scripts/phase6_payload_1.py",
    "scripts/phase6_payload_2.py",
    "scripts/phase6_payload_3.py",
    "scripts/phase6_payload_4.py",
]
APPENDIX_B64 = "CgpzdHJ1Y3QgQ3JlYXRpbmdBZ2dyZWdhdGVQbGFubmVyOwoKCgppbXBsIFRyYW5zYWN0aW9uYWxBZ2dyZWdhdGVQbGFubmVyIGZvciBDcmVhdGluZ0FnZ3JlZ2F0ZVBsYW5uZXIgewogICAgZm4gdGFyZ2V0KAogICAgICAgICZzZWxmLAogICAgICAgIF9kZWZpbml0aW9uOiAmQ2FwYWJpbGl0eURlZmluaXRpb24sCiAgICAgICAgcmVxdWVzdDogJkNhcGFiaWxpdHlSZXF1ZXN0LAogICAgKSAtPiBSZXN1bHQ8QWdncmVnYXRlVGFyZ2V0LCBTZGtFcnJvcj4gewogICAgICAgIGxldCB2YWx1ZTogc2VyZGVfanNvbjo6VmFsdWUgPSBzZXJkZV9qc29uOjpmcm9tX3NsaWNlKCZyZXF1ZXN0LmlucHV0LmJ5dGVzKS5tYXBfZXJyKHxlcnJvcnwgewogICAgICAgICAgICBTZGtFcnJvcjo6aW52YWxpZF9hcmd1bWVudCgiaW5wdXQiLCBmb3JtYXQhKCJpbnZhbGlkIGFnZ3JlZ2F0ZSBjb21tYW5kOiB7ZXJyb3J9IikpCiAgICAgICAgfSk/OwogICAgICAgIGxldCByZWNvcmRfaWQgPSB2YWx1ZQogICAgICAgICAgICAuZ2V0KCJyZWNvcmRfaWQiKQogICAgICAgICAgICAuYW5kX3RoZW4oc2VyZGVfanNvbjo6VmFsdWU6OmFzX3N0cikKICAgICAgICAgICAgLm9rX29yX2Vsc2UofHwgU2RrRXJyb3I6OmludmFsaWRfYXJndW1lbnQoImlucHV0LnJlY29yZF9pZCIsICJyZWNvcmQgaWQgaXMgcmVxdWlyZWQiKSk/OwogICAgICAgIE9rKEFnZ3JlZ2F0ZVRhcmdldCB7CiAgICAgICAgICAgIHJlZmVyZW5jZTogcmVjb3JkKHJlY29yZF9pZCksCiAgICAgICAgICAgIHByZXNlbmNlOiBBZ2dyZWdhdGVQcmVzZW5jZTo6TXVzdEJlQWJzZW50LAogICAgICAgIH0pCiAgICB9CgogICAgZm4gcGxhbigKICAgICAgICAmc2VsZiwKICAgICAgICBfZGVmaW5pdGlvbjogJkNhcGFiaWxpdHlEZWZpbml0aW9uLAogICAgICAgIHJlcXVlc3Q6ICZDYXBhYmlsaXR5UmVxdWVzdCwKICAgICAgICBjdXJyZW50OiBPcHRpb248JmNybV9tb2R1bGVfc2RrOjpSZWNvcmRTbmFwc2hvdD4sCiAgICApIC0+IFJlc3VsdDxDYXBhYmlsaXR5QmF0Y2hFeGVjdXRpb25QbGFuLCBTZGtFcnJvcj4gewogICAgICAgIGlmIGN1cnJlbnQuaXNfc29tZSgpIHsKICAgICAgICAgICAgcmV0dXJuIEVycihTZGtFcnJvcjo6bmV3KAogICAgICAgICAgICAgICAgIlRFU1RfQUdHUkVHQVRFX0FMUkVBRFlfRVhJU1RTIiwKICAgICAgICAgICAgICAgIGNybV9tb2R1bGVfc2RrOjpFcnJvckNhdGVnb3J5OjpDb25mbGljdCwKICAgICAgICAgICAgICAgIGZhbHNlLAogICAgICAgICAgICAgICAgIlRoZSB0ZXN0IGFnZ3JlZ2F0ZSBhbHJlYWR5IGV4aXN0cy4iLAogICAgICAgICAgICApKTsKICAgICAgICB9CiAgICAgICAgbGV0IHZhbHVlOiBzZXJkZV9qc29uOjpWYWx1ZSA9IHNlcmRlX2pzb246OmZyb21fc2xpY2UoJnJlcXVlc3QuaW5wdXQuYnl0ZXMpLm1hcF9lcnIofGVycm9yfCB7CiAgICAgICAgICAgIFNka0Vycm9yOjppbnZhbGlkX2FyZ3VtZW50KCJpbnB1dCIsIGZvcm1hdCEoImludmFsaWQgYWdncmVnYXRlIGNvbW1hbmQ6IHtlcnJvcn0iKSkKICAgICAgICB9KT87CiAgICAgICAgbGV0IHJlY29yZF9pZCA9IHZhbHVlCiAgICAgICAgICAgIC5nZXQoInJlY29yZF9pZCIpCiAgICAgICAgICAgIC5hbmRfdGhlbihzZXJkZV9qc29uOjpWYWx1ZTo6YXNfc3RyKQogICAgICAgICAgICAub2tfb3JfZWxzZSh8fCBTZGtFcnJvcjo6aW52YWxpZF9hcmd1bWVudCgiaW5wdXQucmVjb3JkX2lkIiwgInJlY29yZCBpZCBpcyByZXF1aXJlZCIpKT87CiAgICAgICAgbGV0IG5leHRfdmFsdWUgPSB2YWx1ZQogICAgICAgICAgICAuZ2V0KCJ2YWx1ZSIpCiAgICAgICAgICAgIC5hbmRfdGhlbihzZXJkZV9qc29uOjpWYWx1ZTo6YXNfdTY0KQogICAgICAgICAgICAuYW5kX3RoZW4ofHZhbHVlfCB1ODo6dHJ5X2Zyb20odmFsdWUpLm9rKCkpCiAgICAgICAgICAgIC5va19vcl9lbHNlKHx8IFNka0Vycm9yOjppbnZhbGlkX2FyZ3VtZW50KCJpbnB1dC52YWx1ZSIsICJ2YWx1ZSBtdXN0IGZpdCBpbiB1OCIpKT87CiAgICAgICAgbGV0IHJlZmVyZW5jZSA9IHJlY29yZChyZWNvcmRfaWQpOwogICAgICAgIGxldCB0eCA9IHJlcXVlc3QuY29udGV4dC5leGVjdXRpb24uYnVzaW5lc3NfdHJhbnNhY3Rpb25faWQuYXNfc3RyKCk7CiAgICAgICAgbGV0IG91dHB1dCA9IFR5cGVkUGF5bG9hZCB7CiAgICAgICAgICAgIG93bmVyOiBNb2R1bGVJZDo6dHJ5X25ldygiY3JtLnRlc3QiKS51bndyYXAoKSwKICAgICAgICAgICAgc2NoZW1hX2lkOiBTY2hlbWFJZDo6dHJ5X25ldygidGVzdC5hZ2dyZWdhdGUub3V0cHV0IikudW53cmFwKCksCiAgICAgICAgICAgIHNjaGVtYV92ZXJzaW9uOiBTY2hlbWFWZXJzaW9uOjp0cnlfbmV3KCIxLjAuMCIpLnVud3JhcCgpLAogICAgICAgICAgICBkZXNjcmlwdG9yX2hhc2g6IFsweGQxOyAzMl0sCiAgICAgICAgICAgIGRhdGFfY2xhc3M6IERhdGFDbGFzczo6SW50ZXJuYWwsCiAgICAgICAgICAgIGVuY29kaW5nOiBQYXlsb2FkRW5jb2Rpbmc6Okpzb24sCiAgICAgICAgICAgIG1heGltdW1fc2l6ZV9ieXRlczogMTAyNCwKICAgICAgICAgICAgcmV0ZW50aW9uX3BvbGljeV9pZDogUmV0ZW50aW9uUG9saWN5SWQ6OnRyeV9uZXcoInN0YW5kYXJkIikudW53cmFwKCksCiAgICAgICAgICAgIGJ5dGVzOiBzZXJkZV9qc29uOjp0b192ZWMoJnNlcmRlX2pzb246Ompzb24hKHsKICAgICAgICAgICAgICAgICJyZWNvcmRfaWQiOiByZWNvcmRfaWQsCiAgICAgICAgICAgICAgICAidmVyc2lvbiI6IDEsCiAgICAgICAgICAgICAgICAidmFsdWUiOiBuZXh0X3ZhbHVlLAogICAgICAgICAgICB9KSkKICAgICAgICAgICAgLnVud3JhcCgpLAogICAgICAgIH07CiAgICAgICAgT2soQ2FwYWJpbGl0eUJhdGNoRXhlY3V0aW9uUGxhbiB7CiAgICAgICAgICAgIGJhdGNoOiBCYXRjaE11dGF0aW9uUGxhbiB7CiAgICAgICAgICAgICAgICBjb250ZXh0OiByZXF1ZXN0LmNvbnRleHQuY2xvbmUoKSwKICAgICAgICAgICAgICAgIHJlY29yZHM6IHZlYyFbUmVjb3JkTXV0YXRpb246OkNyZWF0ZSB7CiAgICAgICAgICAgICAgICAgICAgcmVmZXJlbmNlOiByZWZlcmVuY2UuY2xvbmUoKSwKICAgICAgICAgICAgICAgICAgICBwYXlsb2FkOiBwYXlsb2FkKG5leHRfdmFsdWUsICJ0ZXN0LmJhdGNoX3JlY29yZC52MSIpLAogICAgICAgICAgICAgICAgfV0sCiAgICAgICAgICAgICAgICByZWxhdGlvbnNoaXBzOiBWZWM6Om5ldygpLAogICAgICAgICAgICAgICAgZXZlbnRzOiB2ZWMhW3JlY29yZF9ldmVudCgKICAgICAgICAgICAgICAgICAgICAmZm9ybWF0ISgiZXZlbnQte3R4fSIpLAogICAgICAgICAgICAgICAgICAgICJ0ZXN0LmJhdGNoX3JlY29yZC5jcmVhdGVkIiwKICAgICAgICAgICAgICAgICAgICByZWZlcmVuY2UsCiAgICAgICAgICAgICAgICAgICAgMSwKICAgICAgICAgICAgICAgICAgICAxLAogICAgICAgICAgICAgICAgICAgIG5leHRfdmFsdWUud3JhcHBpbmdfYWRkKDEpLAogICAgICAgICAgICAgICAgKV0sCiAgICAgICAgICAgICAgICBpZGVtcG90ZW5jeTogSWRlbXBvdGVuY3lFdmlkZW5jZSB7CiAgICAgICAgICAgICAgICAgICAgc2NvcGU6ICJjYXBhYmlsaXR5OnRlc3QucmVjb3JkLm11dGF0ZToxLjAuMCIudG9fb3duZWQoKSwKICAgICAgICAgICAgICAgICAgICBrZXk6IHJlcXVlc3QuY29udGV4dC5leGVjdXRpb24uaWRlbXBvdGVuY3lfa2V5LnRvX3N0cmluZygpLAogICAgICAgICAgICAgICAgICAgIHJlcXVlc3RfaGFzaDogcmVxdWVzdC5pbnB1dF9oYXNoLAogICAgICAgICAgICAgICAgICAgIGV4cGlyZXNfYXRfdW5peF9uYW5vczogMV84MDBfMDAwXzAwMF8wMDBfMDAwXzAwMCwKICAgICAgICAgICAgICAgIH0sCiAgICAgICAgICAgICAgICBhdWRpdHM6IHZlYyFbYXVkaXQoJmZvcm1hdCEoImF1ZGl0LXt0eH0iKSwgNTAxKV0sCiAgICAgICAgICAgIH0sCiAgICAgICAgICAgIG91dHB1dDogU29tZShvdXRwdXQpLAogICAgICAgIH0pCiAgICB9Cn0KCmZuIGNyZWF0aW5nX3JlcXVlc3QoCiAgICB0cmFuc2FjdGlvbl9pZDogJnN0ciwKICAgIGlkZW1wb3RlbmN5X2tleTogJnN0ciwKICAgIHJlY29yZF9pZDogJnN0ciwKICAgIHZhbHVlOiB1OCwKKSAtPiBDYXBhYmlsaXR5UmVxdWVzdCB7CiAgICBsZXQgaW5wdXQgPSBUeXBlZFBheWxvYWQgewogICAgICAgIG93bmVyOiBNb2R1bGVJZDo6dHJ5X25ldygiY3JtLnRlc3QiKS51bndyYXAoKSwKICAgICAgICBzY2hlbWFfaWQ6IFNjaGVtYUlkOjp0cnlfbmV3KCJ0ZXN0LmFnZ3JlZ2F0ZS5jb21tYW5kIikudW53cmFwKCksCiAgICAgICAgc2NoZW1hX3ZlcnNpb246IFNjaGVtYVZlcnNpb246OnRyeV9uZXcoIjEuMC4wIikudW53cmFwKCksCiAgICAgICAgZGVzY3JpcHRvcl9oYXNoOiBbMHhjMTsgMzJdLAogICAgICAgIGRhdGFfY2xhc3M6IERhdGFDbGFzczo6SW50ZXJuYWwsCiAgICAgICAgZW5jb2Rpbmc6IFBheWxvYWRFbmNvZGluZzo6SnNvbiwKICAgICAgICBtYXhpbXVtX3NpemVfYnl0ZXM6IDEwMjQsCiAgICAgICAgcmV0ZW50aW9uX3BvbGljeV9pZDogUmV0ZW50aW9uUG9saWN5SWQ6OnRyeV9uZXcoInN0YW5kYXJkIikudW53cmFwKCksCiAgICAgICAgYnl0ZXM6IHNlcmRlX2pzb246OnRvX3ZlYygmc2VyZGVfanNvbjo6anNvbiEoewogICAgICAgICAgICAicmVjb3JkX2lkIjogcmVjb3JkX2lkLAogICAgICAgICAgICAidmFsdWUiOiB2YWx1ZSwKICAgICAgICB9KSkKICAgICAgICAudW53cmFwKCksCiAgICB9OwogICAgQ2FwYWJpbGl0eVJlcXVlc3QgewogICAgICAgIGNvbnRleHQ6IGNvbnRleHQodHJhbnNhY3Rpb25faWQsIGlkZW1wb3RlbmN5X2tleSksCiAgICAgICAgaW5wdXQsCiAgICAgICAgaW5wdXRfaGFzaDogW3ZhbHVlLm1heCgxKTsgMzJdLAogICAgICAgIGFwcHJvdmFsOiBOb25lLAogICAgfQp9Cg=="


def run_json(command: list[str], payload: dict[str, object]) -> str:
    result = subprocess.run(
        command,
        input=json.dumps(payload).encode(),
        stdout=subprocess.PIPE,
        check=True,
    )
    return result.stdout.decode().strip()


def main() -> None:
    for script_path in PAYLOADS:
        module = ast.parse(Path(script_path).read_text())
        assignment = next(
            node
            for node in module.body
            if isinstance(node, ast.Assign)
            and any(
                isinstance(target, ast.Name) and target.id == "FILES"
                for target in node.targets
            )
        )
        for path, encoded in ast.literal_eval(assignment.value).items():
            if path == CORRUPT_PATH:
                continue
            target = Path(path)
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(zlib.decompress(base64.b64decode(encoded)))

    support = Path(CORRUPT_PATH)
    support.write_text(
        support.read_text().rstrip() + base64.b64decode(APPENDIX_B64).decode()
    )
    subprocess.run(["cargo", "generate-lockfile"], check=True)
    subprocess.run(["cargo", "fmt", "--all"], check=True)

    base_sha = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
    base_tree = subprocess.check_output(
        ["gh", "api", f"repos/{REPO}/git/commits/{base_sha}", "--jq", ".tree.sha"],
        text=True,
    ).strip()
    status = subprocess.check_output(
        ["git", "status", "--porcelain=v1", "-z"]
    ).decode().split("\0")
    entries: list[dict[str, str]] = []
    for item in status:
        if not item:
            continue
        path = item[3:]
        if path == ".github/workflows/phase6-apply-capability-adapters.yml":
            continue
        if path == "scripts/phase6_recovery_publish.py":
            continue
        if path.startswith("scripts/phase6_payload_"):
            continue
        blob = run_json(
            ["gh", "api", f"repos/{REPO}/git/blobs", "--input", "-", "--jq", ".sha"],
            {"content": Path(path).read_text(), "encoding": "utf-8"},
        )
        entries.append({"path": path, "mode": "100644", "type": "blob", "sha": blob})

    tree_sha = run_json(
        ["gh", "api", f"repos/{REPO}/git/trees", "--input", "-", "--jq", ".sha"],
        {"base_tree": base_tree, "tree": entries},
    )
    commit_sha = run_json(
        ["gh", "api", f"repos/{REPO}/git/commits", "--input", "-", "--jq", ".sha"],
        {
            "message": "feat: materialize recovered Phase 6F capability implementation",
            "tree": tree_sha,
            "parents": [base_sha],
        },
    )
    body = (
        "Phase 6F recovery object built.\n\n"
        f"MATERIALIZED_FILES={len(entries)}\n"
        f"MATERIALIZED_COMMIT_SHA={commit_sha}"
    )
    subprocess.run(
        ["gh", "pr", "comment", PR_NUMBER, "--repo", REPO, "--body", body],
        check=True,
    )
    print(body)


if __name__ == "__main__":
    main()

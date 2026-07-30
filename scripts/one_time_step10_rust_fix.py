from __future__ import annotations

from pathlib import Path


def replace_once(path: str, old: str, new: str, label: str) -> None:
    target = Path(path)
    text = target.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: found {count}, expected 1")
    target.write_text(text.replace(old, new), encoding="utf-8")


replace_once(
    "crates/crm-application-runtime/tests/customer_privacy_access_export_postgres.rs",
    """            if command.chunk_index != artifact.metadata.next_chunk_index
                || Sha256::digest(&command.bytes).as_slice() != command.chunk_sha256
            {
""",
    """            let chunk_sha256: [u8; 32] = Sha256::digest(&command.bytes).into();
            if command.chunk_index != artifact.metadata.next_chunk_index
                || chunk_sha256 != command.chunk_sha256
            {
""",
    "test chunk digest comparison",
)
replace_once(
    "crates/crm-application-runtime/tests/customer_privacy_access_export_postgres.rs",
    """            if artifact.bytes.len() as u64 != artifact.metadata.expected_size_bytes
                || Sha256::digest(&artifact.bytes).as_slice() != artifact.metadata.expected_sha256
            {
""",
    """            let artifact_sha256: [u8; 32] = Sha256::digest(&artifact.bytes).into();
            if artifact.bytes.len() as u64 != artifact.metadata.expected_size_bytes
                || artifact_sha256 != artifact.metadata.expected_sha256
            {
""",
    "test artifact digest comparison",
)

from pathlib import Path

SOURCE = Path('.github/workflows/phase8a10-stale-evidence-guards.yml')
lines = SOURCE.read_text().splitlines()
start = next(
    index for index, line in enumerate(lines) if line.strip() == "python - <<'PY'"
) + 1
end = next(
    index for index in range(start, len(lines)) if lines[index].strip() == 'PY'
)
payload_lines = []
for line in lines[start:end]:
    payload_lines.append(line[10:] if line.startswith('          ') else line)
payload = '\n'.join(payload_lines) + '\n'

placeholder = "AND attempt.record_type = $8"
unused_bind = "                .bind(coordinate.lock_key)\n                .bind(APPLICATION_ATTEMPT_RECORD_TYPE)"
if payload.count(placeholder) != 1 or payload.count(unused_bind) != 1:
    raise SystemExit('prepared materialization SQL binding anchors are not exact')
payload = payload.replace(placeholder, "AND attempt.record_type = $7", 1)
payload = payload.replace(
    unused_bind,
    "                .bind(APPLICATION_ATTEMPT_RECORD_TYPE)",
    1,
)

compile(payload, str(SOURCE), 'exec')
exec(payload, {'__name__': '__main__'})

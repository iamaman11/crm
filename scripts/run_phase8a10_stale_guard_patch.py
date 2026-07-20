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
compile(payload, str(SOURCE), 'exec')
exec(payload, {'__name__': '__main__'})

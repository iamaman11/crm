from pathlib import Path

path = Path(".github/workflows/database.yml")
text = path.read_text()

query_path = '      - "crates/crm-sales-activities-query-adapter/**"\n'
if text.count(query_path) != 2:
    raise RuntimeError(f"database path anchors: expected 2, found {text.count(query_path)}")
text = text.replace(
    query_path,
    query_path
    + '      - "crates/crm-search-runtime/**"\n'
    + '      - "crates/crm-search-query-adapter/**"\n',
)

up = "          psql --set ON_ERROR_STOP=1 --file database/migrations/0008_rebuildable_projection_runtime.up.sql\n"
if text.count(up) != 3:
    raise RuntimeError(f"0008 up anchors: expected 3, found {text.count(up)}")
text = text.replace(
    up,
    up + "          psql --set ON_ERROR_STOP=1 --file database/migrations/0009_search_index_generations.up.sql\n",
)

down = "          psql --set ON_ERROR_STOP=1 --file database/migrations/0008_rebuildable_projection_runtime.down.sql\n"
if text.count(down) != 2:
    raise RuntimeError(f"0008 down anchors: expected 2, found {text.count(down)}")
text = text.replace(
    down,
    "          psql --set ON_ERROR_STOP=1 --file database/migrations/0009_search_index_generations.down.sql\n" + down,
)

diag = "            crates/crm-sales-activities-query-adapter/\n"
if text.count(diag) != 1:
    raise RuntimeError(f"diagnostic anchor: expected 1, found {text.count(diag)}")
text = text.replace(
    diag,
    diag
    + "            crates/crm-search-runtime/\n"
    + "            crates/crm-search-query-adapter/\n",
)

path.write_text(text)

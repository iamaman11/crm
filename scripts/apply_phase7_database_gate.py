from pathlib import Path

path = Path("crates/crm-search-runtime/src/lib.rs")
text = path.read_text()
redundant = """                let next_after = has_more && !filtered.is_empty().then(|| ());
                let next_after = if has_more {
                    filtered.last().map(SearchCandidate::cursor)
                } else {
                    None
                };
                let _ = next_after;
"""
count = text.count(redundant)
if count != 1:
    raise RuntimeError(f"search pagination compile-fix anchor: expected 1, found {count}")
path.write_text(text.replace(redundant, "", 1))

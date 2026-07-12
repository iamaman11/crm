from pathlib import Path


def one(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected 1 match, found {count}")
    return text.replace(old, new, 1)


path = Path("crates/crm-core-data/src/search_store.rs")
text = path.read_text()
text = one(
    text,
    "use std::collections::BTreeMap;",
    "use std::collections::{BTreeMap, BTreeSet};",
    "set import",
)
text = one(
    text,
    """                source_version,
                document,
                GREATEST(
""",
    """                source_version,
                document,
                ARRAY(
                  SELECT search_field.field_name
                  FROM jsonb_each_text(document -> 'searchable_fields')
                    AS search_field(field_name, field_value)
                  WHERE to_tsvector('simple', search_field.field_value)
                    @@ websearch_to_tsquery('simple', $3)
                  ORDER BY search_field.field_name
                ) AS matched_fields,
                GREATEST(
""",
    "matched field projection",
)
text = one(
    text,
    """                AND document ? 'search_text'
                AND (cardinality($4::text[]) = 0 OR resource_type = ANY($4::text[]))
                AND to_tsvector('simple', COALESCE(document ->> 'search_text', ''))
                    @@ websearch_to_tsquery('simple', $3)
            )
            SELECT resource_type, resource_id, source_version, document, rank_micros
""",
    """                AND document ? 'search_text'
                AND document ? 'searchable_fields'
                AND (cardinality($4::text[]) = 0 OR resource_type = ANY($4::text[]))
                AND to_tsvector('simple', COALESCE(document ->> 'search_text', ''))
                    @@ websearch_to_tsquery('simple', $3)
                AND EXISTS (
                  SELECT 1
                  FROM jsonb_each_text(document -> 'searchable_fields')
                    AS search_field(field_name, field_value)
                  WHERE to_tsvector('simple', search_field.field_value)
                    @@ websearch_to_tsquery('simple', $3)
                )
            )
            SELECT resource_type, resource_id, source_version, document, matched_fields, rank_micros
""",
    "field-local candidate semantics",
)
text = one(
    text,
    """    let rank_micros: i64 = row
        .try_get("rank_micros")
        .map_err(|error| search_stored_value_invalid(error.to_string()))?;
    let document: Value = row
""",
    """    let rank_micros: i64 = row
        .try_get("rank_micros")
        .map_err(|error| search_stored_value_invalid(error.to_string()))?;
    let matched_fields: Vec<String> = row
        .try_get("matched_fields")
        .map_err(|error| search_stored_value_invalid(error.to_string()))?;
    let document: Value = row
""",
    "decode match evidence",
)
text = one(
    text,
    """        rank_micros,
        searchable_fields: string_map_field(object, "searchable_fields")?,
        display_fields: string_map_field(object, "display_fields")?,
""",
    """        rank_micros,
        searchable_fields: string_map_field(object, "searchable_fields")?,
        matched_fields: matched_fields.into_iter().collect::<BTreeSet<_>>(),
        display_fields: string_map_field(object, "display_fields")?,
""",
    "candidate match evidence",
)
path.write_text(text)

path = Path("crates/crm-core-data/tests/postgres_search.rs")
text = path.read_text()
text = one(
    text,
    """    assert_eq!(first.candidates.len(), 1);
    let first_id = first.candidates[0].resource.record_id.as_str().to_owned();
""",
    """    assert_eq!(first.candidates.len(), 1);
    assert_eq!(
        first.candidates[0].matched_fields,
        BTreeSet::from(["name".to_owned()])
    );
    let first_id = first.candidates[0].resource.record_id.as_str().to_owned();
""",
    "PostgreSQL match evidence acceptance",
)
path.write_text(text)

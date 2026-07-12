from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected 1 match, found {count}")
    return text.replace(old, new, 1)


runtime_path = Path("crates/crm-search-runtime/src/lib.rs")
runtime = runtime_path.read_text()
runtime = replace_once(
    runtime,
    """pub struct SearchCandidate {
    pub owner_module_id: ModuleId,
    pub resource: RecordRef,
    pub source_version: i64,
    pub rank_micros: i64,
    pub searchable_fields: BTreeMap<String, String>,
    pub display_fields: BTreeMap<String, String>,
}""",
    """pub struct SearchCandidate {
    pub owner_module_id: ModuleId,
    pub resource: RecordRef,
    pub source_version: i64,
    pub rank_micros: i64,
    pub searchable_fields: BTreeMap<String, String>,
    pub matched_fields: BTreeSet<String>,
    pub display_fields: BTreeMap<String, String>,
}""",
    "candidate backend match evidence",
)
runtime = replace_once(
    runtime,
    """        validate_fields(&self.searchable_fields)?;
        validate_fields(&self.display_fields)?;
        Ok(())
""",
    """        validate_fields(&self.searchable_fields)?;
        if self.matched_fields.is_empty()
            || self
                .matched_fields
                .iter()
                .any(|field| !self.searchable_fields.contains_key(field))
        {
            return Err(search_internal(
                "SEARCH_CANDIDATE_MATCH_EVIDENCE_INVALID",
                "The search service returned invalid match evidence.",
            ));
        }
        validate_fields(&self.display_fields)?;
        Ok(())
""",
    "candidate match evidence validation",
)
runtime = replace_once(
    runtime,
    """                let searchable_fields = candidate
                    .searchable_fields
                    .iter()
                    .filter(|(field, _)| decision.allows_field(field))
                    .map(|(field, value)| (field.clone(), value.clone()))
                    .collect::<BTreeMap<_, _>>();
                let matched_fields = searchable_fields
                    .iter()
                    .filter(|(_, value)| normalized_contains(value, &normalized_text))
                    .map(|(field, _)| field.clone())
                    .collect::<BTreeSet<_>>();
""",
    """                let matched_fields = candidate
                    .matched_fields
                    .iter()
                    .filter(|field| decision.allows_field(field))
                    .cloned()
                    .collect::<BTreeSet<_>>();
""",
    "live visibility intersection with backend match evidence",
)
runtime = replace_once(
    runtime,
    """fn normalized_contains(value: &str, normalized_text: &str) -> bool {
    value.to_lowercase().contains(normalized_text)
}

""",
    "",
    "remove duplicate runtime matching semantics",
)
runtime = replace_once(
    runtime,
    """                        candidate
                            .searchable_fields
                            .values()
                            .any(|value| normalized_contains(value, &request.normalized_text))
""",
    """                        candidate
                            .searchable_fields
                            .values()
                            .any(|value| value.to_lowercase().contains(&request.normalized_text))
""",
    "test store local matching semantics",
)
runtime = replace_once(
    runtime,
    """            searchable_fields: BTreeMap::from([("name".to_owned(), name.to_owned())]),
            display_fields: BTreeMap::from([("amount".to_owned(), "1000".to_owned())]),
""",
    """            searchable_fields: BTreeMap::from([("name".to_owned(), name.to_owned())]),
            matched_fields: BTreeSet::from(["name".to_owned()]),
            display_fields: BTreeMap::from([("amount".to_owned(), "1000".to_owned())]),
""",
    "test candidate match evidence",
)
runtime_path.write_text(runtime)

store_path = Path("crates/crm-core-data/src/search_store.rs")
store = store_path.read_text()
store = replace_once(
    store,
    "use std::collections::BTreeMap;",
    "use std::collections::{BTreeMap, BTreeSet};",
    "search store set import",
)
old_sql = """            WITH ranked AS (
              SELECT
                resource_type,
                resource_id,
                source_version,
                document,
                GREATEST(
                  1::bigint,
                  ROUND(
                    ts_rank_cd(
                      to_tsvector('simple', COALESCE(document ->> 'search_text', '')),
                      websearch_to_tsquery('simple', $3)
                    ) * 1000000.0
                  )::bigint
                ) AS rank_micros
              FROM crm.projection_documents
              WHERE tenant_id = $1
                AND projection_id = $2
                AND document ? 'search_text'
                AND (cardinality($4::text[]) = 0 OR resource_type = ANY($4::text[]))
                AND to_tsvector('simple', COALESCE(document ->> 'search_text', ''))
                    @@ websearch_to_tsquery('simple', $3)
            )
            SELECT resource_type, resource_id, source_version, document, rank_micros
            FROM ranked
"""
new_sql = """            WITH matched AS (
              SELECT
                resource_type,
                resource_id,
                source_version,
                document,
                ARRAY(
                  SELECT search_field.field_name
                  FROM jsonb_each_text(document -> 'searchable_fields')
                    AS search_field(field_name, field_value)
                  WHERE to_tsvector('simple', search_field.field_value)
                    @@ websearch_to_tsquery('simple', $3)
                  ORDER BY search_field.field_name
                ) AS matched_fields
              FROM crm.projection_documents
              WHERE tenant_id = $1
                AND projection_id = $2
                AND document ? 'search_text'
                AND document ? 'searchable_fields'
                AND (cardinality($4::text[]) = 0 OR resource_type = ANY($4::text[]))
                AND to_tsvector('simple', COALESCE(document ->> 'search_text', ''))
                    @@ websearch_to_tsquery('simple', $3)
            ),
            ranked AS (
              SELECT
                resource_type,
                resource_id,
                source_version,
                document,
                matched_fields,
                GREATEST(
                  1::bigint,
                  ROUND(
                    ts_rank_cd(
                      to_tsvector('simple', COALESCE(document ->> 'search_text', '')),
                      websearch_to_tsquery('simple', $3)
                    ) * 1000000.0
                  )::bigint
                ) AS rank_micros
              FROM matched
              WHERE cardinality(matched_fields) > 0
            )
            SELECT resource_type, resource_id, source_version, document, matched_fields, rank_micros
            FROM ranked
"""
store = replace_once(store, old_sql, new_sql, "field-local PostgreSQL match evidence")
store = replace_once(
    store,
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
    "decode PostgreSQL match evidence",
)
store = replace_once(
    store,
    """        source_version,
        rank_micros,
        searchable_fields: string_map_field(object, "searchable_fields")?,
        display_fields: string_map_field(object, "display_fields")?,
""",
    """        source_version,
        rank_micros,
        searchable_fields: string_map_field(object, "searchable_fields")?,
        matched_fields: matched_fields.into_iter().collect::<BTreeSet<_>>(),
        display_fields: string_map_field(object, "display_fields")?,
""",
    "store candidate match evidence",
)
store_path.write_text(store)

revocation_path = Path("crates/crm-search-query-adapter/tests/live_permission_revocation.rs")
revocation = revocation_path.read_text()
revocation = replace_once(
    revocation,
    """                    searchable_fields: BTreeMap::from([(
                        "name".to_owned(),
                        "Acme Enterprise".to_owned(),
                    )]),
                    display_fields: BTreeMap::from([(
""",
    """                    searchable_fields: BTreeMap::from([(
                        "name".to_owned(),
                        "Acme Enterprise".to_owned(),
                    )]),
                    matched_fields: BTreeSet::from(["name".to_owned()]),
                    display_fields: BTreeMap::from([(
""",
    "revocation test backend match evidence",
)
revocation_path.write_text(revocation)

postgres_test_path = Path("crates/crm-core-data/tests/postgres_search.rs")
postgres_test = postgres_test_path.read_text()
postgres_test = replace_once(
    postgres_test,
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
postgres_test_path.write_text(postgres_test)

readme_path = Path("crates/crm-search-runtime/README.md")
readme = readme_path.read_text()
anchor = "A candidate is suppressed when the current actor cannot see the resource or when the query matched only fields that are currently hidden.\n"
insert = anchor + "\nThe candidate store also returns the exact fields that matched according to the backend query semantics. The permission-aware runtime does not re-interpret the query with a second matching algorithm; it intersects backend match evidence with live field visibility before disclosure. Backends must therefore apply the full query within at least one searchable field rather than combining hidden and visible fields to manufacture a cross-field match.\n"
readme = replace_once(readme, anchor, insert, "search match semantics documentation")
readme_path.write_text(readme)

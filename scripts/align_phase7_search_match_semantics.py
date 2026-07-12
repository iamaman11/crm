from pathlib import Path


def one(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected 1 match, found {count}")
    return text.replace(old, new, 1)


path = Path("crates/crm-search-runtime/src/lib.rs")
text = path.read_text()
text = one(
    text,
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
    "candidate match evidence",
)
text = one(
    text,
    """    fn validate(&self) -> Result<(), SdkError> {
        if self.source_version <= 0 || self.rank_micros <= 0 {
            return Err(search_internal(
                "SEARCH_CANDIDATE_INVALID",
                "The search service returned an invalid candidate.",
            ));
        }
        validate_fields(&self.searchable_fields)?;
        validate_fields(&self.display_fields)?;
        Ok(())
    }
""",
    """    fn validate(&self) -> Result<(), SdkError> {
        if self.source_version <= 0 || self.rank_micros <= 0 {
            return Err(search_internal(
                "SEARCH_CANDIDATE_INVALID",
                "The search service returned an invalid candidate.",
            ));
        }
        validate_fields(&self.searchable_fields)?;
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
    }
""",
    "candidate validation",
)
text = one(
    text,
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
    "visibility intersection",
)
text = one(
    text,
    """fn normalized_contains(value: &str, normalized_text: &str) -> bool {
    value.to_lowercase().contains(normalized_text)
}

""",
    "",
    "duplicate matcher removal",
)
text = one(
    text,
    ".any(|value| normalized_contains(value, &request.normalized_text))",
    ".any(|value| value.to_lowercase().contains(&request.normalized_text))",
    "test-store matcher",
)
text = one(
    text,
    """            searchable_fields: BTreeMap::from([("name".to_owned(), name.to_owned())]),
            display_fields: BTreeMap::from([("amount".to_owned(), "1000".to_owned())]),
""",
    """            searchable_fields: BTreeMap::from([("name".to_owned(), name.to_owned())]),
            matched_fields: BTreeSet::from(["name".to_owned()]),
            display_fields: BTreeMap::from([("amount".to_owned(), "1000".to_owned())]),
""",
    "test candidate evidence",
)
path.write_text(text)

path = Path("crates/crm-search-query-adapter/tests/live_permission_revocation.rs")
text = path.read_text()
text = one(
    text,
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
    "revocation evidence",
)
path.write_text(text)

path = Path("crates/crm-search-runtime/README.md")
text = path.read_text()
anchor = "A candidate is suppressed when the current actor cannot see the resource or when the query matched only fields that are currently hidden.\n"
addition = anchor + "\nThe candidate store returns the exact fields that matched according to backend query semantics. The permission-aware runtime does not re-interpret the query with a second matcher; it intersects backend match evidence with live field visibility before disclosure.\n"
path.write_text(one(text, anchor, addition, "README semantics"))

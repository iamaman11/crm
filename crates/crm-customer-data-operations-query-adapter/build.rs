use std::env;
use std::fs;
use std::path::PathBuf;

fn replace_once(source: &mut String, old: &str, new: &str, label: &str) {
    assert!(source.contains(old), "{label} patch anchor is missing");
    *source = source.replacen(old, new, 1);
}

fn main() {
    if env::var("GITHUB_WORKFLOW").as_deref() != Ok("Rust Generated Sync") {
        return;
    }
    let manifest_dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be configured"),
    );
    let path = manifest_dir.join("src/lib.rs");
    let mut source = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));

    replace_once(
        &mut source,
        "#![forbid(unsafe_code)]\n\n",
        "#![forbid(unsafe_code)]\n\nmod export_query;\npub use export_query::{\n    EXPORT_QUERY_CAPABILITY_IDS, GET_EXPORT_JOB_CAPABILITY, LIST_EXPORT_JOBS_CAPABILITY,\n    PartyExportQueryAdapter, export_query_capability_definitions,\n};\n\n",
        "export query module",
    );

    replace_once(
        &mut source,
        "pub const QUERY_CAPABILITY_IDS: [&str; 3] = [\n    GET_IMPORT_JOB_CAPABILITY,\n    LIST_IMPORT_JOBS_CAPABILITY,\n    LIST_IMPORT_ROWS_CAPABILITY,\n];",
        "pub const IMPORT_QUERY_CAPABILITY_IDS: [&str; 3] = [\n    GET_IMPORT_JOB_CAPABILITY,\n    LIST_IMPORT_JOBS_CAPABILITY,\n    LIST_IMPORT_ROWS_CAPABILITY,\n];\n\npub const QUERY_CAPABILITY_IDS: [&str; 5] = [\n    GET_IMPORT_JOB_CAPABILITY,\n    LIST_IMPORT_JOBS_CAPABILITY,\n    LIST_IMPORT_ROWS_CAPABILITY,\n    GET_EXPORT_JOB_CAPABILITY,\n    LIST_EXPORT_JOBS_CAPABILITY,\n];",
        "combined query IDs",
    );

    replace_once(
        &mut source,
        "    page_policy: PageSizePolicy,\n}",
        "    page_policy: PageSizePolicy,\n    export: PartyExportQueryAdapter,\n}",
        "export query adapter field",
    );

    replace_once(
        &mut source,
        "        Ok(Self {\n            store,\n            cursor_codec,\n            visibility,\n            page_policy,\n        })",
        "        let export = PartyExportQueryAdapter::new(\n            store.clone(),\n            cursor_codec.clone(),\n            visibility.clone(),\n        )?;\n        Ok(Self {\n            store,\n            cursor_codec,\n            visibility,\n            page_policy,\n            export,\n        })",
        "export query adapter construction",
    );

    replace_once(
        &mut source,
        "pub fn query_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {\n    QUERY_CAPABILITY_IDS\n        .iter()\n        .map(|capability_id| query_capability_definition(capability_id))\n        .collect()\n}",
        "pub fn query_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {\n    let mut definitions = IMPORT_QUERY_CAPABILITY_IDS\n        .iter()\n        .map(|capability_id| query_capability_definition(capability_id))\n        .collect::<Result<Vec<_>, _>>()?;\n    definitions.extend(export_query_capability_definitions()?);\n    Ok(definitions)\n}",
        "combined query definitions",
    );

    replace_once(
        &mut source,
        "                LIST_IMPORT_ROWS_CAPABILITY => {\n                    let command: wire::ListPartyImportRowsRequest =\n                        decode_input(request, LIST_IMPORT_ROWS_REQUEST_SCHEMA)?;\n                    let job_id = import_job_record_id(command.import_job_ref)?;\n                    validate_row_status_filter(command.status)?;\n                    let page_size = self\n                        .page_policy\n                        .resolve(command.page_size)\n                        .map_err(cursor_error)?;\n                    let binding = rows_cursor_binding(\n                        request,\n                        row_filter_hash(job_id.as_str(), command.status),\n                        page_size,\n                    )?;\n                    let _ = decode_row_after(self, &command.cursor, &binding)?;\n                }\n                _ => return Err(unsupported_query()),",
        "                LIST_IMPORT_ROWS_CAPABILITY => {\n                    let command: wire::ListPartyImportRowsRequest =\n                        decode_input(request, LIST_IMPORT_ROWS_REQUEST_SCHEMA)?;\n                    let job_id = import_job_record_id(command.import_job_ref)?;\n                    validate_row_status_filter(command.status)?;\n                    let page_size = self\n                        .page_policy\n                        .resolve(command.page_size)\n                        .map_err(cursor_error)?;\n                    let binding = rows_cursor_binding(\n                        request,\n                        row_filter_hash(job_id.as_str(), command.status),\n                        page_size,\n                    )?;\n                    let _ = decode_row_after(self, &command.cursor, &binding)?;\n                }\n                GET_EXPORT_JOB_CAPABILITY | LIST_EXPORT_JOBS_CAPABILITY => {\n                    self.export\n                        .validate_request(definition.capability_id.as_str(), request)?;\n                }\n                _ => return Err(unsupported_query()),",
        "export query validation routing",
    );

    replace_once(
        &mut source,
        "                GET_IMPORT_JOB_CAPABILITY => self.execute_get_job(&request).await?,\n                LIST_IMPORT_JOBS_CAPABILITY => self.execute_list_jobs(&request).await?,\n                LIST_IMPORT_ROWS_CAPABILITY => self.execute_list_rows(&request).await?,\n                _ => return Err(unsupported_query()),",
        "                GET_IMPORT_JOB_CAPABILITY => self.execute_get_job(&request).await?,\n                LIST_IMPORT_JOBS_CAPABILITY => self.execute_list_jobs(&request).await?,\n                LIST_IMPORT_ROWS_CAPABILITY => self.execute_list_rows(&request).await?,\n                GET_EXPORT_JOB_CAPABILITY | LIST_EXPORT_JOBS_CAPABILITY => {\n                    self.export\n                        .execute_request(definition.capability_id.as_str(), &request)\n                        .await?\n                }\n                _ => return Err(unsupported_query()),",
        "export query execution routing",
    );

    source = source.replace(
        "fn publishes_three_personal_read_only_queries()",
        "fn publishes_five_personal_read_only_queries()",
    );
    source = source.replace("assert_eq!(definitions.len(), 3);", "assert_eq!(definitions.len(), 5);");

    assert!(
        source.contains("pub const QUERY_CAPABILITY_IDS: [&str; 5]")
            && source.contains("self.export")
            && source.contains("publishes_five_personal_read_only_queries"),
        "combined export query routing patch was not applied"
    );
    fs::write(&path, source)
        .unwrap_or_else(|error| panic!("cannot write {}: {error}", path.display()));
    fs::remove_file(manifest_dir.join("build.rs"))
        .expect("temporary export query routing patch must be removable");
}

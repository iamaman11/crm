from pathlib import Path

path = Path("services/crm-api/tests/process_e2e.rs")
text = path.read_text()

old_import = "use crm_proto_contracts::crm::{core::v1 as core, sales::v1 as sales};\n"
new_import = "use crm_proto_contracts::crm::{core::v1 as core, sales::v1 as sales, search::v1 as search};\n"
if text.count(old_import) != 1:
    raise RuntimeError(f"search proto import anchor: expected 1, found {text.count(old_import)}")
text = text.replace(old_import, new_import, 1)

old_const = 'const SALES_GET: &str = "sales.deal.get";\n'
new_const = old_const + 'const SEARCH_GLOBAL: &str = "search.global.query";\n'
if text.count(old_const) != 1:
    raise RuntimeError(f"search capability constant anchor: expected 1, found {text.count(old_const)}")
text = text.replace(old_const, new_const, 1)

old_tail = '''    assert_eq!(
        deal.stage_details.expect("Deal stage details").stage_id,
        "proposal"
    );

    send_sigint(&child).await;
'''
new_tail = '''    assert_eq!(
        deal.stage_details.expect("Deal stage details").stage_id,
        "proposal"
    );

    let search_definition = query_definition(SEARCH_GLOBAL);
    let search_payload = wire_payload(payload(
        &search_definition,
        search::SearchRequest {
            text: "Phase 6L process deal".to_owned(),
            resource_types: vec!["sales.deal".to_owned()],
            page_size: 25,
            cursor: String::new(),
        },
    ));
    let search_deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let mut request = Request::new(GatewayQueryRequest {
            owner_module_id: search_definition.owner_module_id.as_str().to_owned(),
            capability_id: search_definition.capability_id.as_str().to_owned(),
            capability_version: search_definition.capability_version.as_str().to_owned(),
            input: Some(search_payload.clone()),
        });
        request.metadata_mut().insert(
            "authorization",
            format!("Bearer {TOKEN}")
                .parse()
                .expect("valid search authorization metadata"),
        );
        request.metadata_mut().insert(
            "x-tenant-id",
            TENANT.parse().expect("valid search tenant metadata"),
        );
        let response = grpc
            .query(request)
            .await
            .expect("query governed search through production gRPC gateway")
            .into_inner();
        let output = response.output.expect("gRPC search output payload");
        let page = search::SearchResponse::decode(output.payload.as_slice())
            .expect("decode production search response");
        if let Some(hit) = page.hits.iter().find(|hit| hit.resource_id == DEAL_ID) {
            assert_eq!(hit.owner_module_id, "crm.sales");
            assert_eq!(hit.resource_type, "sales.deal");
            assert_eq!(hit.fields.len(), 1);
            assert_eq!(
                hit.fields.get("name").map(String::as_str),
                Some("Phase 6L process deal")
            );
            assert_eq!(hit.matched_fields, vec!["name"]);
            break;
        }
        assert!(
            Instant::now() < search_deadline,
            "production search did not expose the indexed Deal before the acceptance deadline"
        );
        sleep(Duration::from_millis(250)).await;
    }

    send_sigint(&child).await;
'''
if text.count(old_tail) != 1:
    raise RuntimeError(f"process search acceptance anchor: expected 1, found {text.count(old_tail)}")
path.write_text(text.replace(old_tail, new_tail, 1))

use crm_proto_contracts::crm::search::v1::{SearchHit, SearchResponse};
use prost::Message;
use std::collections::HashMap;

#[test]
fn public_search_response_round_trips_only_client_safe_match_data() {
    let response = SearchResponse {
        hits: vec![SearchHit {
            owner_module_id: "crm.sales".to_owned(),
            resource_type: "sales.deal".to_owned(),
            resource_id: "deal-1".to_owned(),
            source_version: 7,
            rank_micros: 900_000,
            fields: HashMap::from([("name".to_owned(), "Acme Renewal".to_owned())]),
            matched_fields: vec!["name".to_owned()],
        }],
        next_cursor: String::new(),
    };

    let decoded = SearchResponse::decode(response.encode_to_vec().as_slice()).unwrap();
    assert_eq!(decoded, response);
    assert_eq!(decoded.hits[0].fields.len(), 1);
    assert_eq!(decoded.hits[0].matched_fields, vec!["name"]);
}

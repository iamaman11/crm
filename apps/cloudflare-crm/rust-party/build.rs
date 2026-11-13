fn main() {
    // A single source list drives code generation; Cargo tracks all imports below.
    // Buf's canonical Protobuf remains the source of truth; no second registry.
    const CONNECT_PROTOS: &[&str] = &[
        "../../../proto/crm/core/v2/operation.proto",
        "../../../proto/crm/parties/v3/party.proto",
        "../../../proto/crm/accounts/v2/account.proto",
        "../../../proto/crm/contact_points/v2/contact_point.proto",
        "../../../proto/crm/party_relationships/v2/relationship.proto",
        "../../../proto/crm/activities/v2/task.proto",
        "../../../proto/crm/data_quality/v2/quality.proto",
        "../../../proto/crm/identity_resolution/v2/identity.proto",
        "../../../proto/crm/consents/v2/consent.proto",
        "../../../proto/crm/sales/v2/deal.proto",
        "../../../proto/crm/customer_privacy/v2/case.proto",
        "../../../proto/crm/customer_enrichment/v2/config.proto",
        "../../../proto/crm/customer_data_operations/v2/party_csv.proto",
        "../../../proto/crm/customer_360/v2/customer_360.proto",
    ];
    let protoc = protoc_bin_vendored::protoc_bin_path().expect("vendored protoc");
    unsafe {
        std::env::set_var("PROTOC", protoc);
    }
    // Track imported schemas too when reusing Cargo outputs across commits.
    println!("cargo:rerun-if-changed=../../../proto");
    connectrpc_build::Config::new()
        .files(CONNECT_PROTOS)
        .includes(&["../../../proto"])
        .include_file("_connectrpc.rs")
        .compile()
        .expect("CRM Connect code generation");
}

fn main() {
    // Existing canonical proto is the source of truth; regenerate service types
    // during the Rust build, no checked-in duplicate schema.
    let protoc = protoc_bin_vendored::protoc_bin_path().expect("vendored protoc");
    unsafe { std::env::set_var("PROTOC", protoc); }
    println!("cargo:rerun-if-changed=../../../proto/crm/sales/v1/deal.proto");
    println!("cargo:rerun-if-changed=../../../proto/crm/core/v1/common.proto");
    println!("cargo:rerun-if-changed=../../../proto/crm/parties/v2/party.proto");
    println!("cargo:rerun-if-changed=../../../proto/crm/core/v2/operation.proto");
    println!("cargo:rerun-if-changed=../../../proto/crm/parties/v3/party.proto");
    println!("cargo:rerun-if-changed=../../../proto/crm/accounts/v2/account.proto");
    println!("cargo:rerun-if-changed=../../../proto/crm/contact_points/v2/contact_point.proto");
    println!("cargo:rerun-if-changed=../../../proto/crm/party_relationships/v2/relationship.proto");
    println!("cargo:rerun-if-changed=../../../proto/crm/activities/v2/task.proto");
    println!("cargo:rerun-if-changed=../../../proto/crm/data_quality/v2/quality.proto");
    println!("cargo:rerun-if-changed=../../../proto/crm/identity_resolution/v2/identity.proto");
    println!("cargo:rerun-if-changed=../../../proto/crm/consents/v2/consent.proto");
    println!("cargo:rerun-if-changed=../../../proto/crm/sales/v2/deal.proto");
    connectrpc_build::Config::new()
        .files(&["../../../proto/crm/core/v1/common.proto",
                 "../../../proto/crm/sales/v1/deal.proto",
                 "../../../proto/crm/parties/v2/party.proto",
                 "../../../proto/crm/core/v2/operation.proto",
                 "../../../proto/crm/parties/v3/party.proto",
                 "../../../proto/crm/accounts/v2/account.proto",
                 "../../../proto/crm/contact_points/v2/contact_point.proto",
                 "../../../proto/crm/party_relationships/v2/relationship.proto",
                 "../../../proto/crm/activities/v2/task.proto",
                 "../../../proto/crm/data_quality/v2/quality.proto",
                 "../../../proto/crm/identity_resolution/v2/identity.proto",
                 "../../../proto/crm/consents/v2/consent.proto",
                 "../../../proto/crm/sales/v2/deal.proto"])
        .includes(&["../../../proto"])
        .include_file("_connectrpc.rs")
        .compile()
        .expect("published DealService Connect code generation");
}

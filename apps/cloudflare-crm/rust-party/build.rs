fn main() {
    // Existing canonical proto is the source of truth; regenerate service types
    // during the Rust build, no checked-in duplicate schema.
    let protoc = protoc_bin_vendored::protoc_bin_path().expect("vendored protoc");
    unsafe { std::env::set_var("PROTOC", protoc); }
    println!("cargo:rerun-if-changed=../../../proto/crm/sales/v1/deal.proto");
    println!("cargo:rerun-if-changed=../../../proto/crm/core/v1/common.proto");
    connectrpc_build::Config::new()
        .files(&["../../../proto/crm/core/v1/common.proto",
                 "../../../proto/crm/sales/v1/deal.proto"])
        .includes(&["../../../proto"])
        .include_file("_connectrpc.rs")
        .compile()
        .expect("published DealService Connect code generation");
}

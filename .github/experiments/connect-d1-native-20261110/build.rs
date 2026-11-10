fn main() {
    let protoc = protoc_bin_vendored::protoc_bin_path().expect("vendored protoc");
    unsafe { std::env::set_var("PROTOC", protoc); }
    connectrpc_build::Config::new()
        .files(&["proto/probe.proto"])
        .includes(&["proto/"])
        .include_file("_connectrpc.rs")
        .compile()
        .expect("generate real Connect service");
}

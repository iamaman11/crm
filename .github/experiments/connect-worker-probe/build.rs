fn main() {
    let protoc = protoc_bin_vendored::protoc_bin_path().expect("vendored protoc");
    // The build script runs in its own process, before compiling the crate.
    unsafe { std::env::set_var("PROTOC", protoc); }
    connectrpc_build::Config::new()
        .files(&["proto/probe.proto"])
        .includes(&["proto/"])
        .include_file("_connectrpc.rs")
        .compile()
        .expect("Connect Rust generated service");
}

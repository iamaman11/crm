use std::error::Error;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn Error>> {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let repository_root = manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .ok_or("crm-application-runtime is not under repository crates/")?;
    let proto_root = repository_root.join("proto");
    let gateway_proto = proto_root.join("crm/gateway/v1/gateway.proto");
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    let mut prost = prost_build::Config::new();
    prost.protoc_executable(protoc);

    println!("cargo:rerun-if-changed={}", gateway_proto.display());
    tonic_prost_build::configure()
        .build_client(true)
        .build_server(true)
        .compile_with_config(prost, &[gateway_proto], &[proto_root])?;
    Ok(())
}

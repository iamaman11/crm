use std::path::{Path, PathBuf};

fn active_protos(proto_root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    // The existing Protobuf tree is the only source of Rust service selection.
    // Historical v1 is source-only; Parties v2 was superseded by v3.
    for domain in std::fs::read_dir(proto_root.join("crm")).expect("read CRM Protobuf domains") {
        let domain = domain.expect("read CRM domain");
        if !domain.file_type().expect("CRM domain file type").is_dir() {
            continue;
        }
        for version in std::fs::read_dir(domain.path()).expect("read CRM Protobuf versions") {
            let version = version.expect("read CRM version");
            if !version.file_type().expect("CRM version file type").is_dir() {
                continue;
            }
            let version_name = version.file_name();
            let number = version_name
                .to_str()
                .and_then(|name| name.strip_prefix('v'))
                .and_then(|number| number.parse::<u32>().ok())
                .unwrap_or(0);
            if number < 2 || (domain.file_name().to_str() == Some("parties") && number == 2) {
                continue;
            }
            for entry in std::fs::read_dir(version.path()).expect("read CRM Protobuf files") {
                let path = entry.expect("read CRM Protobuf file").path();
                if path
                    .extension()
                    .is_some_and(|extension| extension == "proto")
                {
                    // Descriptor names use Protobuf's forward-slash namespace on every host.
                    files.push(PathBuf::from(
                        path.to_str().expect("UTF-8 proto path").replace('\\', "/"),
                    ));
                }
            }
        }
    }
    files.sort();
    assert!(!files.is_empty(), "active CRM Protobuf files missing");
    files
}

fn main() {
    let proto_root = Path::new("../../../proto");
    let files = active_protos(proto_root);
    let protoc = protoc_bin_vendored::protoc_bin_path().expect("vendored protoc");
    unsafe {
        std::env::set_var("PROTOC", protoc);
    }
    println!("cargo:rerun-if-changed=../../../proto");
    connectrpc_build::Config::new()
        .files(&files)
        .includes(&["../../../proto"])
        .include_file("_connectrpc.rs")
        .compile()
        .expect("CRM Connect code generation");
}

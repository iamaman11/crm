use std::path::{Path, PathBuf};

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ActiveVersions {
    minimum_version: u32,
    domain_minimums: std::collections::HashMap<String, u32>,
}

fn active_protos(proto_root: &Path) -> Vec<PathBuf> {
    let versions: ActiveVersions =
        serde_json::from_str(include_str!("../../active-proto-versions.json"))
            .expect("active CRM Protobuf versions");
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
            if number
                < *versions
                    .domain_minimums
                    .get(domain.file_name().to_str().expect("UTF-8 domain"))
                    .unwrap_or(&versions.minimum_version)
            {
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
    let proto_root = Path::new("../../../../proto");
    let files = active_protos(proto_root);
    let protoc = protoc_bin_vendored::protoc_bin_path().expect("vendored protoc");
    unsafe {
        std::env::set_var("PROTOC", protoc);
    }
    println!("cargo:rerun-if-changed=../../active-proto-versions.json");
    println!("cargo:rerun-if-changed=../../../../proto");
    connectrpc_build::Config::new()
        .files(&files)
        .includes(&["../../../../proto"])
        .include_file("_connectrpc.rs")
        // Rust Worker implements the generated Connect servers. The browser uses
        // generated TypeScript clients, so compiling unused Rust clients into
        // every WASM feature/profile adds work without testing another RPC path.
        // Generated Rust client types remain optional for a future consumer.
        .gate_client_feature(true)
        .compile()
        .expect("CRM Connect code generation");
}

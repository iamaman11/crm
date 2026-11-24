use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn collect_proto_files(directory: &Path, output: &mut Vec<PathBuf>) {
    let mut entries = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", directory.display()))
        .map(|entry| entry.expect("read Protobuf directory entry").path())
        .collect::<Vec<_>>();
    entries.sort();

    for path in entries {
        if path.is_dir() {
            collect_proto_files(&path, output);
        } else if path.extension().and_then(|value| value.to_str()) == Some("proto") {
            output.push(path);
        }
    }
}

fn main() {
    let manifest_directory = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be configured"),
    );
    let proto_root = manifest_directory.join("../../proto");
    let output_directory =
        PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR must be configured"));

    let mut proto_files = Vec::new();
    if cfg!(feature = "full") {
        collect_proto_files(&proto_root, &mut proto_files);
    } else if cfg!(feature = "sales-activities-link") {
        // protoc resolves transitive imports from the same canonical proto root.
        // No private schema, second generator, or generated source checked in.
        proto_files.extend([
            proto_root.join("crm/activities/v1/task.proto"),
            proto_root.join("crm/sales/v1/deal.proto"),
        ]);
    } else {
        panic!("select a published Protobuf contract feature");
    }
    assert!(
        !proto_files.is_empty(),
        "published Protobuf source set is empty"
    );

    let protoc = protoc_bin_vendored::protoc_bin_path().expect("vendored protoc must be available");
    let mut configuration = prost_build::Config::new();
    configuration.btree_map(["."]);
    configuration.include_file("crm_contracts.rs");
    configuration.file_descriptor_set_path(output_directory.join("crm_contracts_descriptor.bin"));
    configuration.protoc_executable(protoc);
    configuration
        .compile_protos(&proto_files, &[proto_root])
        .expect("published Protobuf contracts must compile");

    println!("cargo:rerun-if-changed=../../proto");
}

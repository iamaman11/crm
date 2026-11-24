#![forbid(unsafe_code)]

include!(concat!(env!("OUT_DIR"), "/crm_contracts.rs"));

use prost::Message;
use prost_types::{DescriptorProto, FileDescriptorProto, FileDescriptorSet};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

pub const FILE_DESCRIPTOR_SET: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/crm_contracts_descriptor.bin"));
pub const MAX_PROTOBUF_BYTES: u64 = 1_048_576;

const MESSAGE_DESCRIPTOR_HASH_PROFILE: &[u8] = b"crm.protobuf.message-descriptor.sha256/v1";
static MESSAGE_DESCRIPTOR_HASHES: OnceLock<BTreeMap<String, [u8; 32]>> = OnceLock::new();

/// Returns the canonical identity of one published Protobuf message contract.
///
/// The identity binds the full message name plus the complete transitive file
/// descriptor closure under the versioned
/// `crm.protobuf.message-descriptor.sha256/v1` profile. Callers use this value
/// to reject payloads whose declared schema name is paired with a different
/// compiled descriptor universe.
pub fn message_descriptor_hash(full_message_name: &str) -> [u8; 32] {
    *MESSAGE_DESCRIPTOR_HASHES
        .get_or_init(build_message_descriptor_hashes)
        .get(full_message_name)
        .unwrap_or_else(|| panic!("generated descriptor set is missing {full_message_name}"))
}

fn build_message_descriptor_hashes() -> BTreeMap<String, [u8; 32]> {
    let descriptor_set = FileDescriptorSet::decode(FILE_DESCRIPTOR_SET)
        .expect("generated Protobuf descriptor set must be valid");
    let files = descriptor_set
        .file
        .into_iter()
        .map(|file| {
            let name = file
                .name
                .clone()
                .expect("generated Protobuf file descriptor must have a name");
            (name, file)
        })
        .collect::<BTreeMap<_, _>>();

    let mut hashes = BTreeMap::new();
    for (file_name, file) in &files {
        let package = file.package.as_deref().unwrap_or_default();
        let mut message_names = Vec::new();
        collect_message_names(package, &file.message_type, &mut message_names);

        let mut closure = BTreeSet::new();
        collect_descriptor_closure(file_name, &files, &mut closure);
        let encoded_closure = closure
            .iter()
            .map(|name| {
                let descriptor = files
                    .get(name)
                    .expect("descriptor dependency must exist in the generated set");
                (name.as_bytes(), descriptor.encode_to_vec())
            })
            .collect::<Vec<_>>();

        for full_message_name in message_names {
            let mut hasher = Sha256::new();
            append_hash_field(&mut hasher, MESSAGE_DESCRIPTOR_HASH_PROFILE);
            append_hash_field(&mut hasher, full_message_name.as_bytes());
            for (dependency_name, encoded_descriptor) in &encoded_closure {
                append_hash_field(&mut hasher, dependency_name);
                append_hash_field(&mut hasher, encoded_descriptor);
            }
            hashes.insert(full_message_name, hasher.finalize().into());
        }
    }
    hashes
}

fn collect_message_names(prefix: &str, messages: &[DescriptorProto], output: &mut Vec<String>) {
    for message in messages {
        let name = message
            .name
            .as_deref()
            .expect("generated message descriptor must have a name");
        let full_name = if prefix.is_empty() {
            name.to_owned()
        } else {
            format!("{prefix}.{name}")
        };
        output.push(full_name.clone());
        collect_message_names(&full_name, &message.nested_type, output);
    }
}

fn collect_descriptor_closure(
    file_name: &str,
    files: &BTreeMap<String, FileDescriptorProto>,
    output: &mut BTreeSet<String>,
) {
    if !output.insert(file_name.to_owned()) {
        return;
    }
    let file = files
        .get(file_name)
        .expect("descriptor closure must reference an existing file");
    for dependency in &file.dependency {
        collect_descriptor_closure(dependency, files, output);
    }
}

fn append_hash_field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::crm::{activities::v1 as activities, sales::v1 as sales};
    use super::{FILE_DESCRIPTOR_SET, message_descriptor_hash};
    use prost::Message;
    use std::collections::BTreeSet;

    #[test]
    fn sales_request_round_trip_uses_generated_contract() {
        let request = sales::CreateDealRequest {
            deal_id: "deal-contract-1".to_owned(),
            name: "Enterprise renewal".to_owned(),
            probability_basis_points: 6_500,
            ..Default::default()
        };

        let bytes = request.encode_to_vec();
        let decoded = sales::CreateDealRequest::decode(bytes.as_slice()).unwrap();

        assert_eq!(decoded, request);
    }

    #[test]
    fn activities_request_round_trip_uses_generated_contract() {
        let request = activities::CreateTaskRequest {
            task_id: "task-contract-1".to_owned(),
            subject: "Prepare renewal proposal".to_owned(),
            description: Some("Attach the approved pricing evidence.".to_owned()),
            priority: 2,
            ..Default::default()
        };

        let bytes = request.encode_to_vec();
        let decoded = activities::CreateTaskRequest::decode(bytes.as_slice()).unwrap();

        assert_eq!(decoded, request);
    }

    #[cfg(feature = "full")]
    #[test]
    fn descriptor_set_contains_required_domain_packages() {
        let descriptor_set = prost_types::FileDescriptorSet::decode(FILE_DESCRIPTOR_SET).unwrap();
        let packages = descriptor_set
            .file
            .into_iter()
            .filter_map(|file| file.package)
            .collect::<BTreeSet<_>>();

        assert!(packages.contains("crm.core.v1"));
        assert!(packages.contains("crm.sales.v1"));
        assert!(packages.contains("crm.activities.v1"));
        assert!(packages.contains("crm.communications.v1"));
        assert!(packages.contains("crm.support.v1"));
        assert!(packages.contains("crm.billing.v1"));
    }

    #[cfg(not(feature = "full"))]
    #[test]
    fn link_descriptor_set_contains_only_transitive_imports() {
        let descriptor_set = prost_types::FileDescriptorSet::decode(FILE_DESCRIPTOR_SET).unwrap();
        let names = descriptor_set
            .file
            .into_iter()
            .map(|file| file.name.unwrap())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            names,
            [
                "crm/activities/v1/task.proto".to_owned(),
                "crm/core/v1/common.proto".to_owned(),
                "crm/sales/v1/deal.proto".to_owned(),
            ]
            .into_iter()
            .collect()
        );
    }

    #[test]
    fn link_descriptor_identity_matches_published_v1_vectors() {
        // Frozen compatibility vectors from the existing published contract
        // hashes. Both feature graphs must preserve these exact identities.
        let hex = |name| {
            message_descriptor_hash(name)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        };
        assert_eq!(
            hex("crm.activities.v1.CreateTaskRequest"),
            "73d33b734e70e762d0f4d7c033e61e2a4fefe23878135e4ba6194c7477c1c9f0"
        );
        assert_eq!(
            hex("crm.sales.v1.DealStageChangedEvent"),
            "ca966c11a09250d1c25f5edd21e211aeffe67e375472e3ba40e429517dd5540e"
        );
    }

    #[test]
    fn message_descriptor_identity_is_stable_per_message_and_distinguishes_contracts() {
        let sales = message_descriptor_hash("crm.sales.v1.DealStageChangedEvent");
        let activities = message_descriptor_hash("crm.activities.v1.CreateTaskRequest");

        assert_eq!(
            sales,
            message_descriptor_hash("crm.sales.v1.DealStageChangedEvent")
        );
        assert_ne!(sales, activities);
    }
}

#![forbid(unsafe_code)]

include!(concat!(env!("OUT_DIR"), "/crm_contracts.rs"));

pub const FILE_DESCRIPTOR_SET: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/crm_contracts_descriptor.bin"));

#[cfg(test)]
mod tests {
    use super::crm::{activities::v1 as activities, sales::v1 as sales};
    use super::FILE_DESCRIPTOR_SET;
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

    #[test]
    fn descriptor_set_contains_required_domain_packages() {
        let descriptor_set =
            prost_types::FileDescriptorSet::decode(FILE_DESCRIPTOR_SET).unwrap();
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
}

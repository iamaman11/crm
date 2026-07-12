use crate::scoped::MetadataCatalog;
use crate::{
    ActivationResult, MetadataBundleDraft, MetadataError, MetadataImpactReport, MetadataRevisionId,
    PublishResult, PublishedMetadataRevision, RollbackResult, TenantMetadataSnapshot,
};
use crm_module_sdk::TenantId;
use std::collections::BTreeMap;

/// Tenant-bound application-facing metadata publication authority.
///
/// Each tenant receives an isolated deterministic catalog engine. Equal bundle
/// content may therefore retain the same content identity across tenants while
/// publication authority, activation generations and rollback history remain
/// independent.
#[derive(Debug, Clone, Default)]
pub struct TenantMetadataCatalog {
    catalogs: BTreeMap<TenantId, MetadataCatalog>,
}

impl TenantMetadataCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn publish(
        &mut self,
        tenant_id: TenantId,
        draft: MetadataBundleDraft,
        published_at_unix_millis: u64,
    ) -> PublishResult {
        self.catalogs
            .entry(tenant_id)
            .or_default()
            .publish(draft, published_at_unix_millis)
    }

    pub fn revision(
        &self,
        tenant_id: &TenantId,
        revision_id: &MetadataRevisionId,
    ) -> Option<&PublishedMetadataRevision> {
        self.catalogs
            .get(tenant_id)
            .and_then(|catalog| catalog.revision(revision_id).ok())
    }

    pub fn tenant_state(&self, tenant_id: &TenantId) -> TenantMetadataSnapshot {
        self.catalogs
            .get(tenant_id)
            .map_or_else(|| empty_catalog().tenant_state(tenant_id), |catalog| catalog.tenant_state(tenant_id))
    }

    pub fn impact_for(
        &self,
        tenant_id: &TenantId,
        candidate_revision: &MetadataRevisionId,
    ) -> Result<MetadataImpactReport, MetadataError> {
        match self.catalogs.get(tenant_id) {
            Some(catalog) => catalog.impact_for(tenant_id, candidate_revision),
            None => empty_catalog().impact_for(tenant_id, candidate_revision),
        }
    }

    pub fn activate(
        &mut self,
        tenant_id: TenantId,
        candidate_revision: &MetadataRevisionId,
        expected_generation: u64,
        allow_breaking_changes: bool,
    ) -> Result<ActivationResult, MetadataError> {
        match self.catalogs.get_mut(&tenant_id) {
            Some(catalog) => catalog.activate(
                tenant_id,
                candidate_revision,
                expected_generation,
                allow_breaking_changes,
            ),
            None => empty_catalog().activate(
                tenant_id,
                candidate_revision,
                expected_generation,
                allow_breaking_changes,
            ),
        }
    }

    pub fn rollback(
        &mut self,
        tenant_id: &TenantId,
        expected_generation: u64,
    ) -> Result<RollbackResult, MetadataError> {
        match self.catalogs.get_mut(tenant_id) {
            Some(catalog) => catalog.rollback(tenant_id, expected_generation),
            None => empty_catalog().rollback(tenant_id, expected_generation),
        }
    }
}

fn empty_catalog() -> MetadataCatalog {
    MetadataCatalog::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MetadataDocument, MetadataErrorCode, MetadataId, MetadataKey, MetadataKind};

    fn tenant(value: &str) -> TenantId {
        TenantId::try_new(value).unwrap()
    }

    fn document(content: &str) -> MetadataDocument {
        MetadataDocument::new(
            MetadataKey::new(
                MetadataKind::Object,
                MetadataId::try_new("crm.sales.deal").unwrap(),
            ),
            "1.0.0",
            content.as_bytes().to_vec(),
            Vec::new(),
        )
        .unwrap()
    }

    fn bundle(content: &str) -> MetadataBundleDraft {
        MetadataBundleDraft::new(vec![document(content)]).unwrap()
    }

    #[test]
    fn publication_authority_is_tenant_scoped() {
        let tenant_a = tenant("tenant-a");
        let tenant_b = tenant("tenant-b");
        let mut catalog = TenantMetadataCatalog::new();
        let published = catalog.publish(tenant_a.clone(), bundle("object-v1"), 100);

        assert!(catalog.revision(&tenant_a, &published.revision_id).is_some());
        assert!(catalog.revision(&tenant_b, &published.revision_id).is_none());
        assert_eq!(
            catalog
                .impact_for(&tenant_b, &published.revision_id)
                .unwrap_err()
                .code,
            MetadataErrorCode::RevisionNotFound
        );
        assert_eq!(
            catalog
                .activate(tenant_b, &published.revision_id, 0, false)
                .unwrap_err()
                .code,
            MetadataErrorCode::RevisionNotFound
        );
    }

    #[test]
    fn identical_content_keeps_identity_without_sharing_authority() {
        let tenant_a = tenant("tenant-a");
        let tenant_b = tenant("tenant-b");
        let mut catalog = TenantMetadataCatalog::new();
        let published_a = catalog.publish(tenant_a.clone(), bundle("object-v1"), 100);
        let published_b = catalog.publish(tenant_b.clone(), bundle("object-v1"), 200);

        assert_eq!(published_a.revision_id, published_b.revision_id);
        catalog
            .activate(tenant_a.clone(), &published_a.revision_id, 0, false)
            .unwrap();
        catalog
            .activate(tenant_b.clone(), &published_b.revision_id, 0, false)
            .unwrap();
        assert_eq!(catalog.tenant_state(&tenant_a).generation, 1);
        assert_eq!(catalog.tenant_state(&tenant_b).generation, 1);
    }

    #[test]
    fn activation_and_rollback_histories_remain_isolated() {
        let tenant_a = tenant("tenant-a");
        let tenant_b = tenant("tenant-b");
        let mut catalog = TenantMetadataCatalog::new();
        let first_a = catalog.publish(tenant_a.clone(), bundle("object-v1"), 100);
        let second_a = catalog.publish(tenant_a.clone(), bundle("object-v2"), 200);
        let only_b = catalog.publish(tenant_b.clone(), bundle("object-b"), 300);

        catalog
            .activate(tenant_a.clone(), &first_a.revision_id, 0, false)
            .unwrap();
        catalog
            .activate(tenant_a.clone(), &second_a.revision_id, 1, false)
            .unwrap();
        catalog
            .activate(tenant_b.clone(), &only_b.revision_id, 0, false)
            .unwrap();

        let rollback = catalog.rollback(&tenant_a, 2).unwrap();
        assert_eq!(rollback.active_revision, first_a.revision_id);
        assert_eq!(catalog.tenant_state(&tenant_a).generation, 3);
        assert_eq!(catalog.tenant_state(&tenant_b).generation, 1);
        assert_eq!(
            catalog.tenant_state(&tenant_b).active_revision,
            Some(only_b.revision_id)
        );
    }
}

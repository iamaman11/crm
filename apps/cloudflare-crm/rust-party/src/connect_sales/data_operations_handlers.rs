//! Admin-only transient CSV operations. The original parser and Party owner own all rules.
use super::super::customer_data_operations as owner;
use super::*;
use crm_customer_data_operations::PartyExportKindFilter;

pub(super) struct PartyCsvV2Service {
    pub(super) env: Arc<Env>,
    pub(super) tenant: String,
    pub(super) request_id: String,
}
impl PartyCsvV2Service {
    fn d1(&self) -> std::result::Result<D1Database, ConnectError> {
        self.env
            .d1("DB")
            .map_err(|_| ConnectError::internal("D1 unavailable"))
    }
}
impl csv_wire::PartyCsvService for PartyCsvV2Service {
    #[worker::send]
    async fn preview_party_export(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, csv_wire::PreviewPartyExportRequest>,
    ) -> ServiceResult<csv_wire::PreviewPartyExportResponse> {
        let input = req.to_owned_message();
        let kind = match input.kind {
            None => None,
            Some(x) if x == party_v3::PartyKind::PARTY_KIND_PERSON => {
                Some(PartyExportKindFilter::Person)
            }
            Some(x) if x == party_v3::PartyKind::PARTY_KIND_ORGANIZATION => {
                Some(PartyExportKindFilter::Organization)
            }
            _ => return Err(ConnectError::invalid_argument("EXPORT_KIND_INVALID")),
        };
        let export = owner::assemble(&self.d1()?, &self.tenant, kind)
            .await
            .map_err(|_| ConnectError::internal("export D1 read failed"))?
            .ok_or_else(|| ConnectError::failed_precondition("EXPORT_SCOPE_LIMIT_REACHED"))?;
        RpcResponse::ok(csv_wire::PreviewPartyExportResponse {
            count: export.count as u32,
            csv_sha256_hex: export.sha256,
            specification_version: export.specification.version_id().as_str().to_owned(),
            fields: export
                .specification
                .profile()
                .fields()
                .iter()
                .map(|f| f.canonical_name().to_owned())
                .collect(),
            maximum_tenant_parties: owner::LIMIT as u32,
            ..Default::default()
        })
    }
    #[worker::send]
    async fn preview_party_import(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, csv_wire::PreviewPartyImportRequest>,
    ) -> ServiceResult<csv_wire::PreviewPartyImportResponse> {
        let preview = owner::preview_import(req.csv, &self.d1()?, &self.tenant)
            .await
            .map_err(|_| ConnectError::internal("import preview D1 read failed"))?
            .map_err(PartyV3Service::mutation_error)?;
        RpcResponse::ok(csv_wire::PreviewPartyImportResponse {
            count: preview.count as u32,
            csv_sha256_hex: preview.hash,
            previously_imported: preview.already as u32,
            target_party_ids: preview.target_ids,
            ..Default::default()
        })
    }
    #[worker::send]
    async fn apply_party_import(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, csv_wire::ApplyPartyImportRequest>,
    ) -> ServiceResult<csv_wire::ApplyPartyImportResponse> {
        let input = req.to_owned_message();
        let (key, _) = wire_precondition(input.precondition.as_option(), true)?;
        let applied = owner::apply_import(
            &input.csv,
            &input.expected_sha256_hex,
            &key,
            &self.d1()?,
            &self.tenant,
            &self.request_id,
        )
        .await
        .map_err(|_| ConnectError::internal("import D1 apply failed"))?
        .map_err(PartyV3Service::mutation_error)?;
        RpcResponse::ok(csv_wire::ApplyPartyImportResponse {
            created_count: applied.saved.created as u32,
            replayed_count: applied.saved.replayed as u32,
            source_sha256_hex: applied.hash,
            receipt: Some(core_v2::MutationReceipt {
                request_id: self.request_id.clone(),
                resource_version: 1,
                replayed: applied.saved.operation_replayed,
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }
}

//! Account-rooted Customer360: typed composition of original owner values.
use super::super::account_overview;
use super::*;
use crm_customer_360_query_adapter::account_overview::COLLECTION_LIMIT;

pub(super) struct Customer360V2Service {
    pub(super) env: Arc<Env>,
    pub(super) tenant: String,
}
impl overview_wire::Customer360Service for Customer360V2Service {
    #[worker::send]
    async fn get_account_overview(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, overview_wire::GetAccountOverviewRequest>,
    ) -> ServiceResult<overview_wire::GetAccountOverviewResponse> {
        let id = AccountV2Service::valid_id(req.account_id)?;
        let db = self
            .env
            .d1("DB")
            .map_err(|_| ConnectError::internal("D1 unavailable"))?;
        let overview = account_overview::overview(&db, &self.tenant, &id)
            .await
            .map_err(|_| ConnectError::internal("Account overview read failed"))?
            .ok_or_else(|| ConnectError::not_found("CUSTOMER_ACCOUNTS_NOT_FOUND"))?;
        let values = overview.sources();
        let truncated = overview.truncation();
        RpcResponse::ok(overview_wire::GetAccountOverviewResponse {
            overview: Some(overview_wire::AccountOverview {
                account: Some(AccountV2Service::convert(&values.account)?).into(),
                primary_party: Some(PartyV3Service::convert(&values.primary_party)?).into(),
                contact_points: values
                    .contact_points
                    .iter()
                    .map(ContactPointV2Service::convert)
                    .collect::<std::result::Result<Vec<_>, _>>()?,
                relationships: values
                    .relationships
                    .iter()
                    .map(RelationshipV2Service::convert)
                    .collect::<std::result::Result<Vec<_>, _>>()?,
                deals: values
                    .deals
                    .iter()
                    .map(SalesV2Service::convert)
                    .collect::<std::result::Result<Vec<_>, _>>()?,
                tasks: values
                    .tasks
                    .iter()
                    .map(|(task, account)| TaskV2Service::convert(task, account))
                    .collect::<std::result::Result<Vec<_>, _>>()?,
                truncated: Some(overview_wire::CollectionTruncation {
                    contact_points: truncated.contact_points,
                    relationships: truncated.relationships,
                    deals: truncated.deals,
                    tasks: truncated.tasks,
                    ..Default::default()
                })
                .into(),
                per_collection_limit: COLLECTION_LIMIT as u32,
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }
}

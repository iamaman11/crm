//! Generated Connect trait for the existing DataQualityV2Service Rust domain owner.
//! Access, tenant, and the single native router are defined by the parent.
use super::*;

impl quality_wire::DataQualityService for DataQualityV2Service {
    #[worker::send]
    async fn evaluate_party_name(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, quality_wire::EvaluatePartyNameRequest>,
    ) -> ServiceResult<quality_wire::EvaluatePartyNameResponse> {
        let id = PartyV3Service::valid_id(req.party_id)?;
        let db = self
            .env
            .d1("DB")
            .map_err(|_| ConnectError::internal("D1 unavailable"))?;
        let evaluation = data_quality::evaluate_typed(&db, &self.tenant, id.as_str())
            .await
            .map_err(|_| ConnectError::internal("DataQuality original owner D1 read failed"))?
            .ok_or_else(|| ConnectError::not_found("PARTIES_NOT_FOUND"))?;
        let mut checks = Vec::with_capacity(evaluation.checks.len());
        for check in &evaluation.checks {
            checks.push(quality_wire::QualityRuleResult {
                rule_key: check.rule_key.clone(),
                title: check.title.clone(),
                remediation: check.remediation.clone(),
                passed: check.passed,
                reason_code: check.reason_code.clone(),
                severity_code: check.severity_code.to_owned(),
                ..Default::default()
            });
        }
        RpcResponse::ok(quality_wire::EvaluatePartyNameResponse {
            party: Some(PartyV3Service::convert(&evaluation.party)?).into(),
            rule_set_version: evaluation.rule_set_version,
            checks,
            failed_count: evaluation.failed_count,
            ..Default::default()
        })
    }
}

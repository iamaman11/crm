//! Native CRM deal HTTP/RPC boundary -> canonical published Protobuf Deal.
//! Only reads the ORIGINAL crm-sales aggregate. Does not implement rules,
//! validity, authorization, or persisted state; those remain at the owner.
use crm_proto_contracts::crm::{core::v1 as core, sales::v1 as sales};
use crm_sales::{Deal, DealOwner, DealStatus};

fn owner(value: &DealOwner) -> core::ActorOrTeamOwner {
    let owner = match value {
        DealOwner::Actor(id) => core::actor_or_team_owner::Owner::ActorId(id.as_str().to_owned()),
        DealOwner::Team(id) => core::actor_or_team_owner::Owner::TeamId(id.as_str().to_owned()),
    };
    core::ActorOrTeamOwner { owner: Some(owner) }
}
fn status(value: DealStatus) -> i32 {
    match value {
        DealStatus::Open => sales::DealStatus::Open as i32,
        DealStatus::Won => sales::DealStatus::Won as i32,
        DealStatus::Lost => sales::DealStatus::Lost as i32,
    }
}
fn resource(value: &crm_module_sdk::ResourceRef, tenant: &str) -> core::ResourceRef {
    core::ResourceRef {
        tenant_id: tenant.to_owned(),
        resource_type: value.resource_type.clone(),
        resource_id: value.resource_id.clone(),
        version: value.version,
    }
}

pub(super) fn deal_view(deal: &Deal, tenant: &str) -> sales::Deal {
    let stage = deal.stage();
    sales::Deal {
        deal_id: deal.deal_id().as_str().to_owned(),
        tenant_id: tenant.to_owned(),
        name: deal.name().to_owned(),
        stage_details: Some(sales::DealStage {
            pipeline_id: stage.pipeline_id().as_str().to_owned(),
            stage_id: stage.stage_id().as_str().to_owned(),
            ordinal: u32::from(stage.ordinal()),
        }),
        amount: deal.amount().map(|money| core::ExactMoney {
            minor_units: money.minor_units().to_string(),
            currency_code: money.currency().as_str().to_owned(),
        }),
        owner: Some(owner(deal.owner())),
        account: deal.account().map(|v| resource(v, tenant)),
        primary_contact: deal.primary_contact().map(|v| resource(v, tenant)),
        expected_close_date: deal.expected_close_date().map(|date| core::CalendarDate {
            year: date.year(),
            month: u32::from(date.month()),
            day: u32::from(date.day()),
        }),
        probability_basis_points: u32::from(deal.probability().get()),
        status: status(deal.status()),
        close_outcome: deal.close_outcome().map(|value| sales::DealCloseOutcome {
            status: status(value.status()),
            reason_code: value.reason_code().as_str().to_owned(),
            closed_at: Some(core::UnixTime {
                unix_nanos: value.closed_at_unix_nanos(),
            }),
        }),
        created_at: Some(core::UnixTime {
            unix_nanos: deal.created_at_unix_nanos(),
        }),
        updated_at: Some(core::UnixTime {
            unix_nanos: deal.updated_at_unix_nanos(),
        }),
        version: deal.version(),
        ..Default::default()
    }
}

//! Account-rooted read composition of original owners, without storage or wire DTOs.
use crm_activities::Task;
use crm_contact_points::ContactPoint;
use crm_customer_accounts::{Account, AccountPartyRole};
use crm_module_sdk::SdkError;
use crm_parties::Party;
use crm_party_relationships::PartyRelationship;
use crm_sales::Deal;

pub const COLLECTION_LIMIT: usize = 100;

pub struct Sources {
    pub account: Account,
    pub primary_party: Party,
    pub contact_points: Vec<ContactPoint>,
    pub relationships: Vec<PartyRelationship>,
    pub deals: Vec<Deal>,
    pub tasks: Vec<Task>,
}

pub struct Truncation {
    pub contact_points: bool,
    pub relationships: bool,
    pub deals: bool,
    pub tasks: bool,
}

pub struct AccountOverview {
    sources: Sources,
    truncated: Truncation,
}

impl AccountOverview {
    /// Storage supplies ordered, tenant-authorized values and LIMIT+1 rows.
    /// Cross-owner references must agree before exposing the composed read model.
    pub fn compose(mut sources: Sources) -> Result<Self, SdkError> {
        let account_id = sources.account.account_id().as_str();
        let party_id = sources.primary_party.party_id().as_str();
        let primary = sources
            .account
            .party_associations()
            .iter()
            .find(|item| item.role() == AccountPartyRole::Primary);
        if primary.is_none_or(|item| item.party().as_str() != party_id)
            || sources
                .contact_points
                .iter()
                .any(|item| item.party_ref().as_str() != party_id)
            || sources.relationships.iter().any(|item| {
                item.from_party_ref().as_str() != party_id
                    && item.to_party_ref().as_str() != party_id
            })
            || sources.deals.iter().any(|item| {
                item.account()
                    .is_none_or(|reference| reference.resource_id != account_id)
            })
            || sources.tasks.iter().any(|item| {
                !item.related_resources().iter().any(|reference| {
                    reference.resource_type == "crm.customer-accounts"
                        && reference.resource_id == account_id
                })
            })
        {
            return Err(SdkError::invalid_argument(
                "customer_360.sources",
                "Overview sources must belong to the Account and its primary Party",
            ));
        }
        let truncated = Truncation {
            contact_points: sources.contact_points.len() > COLLECTION_LIMIT,
            relationships: sources.relationships.len() > COLLECTION_LIMIT,
            deals: sources.deals.len() > COLLECTION_LIMIT,
            tasks: sources.tasks.len() > COLLECTION_LIMIT,
        };
        sources.contact_points.truncate(COLLECTION_LIMIT);
        sources.relationships.truncate(COLLECTION_LIMIT);
        sources.deals.truncate(COLLECTION_LIMIT);
        sources.tasks.truncate(COLLECTION_LIMIT);
        Ok(Self { sources, truncated })
    }

    pub fn sources(&self) -> &Sources {
        &self.sources
    }

    pub fn truncation(&self) -> &Truncation {
        &self.truncated
    }
}

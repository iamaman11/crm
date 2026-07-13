use crate::{
    CREATE_CAPABILITY, CREATE_REQUEST_SCHEMA, CREATE_RESPONSE_SCHEMA, CREATED_EVENT_SCHEMA,
    CREATED_EVENT_TYPE, MODULE_ID, MUTATION_CAPABILITY_IDS, RECORD_TYPE, UPDATE_CAPABILITY,
    UPDATE_REQUEST_SCHEMA, UPDATE_RESPONSE_SCHEMA, UPDATED_EVENT_SCHEMA, UPDATED_EVENT_TYPE,
};
use crm_capability_plan_support::{self as support, EventSpec, PersistedPayloadContract};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest};
use crm_core_data::{
    AggregatePresence, AggregateTarget, BatchMutationPlan, CapabilityBatchExecutionPlan,
    RecordMutation, TransactionalAggregatePlanner,
};
use crm_customer_accounts::{
    ACCOUNT_STATE_MAXIMUM_BYTES, ACCOUNT_STATE_RETENTION_POLICY_ID, ACCOUNT_STATE_SCHEMA_ID,
    ACCOUNT_STATE_SCHEMA_VERSION, Account, AccountId, AccountPartyAssociation, AccountPartyRole,
    AccountStatus, CreateAccount, PartyReference, UpdateAccount, account_state_descriptor_hash,
    decode_account_state, encode_account_state,
};
use crm_module_sdk::{DataClass, RecordSnapshot, SdkError};
use crm_proto_contracts::crm::{accounts::v1 as wire, core::v1 as core, customer::v1 as customer};

#[derive(Debug, Default, Clone, Copy)]
pub struct CustomerAccountCapabilityPlanner;

impl TransactionalAggregatePlanner for CustomerAccountCapabilityPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        ensure_definition(definition, request)?;
        let (account_id, presence) = match definition.capability_id.as_str() {
            CREATE_CAPABILITY => {
                let command: wire::CreateAccountRequest = support::decode_request_with_data_class(
                    request,
                    MODULE_ID,
                    CREATE_REQUEST_SCHEMA,
                    DataClass::Personal,
                )?;
                (
                    account_id_from_ref(command.account_ref, "account.account_ref")?,
                    AggregatePresence::MustBeAbsent,
                )
            }
            UPDATE_CAPABILITY => {
                let command: wire::UpdateAccountRequest = support::decode_request_with_data_class(
                    request,
                    MODULE_ID,
                    UPDATE_REQUEST_SCHEMA,
                    DataClass::Personal,
                )?;
                (
                    account_id_from_ref(command.account_ref, "account.account_ref")?,
                    AggregatePresence::MustExist,
                )
            }
            _ => return Err(unsupported_capability()),
        };

        Ok(AggregateTarget {
            reference: support::record_ref(
                RECORD_TYPE,
                account_id.as_str(),
                "account.account_ref.account_id",
            )?,
            presence,
        })
    }

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        ensure_definition(definition, request)?;
        match definition.capability_id.as_str() {
            CREATE_CAPABILITY => plan_create(definition, request, current),
            UPDATE_CAPABILITY => plan_update(definition, request, current),
            _ => Err(unsupported_capability()),
        }
    }
}

fn plan_create(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    current: Option<&RecordSnapshot>,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    if current.is_some() {
        return Err(invalid_plan());
    }

    let command: wire::CreateAccountRequest = support::decode_request_with_data_class(
        request,
        MODULE_ID,
        CREATE_REQUEST_SCHEMA,
        DataClass::Personal,
    )?;
    let account = Account::create(CreateAccount {
        account_id: account_id_from_ref(command.account_ref, "account.account_ref")?,
        name: command.name,
        party_associations: associations_from_wire(command.party_associations)?,
        occurred_at_unix_nanos: request.context.execution.request_started_at_unix_nanos,
    })?;

    let aggregate = support::record_ref(
        RECORD_TYPE,
        account.account_id().as_str(),
        "account.account_ref.account_id",
    )?;
    let public_account = account_to_wire(&account);
    let output = support::protobuf_payload(
        MODULE_ID,
        CREATE_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::CreateAccountResponse {
            account: Some(public_account.clone()),
        },
    )?;
    let event = support::event_evidence_with_data_class(
        request,
        aggregate.clone(),
        MODULE_ID,
        EventSpec {
            event_type: CREATED_EVENT_TYPE,
            event_schema_id: CREATED_EVENT_SCHEMA,
            aggregate_version: account.version(),
            previous_version: None,
        },
        DataClass::Personal,
        &wire::AccountCreatedEvent {
            account: Some(public_account),
        },
    )?;

    mutation_plan(
        definition,
        request,
        aggregate.clone(),
        RecordMutation::Create {
            reference: aggregate,
            payload: persisted_payload(&account)?,
        },
        event,
        output,
    )
}

fn plan_update(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    current: Option<&RecordSnapshot>,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    let current = current.ok_or_else(invalid_plan)?;
    let command: wire::UpdateAccountRequest = support::decode_request_with_data_class(
        request,
        MODULE_ID,
        UPDATE_REQUEST_SCHEMA,
        DataClass::Personal,
    )?;
    let requested_account_id = account_id_from_ref(command.account_ref, "account.account_ref")?;
    if requested_account_id.as_str() != current.reference.record_id.as_str() {
        return Err(invalid_plan());
    }

    let mut account = account_from_snapshot(current)?;
    account.apply_update(UpdateAccount {
        expected_version: command.expected_version,
        name: command.name,
        status: account_status_from_wire(command.status)?,
        party_associations: associations_from_wire(command.party_associations)?,
        occurred_at_unix_nanos: request.context.execution.request_started_at_unix_nanos,
    })?;

    let aggregate = current.reference.clone();
    let public_account = account_to_wire(&account);
    let output = support::protobuf_payload(
        MODULE_ID,
        UPDATE_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::UpdateAccountResponse {
            account: Some(public_account.clone()),
        },
    )?;
    let event = support::event_evidence_with_data_class(
        request,
        aggregate.clone(),
        MODULE_ID,
        EventSpec {
            event_type: UPDATED_EVENT_TYPE,
            event_schema_id: UPDATED_EVENT_SCHEMA,
            aggregate_version: account.version(),
            previous_version: Some(current.version),
        },
        DataClass::Personal,
        &wire::AccountUpdatedEvent {
            account: Some(public_account),
        },
    )?;

    mutation_plan(
        definition,
        request,
        aggregate.clone(),
        RecordMutation::Update {
            reference: aggregate,
            expected_version: current.version,
            payload: persisted_payload(&account)?,
        },
        event,
        output,
    )
}

fn mutation_plan(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    aggregate: crm_module_sdk::RecordRef,
    mutation: RecordMutation,
    event: crm_core_data::EventEvidence,
    output: crm_module_sdk::TypedPayload,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    let audit = support::audit_intent(
        request,
        &aggregate,
        event.aggregate_version,
        definition.capability_id.as_str(),
        &output.bytes,
    )?;

    Ok(CapabilityBatchExecutionPlan {
        batch: BatchMutationPlan {
            context: request.context.clone(),
            records: vec![mutation],
            relationships: Vec::new(),
            events: vec![event],
            idempotency: support::capability_idempotency(definition, request)?,
            audits: vec![audit],
        },
        output: Some(output),
    })
}

pub fn account_to_wire(account: &Account) -> wire::Account {
    wire::Account {
        account_ref: Some(customer::AccountRef {
            account_id: account.account_id().as_str().to_owned(),
        }),
        name: account.name().to_owned(),
        status: match account.status() {
            AccountStatus::Active => wire::AccountStatus::Active as i32,
            AccountStatus::Inactive => wire::AccountStatus::Inactive as i32,
        },
        party_associations: account
            .party_associations()
            .iter()
            .map(|association| wire::AccountPartyAssociation {
                party_ref: Some(customer::PartyRef {
                    party_id: association.party().as_str().to_owned(),
                }),
                role: match association.role() {
                    AccountPartyRole::Primary => wire::AccountPartyRole::Primary as i32,
                    AccountPartyRole::Member => wire::AccountPartyRole::Member as i32,
                },
            })
            .collect(),
        resource_version: Some(customer::CustomerResourceVersion {
            version: account.version(),
            created_at: Some(core::UnixTime {
                unix_nanos: account.created_at_unix_nanos(),
            }),
            updated_at: Some(core::UnixTime {
                unix_nanos: account.updated_at_unix_nanos(),
            }),
        }),
    }
}

pub fn persisted_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: ACCOUNT_STATE_SCHEMA_ID,
        schema_version: ACCOUNT_STATE_SCHEMA_VERSION,
        descriptor_hash: account_state_descriptor_hash(),
        maximum_size_bytes: ACCOUNT_STATE_MAXIMUM_BYTES,
        retention_policy_id: ACCOUNT_STATE_RETENTION_POLICY_ID,
    }
}

pub fn persisted_payload(account: &Account) -> Result<crm_module_sdk::TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        persisted_contract(),
        DataClass::Personal,
        encode_account_state(account)?,
    )
}

pub fn account_from_snapshot(snapshot: &RecordSnapshot) -> Result<Account, SdkError> {
    let account = decode_account_state(support::persisted_json_bytes_with_data_class(
        snapshot,
        persisted_contract(),
        DataClass::Personal,
    )?)?;
    if account.account_id().as_str() != snapshot.reference.record_id.as_str()
        || account.version() != snapshot.version
    {
        return Err(support::stored_data_error(
            "CUSTOMER_ACCOUNTS_PERSISTED_ACCOUNT_IDENTITY_INVALID",
        ));
    }
    Ok(account)
}

pub fn referenced_party_ids_from_create(
    request: &CapabilityRequest,
) -> Result<Vec<PartyReference>, SdkError> {
    let command: wire::CreateAccountRequest = support::decode_request_with_data_class(
        request,
        MODULE_ID,
        CREATE_REQUEST_SCHEMA,
        DataClass::Personal,
    )?;
    party_references_from_wire(command.party_associations)
}

pub fn referenced_party_ids_from_update(
    request: &CapabilityRequest,
) -> Result<Vec<PartyReference>, SdkError> {
    let command: wire::UpdateAccountRequest = support::decode_request_with_data_class(
        request,
        MODULE_ID,
        UPDATE_REQUEST_SCHEMA,
        DataClass::Personal,
    )?;
    party_references_from_wire(command.party_associations)
}

fn account_id_from_ref(
    account_ref: Option<customer::AccountRef>,
    field: &'static str,
) -> Result<AccountId, SdkError> {
    let account_ref = account_ref
        .ok_or_else(|| SdkError::invalid_argument(field, "Account reference is required"))?;
    AccountId::try_new(account_ref.account_id)
}

fn associations_from_wire(
    associations: Vec<wire::AccountPartyAssociation>,
) -> Result<Vec<AccountPartyAssociation>, SdkError> {
    associations
        .into_iter()
        .map(|association| {
            let party_ref = association.party_ref.ok_or_else(|| {
                SdkError::invalid_argument(
                    "account.party_associations.party_ref",
                    "Party reference is required",
                )
            })?;
            Ok(AccountPartyAssociation::new(
                PartyReference::try_new(party_ref.party_id)?,
                account_party_role_from_wire(association.role)?,
            ))
        })
        .collect()
}

fn party_references_from_wire(
    associations: Vec<wire::AccountPartyAssociation>,
) -> Result<Vec<PartyReference>, SdkError> {
    associations
        .into_iter()
        .map(|association| {
            let party_ref = association.party_ref.ok_or_else(|| {
                SdkError::invalid_argument(
                    "account.party_associations.party_ref",
                    "Party reference is required",
                )
            })?;
            PartyReference::try_new(party_ref.party_id)
        })
        .collect()
}

fn account_party_role_from_wire(value: i32) -> Result<AccountPartyRole, SdkError> {
    match wire::AccountPartyRole::try_from(value) {
        Ok(wire::AccountPartyRole::Primary) => Ok(AccountPartyRole::Primary),
        Ok(wire::AccountPartyRole::Member) => Ok(AccountPartyRole::Member),
        Ok(wire::AccountPartyRole::Unspecified) | Err(_) => Err(SdkError::invalid_argument(
            "account.party_associations.role",
            "Account Party role must be PRIMARY or MEMBER",
        )),
    }
}

fn account_status_from_wire(value: i32) -> Result<AccountStatus, SdkError> {
    match wire::AccountStatus::try_from(value) {
        Ok(wire::AccountStatus::Active) => Ok(AccountStatus::Active),
        Ok(wire::AccountStatus::Inactive) => Ok(AccountStatus::Inactive),
        Ok(wire::AccountStatus::Unspecified) | Err(_) => Err(SdkError::invalid_argument(
            "account.status",
            "Account status must be ACTIVE or INACTIVE",
        )),
    }
}

fn ensure_definition(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    if !MUTATION_CAPABILITY_IDS.contains(&definition.capability_id.as_str())
        || definition.owner_module_id.as_str() != MODULE_ID
        || definition.capability_id.as_str() != request.context.execution.capability_id.as_str()
    {
        return Err(invalid_plan());
    }
    Ok(())
}

fn invalid_plan() -> SdkError {
    SdkError::new(
        "CUSTOMER_ACCOUNTS_CAPABILITY_PLAN_INVALID",
        crm_module_sdk::ErrorCategory::Internal,
        false,
        "The Account capability could not be planned safely.",
    )
}

fn unsupported_capability() -> SdkError {
    SdkError::new(
        "CUSTOMER_ACCOUNTS_CAPABILITY_UNSUPPORTED",
        crm_module_sdk::ErrorCategory::Internal,
        false,
        "The Account capability is not configured.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_mapping_preserves_roles_status_and_version_metadata() {
        let account = Account::create(CreateAccount {
            account_id: AccountId::try_new("account-wire-1").unwrap(),
            name: "Northwind".to_owned(),
            party_associations: vec![AccountPartyAssociation::new(
                PartyReference::try_new("party-org-northwind").unwrap(),
                AccountPartyRole::Primary,
            )],
            occurred_at_unix_nanos: 42,
        })
        .unwrap();

        let wire = account_to_wire(&account);
        assert_eq!(wire.account_ref.unwrap().account_id, "account-wire-1");
        assert_eq!(wire.status, wire::AccountStatus::Active as i32);
        assert_eq!(wire.party_associations.len(), 1);
        assert_eq!(
            wire.party_associations[0].role,
            wire::AccountPartyRole::Primary as i32
        );
        assert_eq!(wire.resource_version.unwrap().version, 1);
    }
}

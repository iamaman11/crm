#![forbid(unsafe_code)]

//! Production-neutral composition for the optional Sales → Activities link module.
//!
//! This crate owns no business data and no transport. It adapts one immutable published Sales
//! event into the pure link-module core, builds the consumer execution context and delegates all
//! target mutation and durable private-state access through governed SDK ports.

use crm_core_events::{
    EventHandlingDisposition, EventHandlingResult, GovernedEventHandler,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityClient, CapabilityId, CapabilityVersion, CausationId,
    Clock, ExecutionContext, IdempotencyKey, ModuleExecutionContext, ModuleId, ModuleStateStore,
    PortFuture, PortResult, RequestId, SchemaVersion, SdkError,
};
use crm_sales_activities_link::{
    LinkDisposition, MODULE_ID, SalesActivitiesLink,
};
use crm_sales_activities_link_contract_adapter::ProtobufSalesActivitiesLinkContractAdapter;
use sha2::{Digest, Sha256};
use std::fmt;
use std::sync::Arc;

const PROCESS_CAPABILITY_ID: &str = "link.sales.stage-changed.process";
const PROCESS_CAPABILITY_VERSION: &str = "1.0.0";
const PROCESS_SCHEMA_VERSION: &str = "1.0.0";
const IDENTITY_HASH_PROFILE: &[u8] = b"crm.sales-activities-link.execution-context/v1";

#[derive(Clone)]
pub struct SalesActivitiesLinkEventHandler {
    link: Arc<SalesActivitiesLink<ProtobufSalesActivitiesLinkContractAdapter>>,
    adapter: ProtobufSalesActivitiesLinkContractAdapter,
    capabilities: Arc<dyn CapabilityClient>,
    state: Arc<dyn ModuleStateStore>,
    clock: Arc<dyn Clock>,
    service_actor_id: ActorId,
}

impl fmt::Debug for SalesActivitiesLinkEventHandler {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SalesActivitiesLinkEventHandler")
            .field("link", &"SalesActivitiesLink")
            .field("adapter", &self.adapter)
            .field("capabilities", &"dyn CapabilityClient")
            .field("state", &"dyn ModuleStateStore")
            .field("clock", &"dyn Clock")
            .field("service_actor_id", &self.service_actor_id)
            .finish()
    }
}

impl SalesActivitiesLinkEventHandler {
    pub fn new(
        capabilities: Arc<dyn CapabilityClient>,
        state: Arc<dyn ModuleStateStore>,
        clock: Arc<dyn Clock>,
        service_actor_id: ActorId,
    ) -> Self {
        let adapter = ProtobufSalesActivitiesLinkContractAdapter;
        Self {
            link: Arc::new(SalesActivitiesLink::new(adapter)),
            adapter,
            capabilities,
            state,
            clock,
            service_actor_id,
        }
    }

    fn execution_context(
        &self,
        delivery: &crm_module_sdk::EventDelivery,
    ) -> Result<ModuleExecutionContext, SdkError> {
        let now = self.clock.now_unix_nanos();
        if now < 0 {
            return Err(SdkError::new(
                "LINK_CLOCK_INVALID",
                crm_module_sdk::ErrorCategory::Unavailable,
                true,
                "The link processing clock is temporarily unavailable.",
            ));
        }
        let identity = deterministic_identity(delivery.delivery_id.as_str());
        Ok(ModuleExecutionContext {
            module_id: configured_module_id(MODULE_ID)?,
            execution: ExecutionContext {
                tenant_id: delivery.tenant_id.clone(),
                actor_id: self.service_actor_id.clone(),
                request_id: RequestId::try_new(format!("link-request-{identity}"))
                    .map_err(|_| configuration_invalid())?,
                correlation_id: delivery.correlation_id.clone(),
                causation_id: CausationId::try_new(delivery.event_id.as_str().to_owned())
                    .map_err(|_| configuration_invalid())?,
                trace_id: delivery.trace_id.clone(),
                capability_id: CapabilityId::try_new(PROCESS_CAPABILITY_ID)
                    .map_err(|_| configuration_invalid())?,
                capability_version: CapabilityVersion::try_new(PROCESS_CAPABILITY_VERSION)
                    .map_err(|_| configuration_invalid())?,
                idempotency_key: IdempotencyKey::try_new(delivery.delivery_id.as_str().to_owned())
                    .map_err(|_| configuration_invalid())?,
                business_transaction_id: BusinessTransactionId::try_new(format!(
                    "link-transaction-{identity}"
                ))
                .map_err(|_| configuration_invalid())?,
                schema_version: SchemaVersion::try_new(PROCESS_SCHEMA_VERSION)
                    .map_err(|_| configuration_invalid())?,
                request_started_at_unix_nanos: now,
            },
        })
    }
}

impl GovernedEventHandler for SalesActivitiesLinkEventHandler {
    fn handle<'a>(
        &'a self,
        delivery: &'a crm_module_sdk::EventDelivery,
    ) -> PortFuture<'a, PortResult<EventHandlingResult>> {
        Box::pin(async move {
            let event = self.adapter.decode_sales_deal_stage_changed(delivery)?;
            let context = self.execution_context(delivery)?;
            let result = self
                .link
                .handle(
                    &context,
                    delivery,
                    &event,
                    self.capabilities.as_ref(),
                    self.state.as_ref(),
                )
                .await?;
            Ok(EventHandlingResult {
                disposition: match result.disposition {
                    LinkDisposition::Applied => EventHandlingDisposition::Applied,
                    LinkDisposition::Ignored => EventHandlingDisposition::Ignored,
                },
                replayed: result.replayed,
                affected_resources: result.affected_resources,
            })
        })
    }
}

fn deterministic_identity(delivery_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(IDENTITY_HASH_PROFILE);
    hasher.update((delivery_id.len() as u64).to_be_bytes());
    hasher.update(delivery_id.as_bytes());
    let digest = hasher.finalize();
    let mut output = String::with_capacity(32);
    for byte in &digest[..16] {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn configured_module_id(value: &str) -> Result<ModuleId, SdkError> {
    ModuleId::try_new(value).map_err(|_| configuration_invalid())
}

fn configuration_invalid() -> SdkError {
    SdkError::new(
        "LINK_CONFIGURATION_INVALID",
        crm_module_sdk::ErrorCategory::Internal,
        false,
        "The link module configuration is invalid.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_identity_is_stable_and_bounded() {
        let first = deterministic_identity("delivery-1");
        let same = deterministic_identity("delivery-1");
        let different = deterministic_identity("delivery-2");
        assert_eq!(first, same);
        assert_ne!(first, different);
        assert_eq!(first.len(), 32);
    }
}

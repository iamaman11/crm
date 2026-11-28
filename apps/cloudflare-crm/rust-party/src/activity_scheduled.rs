//! Activities-owned implementation of the sole native scheduled entrypoint.
//! Cron reads immutable Deal events and overdue Tasks, then uses the existing
//! Automation run ledger and original Task owner. No HTTP/RPC emulation.
#[cfg(feature = "native")]
use worker::*;

#[cfg(feature = "native")]
pub(super) async fn scheduled(event: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
    // Cloudflare exposes scheduledTime in milliseconds since Unix epoch.
    // Use the event's time for business due checks, not the Worker wall clock.
    let scheduled_millis = event.schedule();
    let scheduled_nanos = if scheduled_millis.is_finite() && scheduled_millis >= 0.0 {
        (scheduled_millis as i64).checked_mul(1_000_000)
    } else {
        None
    };
    let result = async {
        let db = env.d1("DB")?;
        let scheduled_nanos = scheduled_nanos.ok_or_else(|| {
            worker::Error::RustError("CRM_AUTOMATION_INVALID_SCHEDULED_TIME".to_owned())
        })?;
        // A delayed Cron invocation must catch up to the actual clock, while
        // tests can advance the scheduled event into the future deterministically.
        let now = scheduled_nanos.max(super::now_nanos());
        super::activity_automation::dispatch_scheduled(&db, now)
            .await
            .map_err(|error| worker::Error::RustError(format!("{error:?}")))
    }
    .await;
    match result {
        Ok(summary) => console_log!("CRM_AUTOMATION_SCHEDULED_DISPATCH: {:?}", summary),
        Err(error) => console_error!("CRM_AUTOMATION_SCHEDULED_DISPATCH_FAILED: {:?}", error),
    }
}

#[cfg(not(feature = "native"))]
pub(super) async fn scheduled(
    _event: worker::ScheduledEvent,
    _env: worker::Env,
    _ctx: worker::ScheduleContext,
) {
}

//! Activities-owned implementation of the sole native scheduled entrypoint.
//! Cron reads immutable Deal events and overdue Tasks, then uses the existing
//! Automation run ledger and original Task owner. No HTTP/RPC emulation.
#[cfg(feature = "native")]
use worker::*;

#[cfg(feature = "native")]
fn evaluation_time_nanos(event: &ScheduledEvent) -> i64 {
    let scheduled_millis = event.schedule();
    let scheduled_nanos = if scheduled_millis.is_finite() && scheduled_millis > 0.0 {
        (scheduled_millis * 1_000_000.0) as i64
    } else {
        0
    };
    super::now_nanos().max(scheduled_nanos)
}

#[cfg(feature = "native")]
pub(super) async fn scheduled(event: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
    let result = async {
        let db = env.d1("DB")?;
        super::activity_automation::dispatch_scheduled(&db, evaluation_time_nanos(&event))
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

//! Activities-owned implementation of the sole native scheduled entrypoint.
//! Cron reads immutable Deal events and overdue Tasks, then uses the existing
//! Automation run ledger and original Task owner. No HTTP/RPC emulation.
use worker::*;

pub(super) async fn scheduled(_event: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
    let result = async {
        let db = env.d1("DB")?;
        super::activity_automation::dispatch_scheduled(&db, super::now_nanos())
            .await
            .map_err(|error| worker::Error::RustError(format!("{error:?}")))
    }
    .await;
    match result {
        Ok(summary) => console_log!("CRM_AUTOMATION_SCHEDULED_DISPATCH: {:?}", summary),
        Err(error) => console_error!("CRM_AUTOMATION_SCHEDULED_DISPATCH_FAILED: {:?}", error),
    }
}

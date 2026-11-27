//! Activities-owned implementation of the sole native scheduled entrypoint.
//! Keep the existing disabled probe until autonomous execution is accepted.
use serde::Deserialize;
use worker::*;

pub(super) async fn scheduled(_event: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
    #[derive(Deserialize)]
    struct Schema {
        present: i64,
    }
    let result = async {
        let db = env.d1("DB")?;
        let schema = db
            .prepare(concat!(
                "SELECT COUNT(*) AS present FROM sqlite_master ",
                "WHERE type='table' AND name='crm_activity_automation_source_events'"
            ))
            .first::<Schema>(None)
            .await?;
        Ok::<_, worker::Error>(schema.is_some_and(|row| row.present == 1))
    }
    .await;
    match result {
        Ok(true) => console_log!("CRM_AUTOMATION_SCHEDULED_DISPATCH_DISABLED"),
        Ok(false) => console_error!("CRM_AUTOMATION_SCHEMA_MISSING"),
        Err(error) => console_error!("CRM_AUTOMATION_SOURCE_READ_FAILED: {error}"),
    }
}

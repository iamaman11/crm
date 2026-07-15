use crate::{
    ApplicationConfig, ApplicationRuntime, ApplicationRuntimeError, PartyExportExecutionProcess,
};
use std::error::Error;
use tokio::task::JoinError;

/// Starts the production CRM process from validated environment configuration.
///
/// The binary host deliberately owns no runtime dependencies or composition
/// logic. Tokio construction, configuration, dependency assembly and graceful
/// shutdown all remain inside the application composition boundary.
pub fn run_from_env() -> Result<(), Box<dyn Error>> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        let config = ApplicationConfig::from_env()?;
        let application = ApplicationRuntime::assemble(config.clone()).await?;
        let export_execution =
            PartyExportExecutionProcess::assemble(&config, application.components().store.clone())?;
        supervise_process(application, export_execution).await
    })?;
    Ok(())
}

async fn supervise_process(
    application: ApplicationRuntime,
    export_execution: PartyExportExecutionProcess,
) -> Result<(), ApplicationRuntimeError> {
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let mut application_task = tokio::spawn(application.run_until_signal());
    let mut export_execution_task = tokio::spawn(export_execution.run_until_shutdown(shutdown_rx));

    tokio::select! {
        application_result = &mut application_task => {
            let _ = shutdown_tx.send(true);
            let application_result = join_result("application runtime", application_result);
            let export_execution_result = join_result(
                "Party export execution runtime",
                export_execution_task.await,
            );
            application_result?;
            export_execution_result
        }
        export_execution_result = &mut export_execution_task => {
            application_task.abort();
            join_result("Party export execution runtime", export_execution_result)
        }
    }
}

fn join_result(
    name: &'static str,
    result: Result<Result<(), ApplicationRuntimeError>, JoinError>,
) -> Result<(), ApplicationRuntimeError> {
    result.map_err(|error| ApplicationRuntimeError::Task(format!("{name}: {error}")))?
}

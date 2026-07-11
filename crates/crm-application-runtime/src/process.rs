use crate::{ApplicationConfig, ApplicationRuntime};
use std::error::Error;

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
        let application = ApplicationRuntime::assemble(config).await?;
        application.run_until_signal().await
    })?;
    Ok(())
}

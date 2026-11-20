fn main() {
    if let Err(error) = crm_application_runtime::run_from_env() {
        eprintln!("crm-api startup failed: {error}");
        std::process::exit(1);
    }
}

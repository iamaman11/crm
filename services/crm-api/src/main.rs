fn main() {
    if std::env::args().nth(1).as_deref() == Some("--published-contracts") {
        match crm_application_runtime::published_contract_catalog_json() {
            Ok(json) => println!("{json}"),
            Err(error) => {
                eprintln!("crm-api contract export failed: {error}");
                std::process::exit(1);
            }
        }
        return;
    }
    if let Err(error) = crm_application_runtime::run_from_env() {
        eprintln!("crm-api startup failed: {error}");
        std::process::exit(1);
    }
}

use crm_core_data::PostgresDataStore;

#[derive(Clone)]
pub struct PostgresPartyEvaluationStageSink {
    store: PostgresDataStore,
}

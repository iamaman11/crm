use crm_customer_privacy_application::RetentionEvaluationPersistencePort;
use crm_customer_privacy_postgres::PostgresRetentionEvaluationPersistence;

#[test]
fn postgres_retention_evaluation_is_a_shareable_internal_port() {
    fn require_port<T: RetentionEvaluationPersistencePort + Send + Sync>() {}

    require_port::<PostgresRetentionEvaluationPersistence>();
}

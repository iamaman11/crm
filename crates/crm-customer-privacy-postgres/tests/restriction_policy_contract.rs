use crm_core_data::TransactionalCustomerSubjectPolicyPort;
use crm_customer_privacy_postgres::PostgresCustomerPrivacySubjectPolicy;

#[test]
fn postgres_restriction_policy_is_a_shareable_final_decision_port() {
    fn require_policy<T: TransactionalCustomerSubjectPolicyPort + Send + Sync>() {}

    require_policy::<PostgresCustomerPrivacySubjectPolicy>();
}

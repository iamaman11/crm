use crm_data_quality_capability_adapter::{
    MUTATION_CAPABILITY_IDS, STAGE_PARTY_EVALUATION_INPUT_CAPABILITY,
};
use crm_data_quality_source_composition::{
    DEFAULT_EVALUATION_STAGE_SCAN_PAGE_SIZE, EVALUATION_WORKER_ACTOR_ID,
    EVALUATION_WORKER_CAPABILITY_VERSION,
};

#[test]
fn staging_worker_surface_is_bounded_and_versioned() {
    assert!(DEFAULT_EVALUATION_STAGE_SCAN_PAGE_SIZE > 0);
    assert!(
        DEFAULT_EVALUATION_STAGE_SCAN_PAGE_SIZE <= crm_core_data::MAXIMUM_RECORD_QUERY_PAGE_SIZE
    );
    assert_eq!(
        EVALUATION_WORKER_ACTOR_ID,
        "crm-api-data-quality-evaluation-worker"
    );
    assert_eq!(EVALUATION_WORKER_CAPABILITY_VERSION, "1.0.0");
    assert_eq!(
        STAGE_PARTY_EVALUATION_INPUT_CAPABILITY,
        "data_quality.party.evaluation.internal.stage"
    );
    assert!(!MUTATION_CAPABILITY_IDS.contains(&STAGE_PARTY_EVALUATION_INPUT_CAPABILITY));
}

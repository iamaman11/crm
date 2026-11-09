//! Executable integration gate for ORIGINAL Rust owner behavior in WASM.
//! It runs in memory, without writes. It is deliberately NOT a D1 or
//! published feature proof. Only the CI-native Worker enables this module.
//! All business rules and codecs are those from the original owner crates.

pub(crate) fn check() -> i32 {
    // crm.customer-accounts: a commercial account is NOT a Party.
    let account = match crm_customer_accounts::Account::create(
        crm_customer_accounts::CreateAccount {
            account_id: crm_customer_accounts::AccountId::try_new("wasm-account-01").unwrap(),
            name: "  WASM   Account  ".into(),
            party_associations: vec![crm_customer_accounts::AccountPartyAssociation::new(
                crm_customer_accounts::PartyReference::try_new("wasm-party-01").unwrap(),
                crm_customer_accounts::AccountPartyRole::Primary,
            )],
            occurred_at_unix_nanos: 100,
        },
    ) { Ok(x) => x, Err(_) => return 11 };
    if account.version() != 1 || account.name() != "WASM Account" { return 12; }
    let bytes = match crm_customer_accounts::encode_account_state(&account) {
        Ok(x) => x, Err(_) => return 13,
    };
    if crm_customer_accounts::decode_account_state(&bytes).ok() != Some(account.clone()) {
        return 14;
    }
    if crm_customer_accounts::Account::create(crm_customer_accounts::CreateAccount {
        account_id: crm_customer_accounts::AccountId::try_new("wasm-account-02").unwrap(),
        name: " ".into(), party_associations: vec![], occurred_at_unix_nanos: 100,
    }).is_ok() { return 15; }

    // crm.contact-points: canonical Party endpoint, verified by owner.
    let mut contact = match crm_contact_points::ContactPoint::create(
        crm_contact_points::CreateContactPoint {
            contact_point_id: crm_contact_points::ContactPointId::try_new("wasm-contact-01").unwrap(),
            party_ref: crm_contact_points::PartyReference::try_new("wasm-party-01").unwrap(),
            kind: crm_contact_points::ContactPointKind::Email,
            value: "proof@example.org".into(), preferred: true,
            valid_from_unix_nanos: None, valid_until_unix_nanos: None,
            occurred_at_unix_nanos: 100,
        },
    ) { Ok(x) => x, Err(_) => return 21 };
    if contact.version() != 1 || contact.normalized_value().is_empty() { return 22; }
    if contact.verify(crm_contact_points::VerifyContactPoint {
        expected_version: 1, evidence_ref: "wasm-proof".into(), occurred_at_unix_nanos: 101,
    }).is_err() { return 23; }
    if !contact.verification().is_verified() || contact.version() != 2 { return 24; }
    let bytes = match crm_contact_points::encode_contact_point_state(&contact) {
        Ok(x) => x, Err(_) => return 25,
    };
    if crm_contact_points::decode_contact_point_state(&bytes).ok() != Some(contact.clone()) {
        return 26;
    }
    if contact.verify(crm_contact_points::VerifyContactPoint {
        expected_version: 1, evidence_ref: "stale".into(), occurred_at_unix_nanos: 102,
    }).is_ok() { return 27; }

    // crm.sales: authoritative deal stage and persisted-state invariants.
    let deal = match crm_sales::Deal::create(crm_sales::CreateDeal {
        deal_id: crm_module_sdk::RecordId::try_new("wasm-deal-01").unwrap(),
        name: "WASM Deal".into(),
        owner: crm_sales::DealOwner::Team(crm_sales::TeamId::try_new("sales-team-01").unwrap()),
        account: None, primary_contact: None,
        stage: crm_sales::DealStage::try_new(
            crm_sales::PipelineId::try_new("main-pipeline").unwrap(),
            crm_sales::StageId::try_new("new").unwrap(), 1,
        ).unwrap(),
        amount: None, expected_close_date: None,
        probability: crm_core_contracts::BasisPoints::try_new(1000).unwrap(),
        occurred_at_unix_nanos: 100,
    }) { Ok(x) => x, Err(_) => return 31 };
    if deal.version() != 1 || deal.name() != "WASM Deal" { return 32; }
    let bytes = match crm_sales::encode_deal_state(&deal) {
        Ok(x) => x, Err(_) => return 33,
    };
    if crm_sales::decode_deal_state(&bytes).ok() != Some(deal.clone()) { return 34; }
    if crm_sales::DealStage::try_new(
        crm_sales::PipelineId::try_new("main-pipeline").unwrap(),
        crm_sales::StageId::try_new("invalid").unwrap(), 0,
    ).is_ok() { return 35; }

    // crm.activities: original task completion and optimistic version.
    let mut task = match crm_activities::Task::create(crm_activities::CreateTask {
        task_id: crm_module_sdk::RecordId::try_new("wasm-task-01").unwrap(),
        subject: "WASM task".into(), description: None,
        owner: crm_activities::TaskOwner::Team(crm_activities::TeamId::try_new("team-01").unwrap()),
        related_resources: vec![], priority: crm_activities::TaskPriority::Normal,
        due_at_unix_nanos: None, reminder_at_unix_nanos: None,
        occurred_at_unix_nanos: 100,
    }) { Ok(x) => x, Err(_) => return 41 };
    if task.version() != 1 { return 42; }
    if task.complete(crm_activities::CompleteTask {
        expected_version: 1, occurred_at_unix_nanos: 101,
    }).is_err() { return 43; }
    if task.version() != 2 || task.status() != crm_activities::TaskStatus::Completed {
        return 44;
    }
    let bytes = match crm_activities::encode_task_state(&task) {
        Ok(x) => x, Err(_) => return 45,
    };
    if crm_activities::decode_task_state(&bytes).ok() != Some(task.clone()) { return 46; }
    if task.complete(crm_activities::CompleteTask {
        expected_version: 1, occurred_at_unix_nanos: 102,
    }).is_ok() { return 47; }
    0
}

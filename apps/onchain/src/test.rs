use super::*;
use soroban_sdk::{testutils::Address as _, vec, Address, Env};

#[test]
fn test_create_and_get_escrow() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VaultixEscrow, ());
    let client = VaultixEscrowClient::new(&env, &contract_id);

    let depositor = Address::generate(&env);
    let recipient = Address::generate(&env);
    let escrow_id = 1u64;

    // Create milestones
    let milestones = vec![
        &env,
        Milestone {
            amount: 3000,
            status: MilestoneStatus::Pending,
            description: symbol_short!("Design"),
        },
        Milestone {
            amount: 3000,
            status: MilestoneStatus::Pending,
            description: symbol_short!("Dev"),
        },
        Milestone {
            amount: 4000,
            status: MilestoneStatus::Pending,
            description: symbol_short!("Deploy"),
        },
    ];

    // Create escrow
    client.create_escrow(&escrow_id, &depositor, &recipient, &milestones);

    // Retrieve escrow
    let escrow = client.get_escrow(&escrow_id);
    assert_eq!(escrow.depositor, depositor);
    assert_eq!(escrow.recipient, recipient);
    assert_eq!(escrow.total_amount, 10000);
    assert_eq!(escrow.total_released, 0);
    assert_eq!(escrow.status, EscrowStatus::Active);
    assert_eq!(escrow.milestones.len(), 3);
}

#[test]
fn test_release_milestone() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VaultixEscrow, ());
    let client = VaultixEscrowClient::new(&env, &contract_id);

    let depositor = Address::generate(&env);
    let recipient = Address::generate(&env);
    let escrow_id = 2u64;

    let milestones = vec![
        &env,
        Milestone {
            amount: 5000,
            status: MilestoneStatus::Pending,
            description: symbol_short!("Phase1"),
        },
        Milestone {
            amount: 5000,
            status: MilestoneStatus::Pending,
            description: symbol_short!("Phase2"),
        },
    ];

    client.create_escrow(&escrow_id, &depositor, &recipient, &milestones);

    // Release first milestone
    client.release_milestone(&escrow_id, &0);

    let escrow = client.get_escrow(&escrow_id);
    assert_eq!(escrow.total_released, 5000);
    assert_eq!(
        escrow.milestones.get(0).unwrap().status,
        MilestoneStatus::Released
    );
    assert_eq!(
        escrow.milestones.get(1).unwrap().status,
        MilestoneStatus::Pending
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #9)")]
fn test_dispute_blocks_release() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VaultixEscrow, ());
    let client = VaultixEscrowClient::new(&env, &contract_id);

    let depositor = Address::generate(&env);
    let recipient = Address::generate(&env);
    let escrow_id = 9u64;

    let milestones = vec![
        &env,
        Milestone {
            amount: 500,
            status: MilestoneStatus::Pending,
            description: symbol_short!("Task"),
        },
    ];

    client.create_escrow(&escrow_id, &depositor, &recipient, &milestones);

    // Either party can raise dispute; use depositor as caller.
    client.raise_dispute(&escrow_id, &depositor);

    let escrow = client.get_escrow(&escrow_id);
    assert_eq!(escrow.status, EscrowStatus::Disputed);

    // This should panic with Error #9 (EscrowNotActive)
    client.release_milestone(&escrow_id, &0);
}

#[test]
fn test_complete_escrow() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VaultixEscrow, ());
    let client = VaultixEscrowClient::new(&env, &contract_id);

    let depositor = Address::generate(&env);
    let recipient = Address::generate(&env);
    let escrow_id = 3u64;

    let milestones = vec![
        &env,
        Milestone {
            amount: 5000,
            status: MilestoneStatus::Pending,
            description: symbol_short!("Task1"),
        },
        Milestone {
            amount: 5000,
            status: MilestoneStatus::Pending,
            description: symbol_short!("Task2"),
        },
    ];

    client.create_escrow(&escrow_id, &depositor, &recipient, &milestones);

    // Release all milestones
    client.release_milestone(&escrow_id, &0);
    client.release_milestone(&escrow_id, &1);

    // Complete the escrow
    client.complete_escrow(&escrow_id);

    let escrow = client.get_escrow(&escrow_id);
    assert_eq!(escrow.status, EscrowStatus::Completed);
    assert_eq!(escrow.total_released, 10000);
}

#[test]
fn test_cancel_escrow() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VaultixEscrow, ());
    let client = VaultixEscrowClient::new(&env, &contract_id);

    let depositor = Address::generate(&env);
    let recipient = Address::generate(&env);
    let escrow_id = 4u64;

    let milestones = vec![
        &env,
        Milestone {
            amount: 10000,
            status: MilestoneStatus::Pending,
            description: symbol_short!("Work"),
        },
    ];

    client.create_escrow(&escrow_id, &depositor, &recipient, &milestones);

    // Cancel before any releases
    client.cancel_escrow(&escrow_id);

    let escrow = client.get_escrow(&escrow_id);
    assert_eq!(escrow.status, EscrowStatus::Cancelled);
}

#[test]
fn test_admin_resolves_dispute_to_recipient() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VaultixEscrow, ());
    let client = VaultixEscrowClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let recipient = Address::generate(&env);
    let escrow_id = 10u64;

    client.init(&admin);

    let milestones = vec![
        &env,
        Milestone {
            amount: 4000,
            status: MilestoneStatus::Pending,
            description: symbol_short!("Phase1"),
        },
        Milestone {
            amount: 6000,
            status: MilestoneStatus::Pending,
            description: symbol_short!("Phase2"),
        },
    ];

    client.create_escrow(&escrow_id, &depositor, &recipient, &milestones);

    // Raise dispute mid-project
    client.raise_dispute(&escrow_id, &recipient);

    // Admin resolves in favor of recipient (force payout)
    client.resolve_dispute(&escrow_id, &recipient);

    let escrow = client.get_escrow(&escrow_id);
    assert_eq!(escrow.status, EscrowStatus::Resolved);
    assert_eq!(escrow.resolution, Resolution::Recipient);
    assert_eq!(escrow.total_released, escrow.total_amount);
    assert!(escrow
        .milestones
        .iter()
        .all(|m| m.status == MilestoneStatus::Released));
}

#[test]
fn test_admin_resolves_dispute_to_depositor() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VaultixEscrow, ());
    let client = VaultixEscrowClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let recipient = Address::generate(&env);
    let escrow_id = 11u64;

    client.init(&admin);

    let milestones = vec![
        &env,
        Milestone {
            amount: 2000,
            status: MilestoneStatus::Pending,
            description: symbol_short!("Alpha"),
        },
        Milestone {
            amount: 3000,
            status: MilestoneStatus::Pending,
            description: symbol_short!("Beta"),
        },
    ];

    client.create_escrow(&escrow_id, &depositor, &recipient, &milestones);

    // Raise dispute as depositor
    client.raise_dispute(&escrow_id, &depositor);

    // Admin rules in favor of depositor (refund remaining funds)
    client.resolve_dispute(&escrow_id, &depositor);

    let escrow = client.get_escrow(&escrow_id);
    assert_eq!(escrow.status, EscrowStatus::Resolved);
    assert_eq!(escrow.resolution, Resolution::Depositor);
    // No additional releases occurred
    assert_eq!(escrow.total_released, 0);
    assert!(escrow
        .milestones
        .iter()
        .all(|m| m.status == MilestoneStatus::Disputed));
}

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_duplicate_escrow_id() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VaultixEscrow, ());
    let client = VaultixEscrowClient::new(&env, &contract_id);

    let depositor = Address::generate(&env);
    let recipient = Address::generate(&env);
    let escrow_id = 5u64;

    let milestones = vec![
        &env,
        Milestone {
            amount: 1000,
            status: MilestoneStatus::Pending,
            description: symbol_short!("Test"),
        },
    ];

    client.create_escrow(&escrow_id, &depositor, &recipient, &milestones);
    // This should panic with Error #2 (EscrowAlreadyExists)
    client.create_escrow(&escrow_id, &depositor, &recipient, &milestones);
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn test_double_release() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VaultixEscrow, ());
    let client = VaultixEscrowClient::new(&env, &contract_id);

    let depositor = Address::generate(&env);
    let recipient = Address::generate(&env);
    let escrow_id = 6u64;

    let milestones = vec![
        &env,
        Milestone {
            amount: 1000,
            status: MilestoneStatus::Pending,
            description: symbol_short!("Task"),
        },
    ];

    client.create_escrow(&escrow_id, &depositor, &recipient, &milestones);
    client.release_milestone(&escrow_id, &0);
    // This should panic with Error #4 (MilestoneAlreadyReleased)
    client.release_milestone(&escrow_id, &0);
}

#[test]
#[should_panic(expected = "Error(Contract, #10)")]
fn test_too_many_milestones() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VaultixEscrow, ());
    let client = VaultixEscrowClient::new(&env, &contract_id);

    let depositor = Address::generate(&env);
    let recipient = Address::generate(&env);
    let escrow_id = 7u64;

    // Create 21 milestones (exceeds max of 20)
    let mut milestones = Vec::new(&env);
    for _i in 0..21 {
        milestones.push_back(Milestone {
            amount: 100,
            status: MilestoneStatus::Pending,
            description: symbol_short!("Task"),
        });
    }

    // This should panic with Error #10 (VectorTooLarge)
    client.create_escrow(&escrow_id, &depositor, &recipient, &milestones);
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_invalid_milestone_amount() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VaultixEscrow, ());
    let client = VaultixEscrowClient::new(&env, &contract_id);

    let depositor = Address::generate(&env);
    let recipient = Address::generate(&env);
    let escrow_id = 8u64;

    let milestones = vec![
        &env,
        Milestone {
            amount: 0, // Invalid: zero amount
            status: MilestoneStatus::Pending,
            description: symbol_short!("Task"),
        },
    ];

    // This should panic with Error #6 (InvalidMilestoneAmount)
    client.create_escrow(&escrow_id, &depositor, &recipient, &milestones);
}

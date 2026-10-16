#![cfg(test)]

//! Regression and unit tests for issues #21, #22, #23, and #24.
//!
//! - #21: Extend contract instance storage TTL on core read and check-in operations.
//! - #22: Bounded storage refactoring for total_locked_by_token in ProtocolStats (discrete storage keys).
//! - #23: Storage lifetime management and TTL extension for ArchivedWill entries.
//! - #24: Bounded chunked recomputation of protocol stats across 100+ wills.

use soroban_sdk::{
    testutils::{
        storage::{Instance as _, Persistent as _},
        Address as _, Ledger,
    },
    token::StellarAssetClient,
    vec, Address, Env,
};

use crate::storage::{
    self, bump_instance_ttl, get_total_locked, recompute_protocol_stats_chunk, DataKey,
    BUMP_AMOUNT, DAY_IN_LEDGERS, LIFETIME_THRESHOLD,
};
use crate::types::{Allocation, Beneficiary, WillStatus};
use crate::{WillContract, WillContractClient};

const DAY: u64 = 86_400;

fn setup<'a>() -> (Env, WillContractClient<'a>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_700_000_000);

    let owner = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(owner.clone());
    let token_address = sac.address();
    StellarAssetClient::new(&env, &token_address).mint(&owner, &1_000_000_000);

    let contract_id = env.register(WillContract, ());
    let client = WillContractClient::new(&env, &contract_id);

    (env, client, owner, token_address)
}

fn advance_days(env: &Env, days: u64) {
    env.ledger().with_mut(|l| l.timestamp += days * DAY);
}

// ---------------------------------------------------------------------------
// Issue #21: Contract instance storage TTL renewal on check_in and lifecycle
// ---------------------------------------------------------------------------

#[test]
fn test_instance_ttl_extended_on_check_in() {
    let (env, client, owner, token_address) = setup();
    let beneficiary = Address::generate(&env);

    let will_id = client.create_will(
        &owner,
        &vec![&env, (token_address.clone(), 100_000_i128)],
        &vec![
            &env,
            Beneficiary {
                address: beneficiary,
                allocation: Allocation::Percentage(10_000),
            },
        ],
        &90,
        &7,
        &vec![&env],
        &0,
        &None,
        &0,
    );

    // Initial instance TTL after will creation
    let initial_ttl = env.as_contract(&client.address, || env.storage().instance().get_ttl());
    assert_eq!(initial_ttl, BUMP_AMOUNT);

    // Age the ledger past 40 days (LIFETIME_THRESHOLD is 30 days) so TTL drops below threshold
    env.ledger().with_mut(|l| {
        l.sequence_number += DAY_IN_LEDGERS * 40;
    });

    let reduced_ttl = env.as_contract(&client.address, || env.storage().instance().get_ttl());
    assert!(
        reduced_ttl < LIFETIME_THRESHOLD,
        "Advancing sequence number past 40 days must drop instance TTL below threshold: {reduced_ttl}"
    );

    // Calling check_in must refresh instance TTL to BUMP_AMOUNT
    client.check_in(&will_id, &owner);

    let bumped_ttl = env.as_contract(&client.address, || env.storage().instance().get_ttl());
    assert!(
        bumped_ttl > reduced_ttl,
        "check_in must extend the contract instance storage TTL"
    );
    assert_eq!(
        bumped_ttl, BUMP_AMOUNT,
        "bumped instance TTL must reach BUMP_AMOUNT"
    );
}

#[test]
fn test_bump_instance_ttl_helper() {
    let (env, client, _, _) = setup();
    env.as_contract(&client.address, || {
        let initial_ttl = env.storage().instance().get_ttl();
        assert!(initial_ttl < BUMP_AMOUNT);

        bump_instance_ttl(&env);
        let final_ttl = env.storage().instance().get_ttl();
        assert_eq!(final_ttl, BUMP_AMOUNT);
        assert!(final_ttl > initial_ttl);
    });
}

// ---------------------------------------------------------------------------
// Issue #22: Discrete storage keys for total_locked_by_token (constant gas)
// ---------------------------------------------------------------------------

#[test]
fn test_discrete_storage_keys_and_constant_gas_across_50_tokens() {
    let (env, client, _, _) = setup();

    // Verify per-token locked totals are stored under discrete persistent keys DataKey::TotalLocked
    env.as_contract(&client.address, || {
        let mut tokens: std::vec::Vec<Address> = std::vec::Vec::new();

        // Generate 50 distinct tokens
        for _ in 0..50 {
            tokens.push(Address::generate(&env));
        }

        // 1. Add balance to the first token
        let token_0 = &tokens[0];
        storage::adjust_locked_value(&env, token_0, 1_000);

        // Verify discrete persistent key has value
        assert_eq!(get_total_locked(&env, token_0), 1_000);
        assert!(env.storage().persistent().has(&DataKey::TotalLocked(token_0.clone())));

        // Profile CPU cost of updating an existing token when N = 1
        env.budget().reset_default();
        let cpu_before_1 = env.budget().cpu_instruction_cost();
        storage::adjust_locked_value(&env, token_0, 500);
        let cpu_after_1 = env.budget().cpu_instruction_cost();
        let cost_at_1 = cpu_after_1 - cpu_before_1;
        assert!(cost_at_1 < 100_000, "Cost at N=1 must be well within transaction budget: {cost_at_1}");

        assert_eq!(get_total_locked(&env, token_0), 1_500);

        // 2. Add 49 other distinct tokens to the protocol state (total 50 tokens)
        for t in tokens.iter().skip(1) {
            storage::adjust_locked_value(&env, t, 10_000);
            assert!(env.storage().persistent().has(&DataKey::TotalLocked(t.clone())));
        }

        // Verify get_protocol_stats reflects all 50 tokens
        let stats_50 = storage::get_protocol_stats(&env);
        assert_eq!(stats_50.total_locked_by_token.len(), 50);

        // Profile CPU cost of updating distinct tokens across the protocol when N = 50
        // (verifies constant gas and no positional vector-scan dependency)
        env.budget().reset_default();
        let cpu_before_first = env.budget().cpu_instruction_cost();
        storage::adjust_locked_value(&env, token_0, 500);
        let cpu_after_first = env.budget().cpu_instruction_cost();
        let cost_first = cpu_after_first - cpu_before_first;

        env.budget().reset_default();
        let cpu_before_middle = env.budget().cpu_instruction_cost();
        storage::adjust_locked_value(&env, &tokens[25], 500);
        let cpu_after_middle = env.budget().cpu_instruction_cost();
        let cost_middle = cpu_after_middle - cpu_before_middle;

        env.budget().reset_default();
        let cpu_before_last = env.budget().cpu_instruction_cost();
        storage::adjust_locked_value(&env, &tokens[49], 500);
        let cpu_after_last = env.budget().cpu_instruction_cost();
        let cost_last = cpu_after_last - cpu_before_last;

        // Balance updates across 50 distinct tokens must be bounded and independent of token position
        assert!(cost_first < 100_000, "Cost must be well within transaction budget: {cost_first}");
        assert!(cost_middle < 100_000, "Cost must be well within transaction budget: {cost_middle}");
        assert!(cost_last < 100_000, "Cost must be well within transaction budget: {cost_last}");

        assert_eq!(get_total_locked(&env, token_0), 2_000);

        // 3. Decrementing balance to 0 removes the discrete storage entry
        storage::adjust_locked_value(&env, token_0, -2_000);
        assert_eq!(get_total_locked(&env, token_0), 0);
        assert!(!env.storage().persistent().has(&DataKey::TotalLocked(token_0.clone())));

        let stats_after_zero = storage::get_protocol_stats(&env);
        assert_eq!(stats_after_zero.total_locked_by_token.len(), 49);
    });
}

// ---------------------------------------------------------------------------
// Issue #23: Storage lifetime management and TTL extension for ArchivedWill
// ---------------------------------------------------------------------------

#[test]
fn test_archived_will_storage_lifecycle_and_retrieval() {
    let (env, client, owner, token_address) = setup();
    let beneficiary = Address::generate(&env);

    let will_id = client.create_will(
        &owner,
        &vec![&env, (token_address.clone(), 500_000_i128)],
        &vec![
            &env,
            Beneficiary {
                address: beneficiary,
                allocation: Allocation::Percentage(10_000),
            },
        ],
        &90,
        &7,
        &vec![&env],
        &0,
        &None,
        &0,
    );

    advance_days(&env, 91);
    client.trigger_will(&will_id);
    advance_days(&env, 8);
    client.release_inheritance(&will_id, &None);

    // Archive the will
    client.archive_will(&will_id);

    // Normal get_will returns error since will was moved from active storage
    assert!(client.try_get_will(&will_id).is_err());

    // 1. Query archived will via get_archived_will
    let archived = client.get_archived_will(&will_id);
    assert_eq!(archived.id, will_id);
    assert_eq!(archived.owner, owner);
    assert_eq!(archived.status, WillStatus::Released);

    // 2. Verify persistent storage TTL was set on DataKey::ArchivedWill
    let initial_ttl = env.as_contract(&client.address, || {
        env.storage()
            .persistent()
            .get_ttl(&DataKey::ArchivedWill(will_id))
    });
    assert_eq!(
        initial_ttl, BUMP_AMOUNT,
        "Archived will must have persistent storage TTL set to BUMP_AMOUNT"
    );

    // Advance sequence number past 40 days to drop TTL below LIFETIME_THRESHOLD
    env.ledger().with_mut(|l| {
        l.sequence_number += DAY_IN_LEDGERS * 40;
    });

    let reduced_ttl = env.as_contract(&client.address, || {
        env.storage()
            .persistent()
            .get_ttl(&DataKey::ArchivedWill(will_id))
    });
    assert!(reduced_ttl < LIFETIME_THRESHOLD);

    // 3. Extend archived will TTL via entrypoint
    client.extend_archived_will_ttl(&will_id);

    let extended_ttl = env.as_contract(&client.address, || {
        env.storage()
            .persistent()
            .get_ttl(&DataKey::ArchivedWill(will_id))
    });
    assert_eq!(
        extended_ttl, BUMP_AMOUNT,
        "extend_archived_will_ttl must extend persistent storage TTL back to BUMP_AMOUNT"
    );

    // 4. Non-existent will returns error
    assert!(client.try_get_archived_will(&999_999).is_err());
    assert!(client.try_extend_archived_will_ttl(&999_999).is_err());
}

// ---------------------------------------------------------------------------
// Issue #24: Chunked recomputation of protocol stats with 100+ wills
// ---------------------------------------------------------------------------

#[test]
fn test_recompute_protocol_stats_chunked_with_100_plus_wills() {
    let (env, client, _, token_address) = setup();

    // Create 105 wills to test past 100, using distinct owners to remain within
    // MAX_WILLS_PER_INDEX (capped at 10 under cfg(test))
    let total_wills = 105_u64;
    for _ in 1..=total_wills {
        let will_owner = Address::generate(&env);
        StellarAssetClient::new(&env, &token_address).mint(&will_owner, &10_000);
        let beneficiary = Address::generate(&env);
        client.create_will(
            &will_owner,
            &vec![&env, (token_address.clone(), 1_000_i128)],
            &vec![
                &env,
                Beneficiary {
                    address: beneficiary,
                    allocation: Allocation::Percentage(10_000),
                },
            ],
            &90,
            &7,
            &vec![&env],
            &0,
            &None,
            &0,
        );
    }

    env.as_contract(&client.address, || {
        // Recompute using bounded chunks of size 20
        let batch_size = 20_u32;
        let mut start_id = 1_u64;
        let mut accumulated_active_wills = 0_u64;
        let mut total_locked_accumulated = 0_i128;

        loop {
            let (chunk_stats, next_id, is_complete) =
                recompute_protocol_stats_chunk(&env, start_id, batch_size);

            accumulated_active_wills += chunk_stats.active_will_count;
            for entry in chunk_stats.total_locked_by_token.iter() {
                if entry.token == token_address {
                    total_locked_accumulated += entry.total_locked;
                }
            }

            if is_complete {
                break;
            }
            assert!(next_id > start_id);
            start_id = next_id;
        }

        // Verify accumulated chunked result matches expected total
        assert_eq!(accumulated_active_wills, total_wills);
        assert_eq!(total_locked_accumulated, (total_wills as i128) * 1_000);

        // Verify full recompute matches chunked recompute
        let full_stats = storage::recompute_protocol_stats(&env);
        assert_eq!(full_stats.active_will_count, total_wills);
        let locked_in_full = full_stats
            .total_locked_by_token
            .iter()
            .find(|e| e.token == token_address)
            .map(|e| e.total_locked)
            .unwrap_or(0);
        assert_eq!(locked_in_full, (total_wills as i128) * 1_000);
    });
}

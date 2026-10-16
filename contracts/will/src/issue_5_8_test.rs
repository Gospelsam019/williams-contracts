#![cfg(test)]

//! Regression coverage for issues #5-#8:
//!
//! - **#5**: `cancel_will` decremented `total_locked_by_token` once via
//!   `storage::adjust_locked_for_balances` and then a second time via a
//!   redundant per-token loop, deducting every cancelled will's balance twice.
//! - **#6**: `release_inheritance` pre-decremented `total_locked_by_token`
//!   for every token before calling `distribute`, which performs its own
//!   decrement — double-deducting on every estate release.
//! - **#7**: `split_will` called `storage::increment_active_will_count`
//!   twice in a row, so each split moved the counter by +2 instead of +1.
//! - **#8**: `archive_will` only accepted `Released`/`Cancelled`, rejecting
//!   the `Settled` status `close_will` produces — trapping closed wills in
//!   active storage with no path to archival.

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    vec, Address, Env,
};

use crate::storage::DataKey;
use crate::{Allocation, Beneficiary, WillContract, WillContractClient, WillError, WillStatus};

const DAY: u64 = 86_400;

struct Harness<'a> {
    env: Env,
    client: WillContractClient<'a>,
    contract_id: Address,
    owner: Address,
    token_a: Address,
    token_b: Address,
}

fn setup<'a>() -> Harness<'a> {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_700_000_000);

    let owner = Address::generate(&env);
    let token_a = env
        .register_stellar_asset_contract_v2(owner.clone())
        .address();
    StellarAssetClient::new(&env, &token_a).mint(&owner, &1_000_000_000);
    let token_b = env
        .register_stellar_asset_contract_v2(owner.clone())
        .address();
    StellarAssetClient::new(&env, &token_b).mint(&owner, &1_000_000_000);

    let contract_id = env.register(WillContract, ());
    let client = WillContractClient::new(&env, &contract_id);

    Harness {
        env,
        client,
        contract_id,
        owner,
        token_a,
        token_b,
    }
}

fn single_percentage_beneficiary(
    env: &Env,
    beneficiary: &Address,
) -> soroban_sdk::Vec<Beneficiary> {
    vec![
        env,
        Beneficiary {
            address: beneficiary.clone(),
            allocation: Allocation::Percentage(10_000),
        },
    ]
}

/// The protocol's currently tracked locked total for `token`, or `0` when the
/// token has no entry at all (which is what `adjust_locked_value` produces
/// once a total returns to exactly zero).
fn locked_for(client: &WillContractClient, token: &Address) -> i128 {
    client
        .get_protocol_stats()
        .total_locked_by_token
        .iter()
        .find(|e| &e.token == token)
        .map(|e| e.total_locked)
        .unwrap_or(0)
}

fn create_will(h: &Harness, token: &Address, amount: i128, beneficiary: &Address) -> u64 {
    h.client.create_will(
        &h.owner,
        &vec![&h.env, (token.clone(), amount)],
        &single_percentage_beneficiary(&h.env, beneficiary),
        &90,
        &7,
        &vec![&h.env],
        &0,
        &None,
        &0,
    )
}

// ── Issue #5: cancel_will decrements locked totals exactly once ──────────────

/// Cancelling one will must deduct exactly that will's balance from
/// `total_locked_by_token`, leaving every other will's balance untouched.
/// The pre-fix code called both `adjust_locked_for_balances(…, -1)` and a
/// redundant per-token loop, deducting twice and driving the total to
/// `4_000 - 2 * 1_000 = 2_000` instead of `3_000`.
#[test]
fn cancel_will_decrements_locked_totals_exactly_once() {
    let h = setup();
    let beneficiary = Address::generate(&h.env);

    let will_a = create_will(&h, &h.token_a, 1_000, &beneficiary);
    let _will_b = create_will(&h, &h.token_a, 3_000, &beneficiary);
    assert_eq!(locked_for(&h.client, &h.token_a), 4_000);

    h.client.cancel_will(&will_a, &h.owner);

    assert_eq!(
        locked_for(&h.client, &h.token_a),
        3_000,
        "cancel_will must deduct each token's balance exactly once (#5)"
    );

    let audit = h.client.audit_protocol_stats();
    assert!(
        audit.consistent,
        "stored and recomputed stats must agree after cancel_will"
    );
}

// ── Issue #6: release_inheritance decrements locked totals exactly once ──────

/// Releasing a multi-token will must deduct each token's balance from
/// `total_locked_by_token` exactly once. The pre-fix code decremented in
/// `release_inheritance` and again inside `distribute`, producing negative
/// totals of `-5_000` / `-7_000` instead of dropping the entries.
#[test]
fn release_inheritance_decrements_locked_totals_exactly_once() {
    let h = setup();
    let beneficiary = Address::generate(&h.env);

    let will_id = h.client.create_will(
        &h.owner,
        &vec![
            &h.env,
            (h.token_a.clone(), 5_000_i128),
            (h.token_b.clone(), 7_000_i128),
        ],
        &single_percentage_beneficiary(&h.env, &beneficiary),
        &90,
        &7,
        &vec![&h.env],
        &0,
        &None,
        &0,
    );
    assert_eq!(locked_for(&h.client, &h.token_a), 5_000);
    assert_eq!(locked_for(&h.client, &h.token_b), 7_000);

    h.env.ledger().with_mut(|l| l.timestamp += 91 * DAY);
    h.client.trigger_will(&will_id);
    h.env.ledger().with_mut(|l| l.timestamp += 8 * DAY);
    h.client.release_inheritance(&will_id, &None);

    assert_eq!(
        locked_for(&h.client, &h.token_a),
        0,
        "release must deduct token_a exactly once (#6)"
    );
    assert_eq!(
        locked_for(&h.client, &h.token_b),
        0,
        "release must deduct token_b exactly once (#6)"
    );
    assert_eq!(h.client.get_protocol_stats().active_will_count, 0);

    let audit = h.client.audit_protocol_stats();
    assert!(
        audit.consistent,
        "stored and recomputed stats must agree after release_inheritance"
    );
}

// ── Issue #7: split_will increments the active-will counter exactly once ─────

#[test]
fn split_will_increments_active_will_count_by_one() {
    let h = setup();
    let beneficiary_a = Address::generate(&h.env);
    let beneficiary_b = Address::generate(&h.env);

    let source_id = h.client.create_will(
        &h.owner,
        &vec![&h.env, (h.token_a.clone(), 1_000_000_i128)],
        &vec![
            &h.env,
            Beneficiary {
                address: beneficiary_a.clone(),
                allocation: Allocation::Percentage(6_000),
            },
            Beneficiary {
                address: beneficiary_b.clone(),
                allocation: Allocation::Percentage(4_000),
            },
        ],
        &90,
        &7,
        &vec![&h.env],
        &0,
        &None,
        &0,
    );
    assert_eq!(h.client.get_protocol_stats().active_will_count, 1);

    let child_id = h.client.split_will(
        &source_id,
        &h.owner,
        &vec![
            &h.env,
            Beneficiary {
                address: beneficiary_a,
                allocation: Allocation::Percentage(6_000),
            },
        ],
        &vec![&h.env, (h.token_a.clone(), 250_000_i128)],
    );

    assert_eq!(
        h.client.get_protocol_stats().active_will_count,
        2,
        "split_will must net exactly +1 active will for the child (#7)"
    );
    assert_eq!(h.client.get_will(&child_id).status, WillStatus::Active);
}

// ── Issue #8: archive_will accepts Settled wills ─────────────────────────────

/// Full lifecycle: create → trigger → release → close → archive. Before the
/// fix the final step panicked with `WillError::WillNotSettled`, so a closed
/// (`Settled`) will could never leave active storage.
#[test]
fn archive_will_archives_a_settled_will() {
    let h = setup();
    let beneficiary = Address::generate(&h.env);

    let will_id = create_will(&h, &h.token_a, 1_000_000, &beneficiary);
    h.env.ledger().with_mut(|l| l.timestamp += 91 * DAY);
    h.client.trigger_will(&will_id);
    h.env.ledger().with_mut(|l| l.timestamp += 8 * DAY);
    h.client.release_inheritance(&will_id, &None);

    h.client.close_will(&will_id, &h.owner);
    assert_eq!(h.client.get_will(&will_id).status, WillStatus::Settled);

    h.client.archive_will(&will_id);

    // The active entry must be gone…
    let result = h.client.try_get_will(&will_id);
    assert!(
        result.is_err(),
        "an archived settled will's active storage entry must be gone"
    );

    // …and the will must have been moved to archival storage rather than
    // simply dropped, so the rent-paying state is reclaimed from the active
    // keyspace (#8).
    let archived_exists = h.env.as_contract(&h.contract_id, || {
        h.env
            .storage()
            .persistent()
            .has(&DataKey::ArchivedWill(will_id))
    });
    assert!(
        archived_exists,
        "a settled will must be moved to archival storage (#8)"
    );
}

/// Non-terminal statuses must still be rejected with the same error.
#[test]
fn archive_will_rejects_a_non_terminal_will() {
    let h = setup();
    let beneficiary = Address::generate(&h.env);

    let will_id = create_will(&h, &h.token_a, 1_000, &beneficiary);

    let result = h.client.try_archive_will(&will_id);
    assert_eq!(result, Err(Ok(WillError::WillNotSettled.into())));
}

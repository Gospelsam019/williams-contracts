#![cfg(test)]

//! Regression tests for issue #10: `distribute()` double-counted the remainder
//! of a fixed-amount will's primary-token balance, first giving it to the last
//! beneficiary via `shares` and then also queuing it for an owner refund via
//! `refund_plan`. The result was a `DistributionMismatch` panic (or, if the
//! check were absent, a double spend) whenever a fixed-amount will held more
//! primary-token balance than the sum of its fixed allocations.
//!
//! The remainder is now queued exactly once — as an owner refund, matching the
//! canonical "Leftover value" behavior documented on `distribute` (#383): a
//! fixed amount expresses the testator's exact intent, so any excess belongs
//! back with the testator, not with the last beneficiary.

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::{Client as TokenClient, StellarAssetClient},
    vec, Address, Env,
};

use crate::{Allocation, Beneficiary, WillContract, WillContractClient, WillStatus};

const DAY: u64 = 86_400;

fn setup<'a>() -> (
    Env,
    WillContractClient<'a>,
    Address,
    TokenClient<'a>,
    Address,
    Address, // contract_id
) {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_700_000_000);

    let owner = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(owner.clone());
    let token_address = sac.address();
    StellarAssetClient::new(&env, &token_address).mint(&owner, &1_000_000_000);

    let contract_id = env.register(WillContract, ());
    let client = WillContractClient::new(&env, &contract_id);

    (
        env.clone(),
        client,
        owner,
        TokenClient::new(&env, &token_address),
        token_address,
        contract_id,
    )
}

fn advance(env: &Env, days: u64) {
    env.ledger().with_mut(|l| l.timestamp += days * DAY);
}

fn trigger_and_release(env: &Env, client: &WillContractClient, will_id: u64) {
    advance(env, 91);
    client.trigger_will(&will_id);
    advance(env, 8);
    client.release_inheritance(&will_id, &None);
}

/// Issue #10: releasing a fixed-amount will whose balance exceeds the sum of
/// its fixed allocations must not panic with `DistributionMismatch`.
///
/// Will balance: 1_000 tokens.
/// Fixed allocations: beneficiary_a = 300, beneficiary_b = 400 (total 700).
/// Remainder: 300 tokens refunded to the owner exactly once (#383), not paid
/// to the last beneficiary and never queued twice (#10).
#[test]
fn fixed_amount_will_with_excess_balance_releases_without_panic() {
    let (env, client, owner, token, token_address, contract_id) = setup();

    let beneficiary_a = Address::generate(&env);
    let beneficiary_b = Address::generate(&env);

    let balance: i128 = 1_000;
    let tokens = vec![&env, (token_address.clone(), balance)];

    let will_id = client.create_will(
        &owner,
        &tokens,
        &vec![
            &env,
            Beneficiary {
                address: beneficiary_a.clone(),
                allocation: Allocation::FixedAmount(300),
            },
            Beneficiary {
                address: beneficiary_b.clone(),
                allocation: Allocation::FixedAmount(400),
            },
        ],
        &90,
        &7,
        &vec![&env],
        &0,
        &None,
        &0,
    );

    // Should not panic.
    trigger_and_release(&env, &client, will_id);

    let will = client.get_will(&will_id);
    assert_eq!(will.status, WillStatus::Released);

    // beneficiary_a receives exactly 300.
    assert_eq!(token.balance(&beneficiary_a), 300);
    // beneficiary_b receives exactly 400 — the remainder is NOT dumped on the
    // last beneficiary (#10); fixed amounts are the testator's exact intent.
    assert_eq!(token.balance(&beneficiary_b), 400);
    // Owner receives the 300-token remainder back (minted 1_000_000_000,
    // deposited 1_000, refunded 300).
    assert_eq!(token.balance(&owner), 1_000_000_000 - balance + 300);
    // Contract holds nothing after release.
    assert_eq!(token.balance(&contract_id), 0);
}

/// Issue #10: the conservation invariant holds — total tokens transferred must
/// equal the will's original balance, with no double-counting.
#[test]
fn fixed_amount_will_with_top_up_conserves_balance() {
    let (env, client, owner, token, token_address, contract_id) = setup();

    let beneficiary_a = Address::generate(&env);

    let initial_balance: i128 = 700;
    let top_up_amount: i128 = 300;
    let tokens = vec![&env, (token_address.clone(), initial_balance)];

    let will_id = client.create_will(
        &owner,
        &tokens,
        &vec![
            &env,
            Beneficiary {
                address: beneficiary_a.clone(),
                allocation: Allocation::FixedAmount(700),
            },
        ],
        &90,
        &7,
        &vec![&env],
        &0,
        &None,
        &0,
    );

    // Top up to introduce a remainder.
    client.top_up(&will_id, &owner, &token_address, &top_up_amount);

    trigger_and_release(&env, &client, will_id);

    let will = client.get_will(&will_id);
    assert_eq!(will.status, WillStatus::Released);

    // beneficiary_a receives exactly the fixed allocation — the top-up
    // remainder is refunded to the owner rather than paid to the beneficiary.
    assert_eq!(token.balance(&beneficiary_a), initial_balance);
    // Owner: minted 1_000_000_000, deposited 700 + topped up 300, refunded 300.
    assert_eq!(token.balance(&owner), 1_000_000_000 - initial_balance);
    assert_eq!(token.balance(&contract_id), 0);
}

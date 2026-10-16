#![cfg(test)]

//! Regression tests for issue #11: `distribute()` panicked with
//! `DistributionMismatch` for fixed-amount wills that held secondary tokens
//! (tokens other than the primary token).
//!
//! Fixed-amount allocations are denominated in the will's primary token only
//! (#384). For any secondary token, `shares` is empty after the fixed-amount
//! loop. Before the fix, the conservation check (`planned == total`) ran while
//! `planned` was still 0 and `total` equalled the non-zero secondary-token
//! balance, causing an immediate panic before the owner-refund path could run.
//!
//! The fix moves the owner-refund queuing to happen first (before the
//! conservation check) for the secondary-token (empty-shares) case, so
//! `planned` includes the refund amount when the check runs. Primary-token
//! remainders follow the same single owner-refund path (#10, #383).

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::{Client as TokenClient, StellarAssetClient},
    vec, Address, Env,
};

use crate::{Allocation, Beneficiary, WillContract, WillContractClient, WillStatus};

const DAY: u64 = 86_400;

fn setup_two_tokens<'a>() -> (
    Env,
    WillContractClient<'a>,
    Address,
    TokenClient<'a>,
    Address,
    TokenClient<'a>,
    Address,
    Address, // contract_id
) {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_700_000_000);

    let owner = Address::generate(&env);

    let sac_a = env.register_stellar_asset_contract_v2(owner.clone());
    let token_a_address = sac_a.address();
    StellarAssetClient::new(&env, &token_a_address).mint(&owner, &1_000_000_000);

    let sac_b = env.register_stellar_asset_contract_v2(owner.clone());
    let token_b_address = sac_b.address();
    StellarAssetClient::new(&env, &token_b_address).mint(&owner, &1_000_000_000);

    let contract_id = env.register(WillContract, ());
    let client = WillContractClient::new(&env, &contract_id);

    (
        env.clone(),
        client,
        owner,
        TokenClient::new(&env, &token_a_address),
        token_a_address,
        TokenClient::new(&env, &token_b_address),
        token_b_address,
        contract_id,
    )
}

fn advance(env: &Env, days: u64) {
    env.ledger().with_mut(|l| l.timestamp += days * DAY);
}

/// Issue #11: a fixed-amount will that holds a secondary token must release
/// successfully, refunding the secondary token balance to the owner.
///
/// Token A: primary. Beneficiary gets FixedAmount(500).
/// Token B: secondary (added via `top_up`). No allocation — must be refunded.
#[test]
fn fixed_amount_will_with_secondary_token_releases_and_refunds_owner() {
    let (env, client, owner, token_a, token_a_addr, token_b, token_b_addr, contract_id) =
        setup_two_tokens();

    let beneficiary = Address::generate(&env);

    let primary_balance: i128 = 500;
    let secondary_balance: i128 = 800;

    // Create the will with token A as the primary token.
    let will_id = client.create_will(
        &owner,
        &vec![&env, (token_a_addr.clone(), primary_balance)],
        &vec![
            &env,
            Beneficiary {
                address: beneficiary.clone(),
                allocation: Allocation::FixedAmount(primary_balance),
            },
        ],
        &90,
        &7,
        &vec![&env],
        &0,
        &None,
        &0,
    );

    // Top up with secondary token B.
    client.top_up(&will_id, &owner, &token_b_addr, &secondary_balance);

    // Trigger and release — must not panic (#11).
    advance(&env, 91);
    client.trigger_will(&will_id);
    advance(&env, 8);
    client.release_inheritance(&will_id, &None);

    let will = client.get_will(&will_id);
    assert_eq!(will.status, WillStatus::Released, "will must be Released");

    // Beneficiary receives the fixed primary-token allocation.
    assert_eq!(
        token_a.balance(&beneficiary),
        primary_balance,
        "beneficiary must receive their fixed allocation of token A"
    );

    // Owner receives the secondary-token balance refunded in full.
    // Initial mint was 1_000_000_000; locked secondary_balance; got it back.
    assert_eq!(
        token_b.balance(&owner),
        1_000_000_000,
        "owner must receive the full secondary token balance as a refund"
    );

    // Contract holds nothing after release.
    assert_eq!(token_a.balance(&contract_id), 0);
    assert_eq!(token_b.balance(&contract_id), 0);
}

/// Issue #11: a fixed-amount will with an excess primary-token balance AND a
/// secondary token releases correctly. Both the excess primary balance and
/// the whole secondary-token balance are refunded to the owner (#383), each
/// exactly once (#10).
#[test]
fn fixed_amount_will_excess_primary_and_secondary_releases_correctly() {
    let (env, client, owner, token_a, token_a_addr, token_b, token_b_addr, contract_id) =
        setup_two_tokens();

    let beneficiary_a = Address::generate(&env);
    let beneficiary_b = Address::generate(&env);

    let primary_locked: i128 = 1_000;
    let fixed_a: i128 = 300;
    let fixed_b: i128 = 400;
    let secondary_balance: i128 = 250;

    let will_id = client.create_will(
        &owner,
        &vec![&env, (token_a_addr.clone(), primary_locked)],
        &vec![
            &env,
            Beneficiary {
                address: beneficiary_a.clone(),
                allocation: Allocation::FixedAmount(fixed_a),
            },
            Beneficiary {
                address: beneficiary_b.clone(),
                allocation: Allocation::FixedAmount(fixed_b),
            },
        ],
        &90,
        &7,
        &vec![&env],
        &0,
        &None,
        &0,
    );

    client.top_up(&will_id, &owner, &token_b_addr, &secondary_balance);

    advance(&env, 91);
    client.trigger_will(&will_id);
    advance(&env, 8);
    client.release_inheritance(&will_id, &None);

    let will = client.get_will(&will_id);
    assert_eq!(will.status, WillStatus::Released);

    // beneficiary_a receives exactly fixed_a.
    assert_eq!(token_a.balance(&beneficiary_a), fixed_a);
    // beneficiary_b receives exactly fixed_b — the 300-token primary
    // remainder goes to the owner, not to the last beneficiary (#10, #383).
    let remainder = primary_locked - fixed_a - fixed_b;
    assert_eq!(token_a.balance(&beneficiary_b), fixed_b);

    // Owner: token_a = minted 1_000_000_000 - deposited 1_000 + refunded 300;
    // token_b = minted 1_000_000_000 - deposited 250 + refunded 250.
    assert_eq!(
        token_a.balance(&owner),
        1_000_000_000 - primary_locked + remainder
    );
    assert_eq!(token_b.balance(&owner), 1_000_000_000);

    // Contract is empty.
    assert_eq!(token_a.balance(&contract_id), 0);
    assert_eq!(token_b.balance(&contract_id), 0);
}

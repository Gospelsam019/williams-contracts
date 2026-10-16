#![cfg(test)]

//! Regression tests for issue #12: `renounce_beneficiary` redistribution must
//! account for committed hashed-beneficiary basis points so that the surviving
//! visible percentage total never pushes the combined allocation above 10,000 bps.
//!
//! Fixed-amount allocations do not consume the percentage pool, so a will with
//! `FixedAmount` visible beneficiaries plus hashed percentage beneficiaries is
//! the reachable state where hashed_bps > 0. The redistribution `target_total`
//! cap must be `10_000 - hashed_bps`, not simply `10_000`, so that
//! `visible_bps + hashed_bps ≤ 10_000` is preserved after renunciation.
//!
//! A separate validation path: `assert_valid_percentages` is now called after
//! `assert_valid_allocations` at the end of `renounce_beneficiary`, ensuring the
//! combined constraint is always verified before the updated will is saved.

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::{Client as TokenClient, StellarAssetClient},
    vec, Address, Bytes, Env,
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

fn make_commitment(env: &Env, salt: u8) -> Bytes {
    let mut raw = [0u8; 32];
    raw[0] = salt;
    let b = Bytes::from_array(env, &raw);
    let digest = env.crypto().sha256(&b);
    Bytes::from_array(env, &digest.to_array())
}

/// Issue #12: renouncing a `FixedAmount` beneficiary on a will that also carries
/// hashed percentage beneficiaries succeeds without panicking.
///
/// Setup: two `FixedAmount` visible beneficiaries + one hashed beneficiary at
/// 2_000 bps. One visible beneficiary renounces. The hashed bps must not be
/// disturbed, and the surviving visible list must still pass
/// `assert_valid_percentages` (visible% = 0, hashed = 2_000 ≤ 10_000 ✓).
#[test]
fn renounce_fixed_amount_beneficiary_with_hashed_bps_succeeds() {
    let (env, client, owner, _token, token_address, _contract_id) = setup();

    let beneficiary_a = Address::generate(&env);
    let beneficiary_b = Address::generate(&env);

    let will_id = client.create_will(
        &owner,
        &vec![&env, (token_address.clone(), 1_000_000_i128)],
        &vec![
            &env,
            Beneficiary {
                address: beneficiary_a.clone(),
                allocation: Allocation::FixedAmount(300_000),
            },
            Beneficiary {
                address: beneficiary_b.clone(),
                allocation: Allocation::FixedAmount(300_000),
            },
        ],
        &90,
        &7,
        &vec![&env],
        &0,
        &None,
        &0,
    );

    // Add a hashed beneficiary that claims 2_000 bps of the percentage pool.
    let commitment = make_commitment(&env, 1);
    client.add_hashed_beneficiary(&will_id, &owner, &commitment, &2_000);

    // Renouncing beneficiary_a must succeed.
    client.renounce_beneficiary(&will_id, &beneficiary_a);

    let will = client.get_will(&will_id);
    assert_eq!(
        will.beneficiaries.len(),
        1,
        "one visible beneficiary remains"
    );
    assert_eq!(
        will.hashed_beneficiaries.len(),
        1,
        "hashed beneficiary must be untouched"
    );
    assert_eq!(
        will.hashed_beneficiaries.get_unchecked(0).percentage,
        2_000,
        "hashed beneficiary's 2_000 bps must be preserved (#12)"
    );
}

/// Issue #12: after a percentage renounce, `visible_bps + hashed_bps ≤ 10_000`
/// must hold on the surviving will.
///
/// Setup: three `Percentage` visible beneficiaries summing to 10_000 bps, no
/// hashed (hashed cannot be added on top of a full 10_000-bps visible list).
/// One renounces; the other two absorb the renounced share. The combined total
/// is still exactly 10_000 bps and `assert_valid_percentages` must pass.
///
/// This test confirms the redistribution logic works correctly in the standard
/// (no-hashed) case and that the updated `max_visible_bps` cap behaves
/// identically to the old `10_000` cap when `hashed_bps == 0`.
#[test]
fn renounce_percentage_beneficiary_no_hashed_still_sums_to_10000() {
    let (env, client, owner, _token, token_address, _contract_id) = setup();

    let a = Address::generate(&env);
    let b = Address::generate(&env);
    let c = Address::generate(&env);

    let will_id = client.create_will(
        &owner,
        &vec![&env, (token_address.clone(), 1_000_000_i128)],
        &vec![
            &env,
            Beneficiary {
                address: a.clone(),
                allocation: Allocation::Percentage(4_000),
            },
            Beneficiary {
                address: b.clone(),
                allocation: Allocation::Percentage(2_000),
            },
            Beneficiary {
                address: c.clone(),
                allocation: Allocation::Percentage(4_000),
            },
        ],
        &90,
        &7,
        &vec![&env],
        &0,
        &None,
        &0,
    );

    // B renounces their 2_000 bps; A and C absorb them.
    client.renounce_beneficiary(&will_id, &b);

    let will = client.get_will(&will_id);
    assert_eq!(
        will.beneficiaries.len(),
        2,
        "two visible beneficiaries remain"
    );

    let mut total_visible_bps: u32 = 0;
    for beneficiary in will.beneficiaries.iter() {
        if let Allocation::Percentage(bp) = beneficiary.allocation {
            total_visible_bps += bp;
        }
    }
    assert_eq!(
        total_visible_bps, 10_000,
        "visible bps must still sum to 10_000 when hashed_bps == 0 (#12 cap is identical to old cap)"
    );
    assert_eq!(
        will.hashed_beneficiaries.len(),
        0,
        "no hashed beneficiaries on this will"
    );
}

/// Issue #12: renouncing a fixed-amount beneficiary on a will with hashed bps
/// leaves the will in a state that can still be triggered and released
/// successfully, with the hashed reserve correctly withheld.
#[test]
fn will_with_hashed_bps_releases_after_fixed_beneficiary_renounces() {
    let (env, client, owner, token, token_address, contract_id) = setup();

    let beneficiary_a = Address::generate(&env);
    let beneficiary_b = Address::generate(&env);

    let balance: i128 = 1_000_000;
    let fixed_a: i128 = 200_000;
    let fixed_b: i128 = 300_000;
    let hashed_bps: u32 = 3_000; // 30% reserved for hashed beneficiary

    let will_id = client.create_will(
        &owner,
        &vec![&env, (token_address.clone(), balance)],
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

    let commitment = make_commitment(&env, 42);
    client.add_hashed_beneficiary(&will_id, &owner, &commitment, &hashed_bps);

    // beneficiary_a renounces.
    client.renounce_beneficiary(&will_id, &beneficiary_a);

    // Trigger and release.
    advance(&env, 91);
    client.trigger_will(&will_id);
    advance(&env, 8);
    client.release_inheritance(&will_id, &None);

    let will = client.get_will(&will_id);
    assert_eq!(will.status, WillStatus::Released, "will must be Released");

    // beneficiary_a receives nothing (renounced).
    assert_eq!(token.balance(&beneficiary_a), 0);

    // The hashed reserve is withheld from the distributable balance.
    // hashed_reserve = floor(balance * hashed_bps / 10_000) = 300_000
    let hashed_reserve = balance * hashed_bps as i128 / 10_000;
    let available = balance - hashed_reserve; // 700_000

    // beneficiary_b (last remaining fixed-amount) receives exactly fixed_b —
    // the unallocated primary remainder is refunded to the owner (#10, #383),
    // not dumped on the surviving beneficiary.
    assert_eq!(
        token.balance(&beneficiary_b),
        fixed_b,
        "beneficiary_b should receive exactly their fixed amount"
    );
    // Owner gets the remainder back: minted 1_000_000_000 - deposited 1_000_000
    // + refunded (available - fixed_b = 400_000).
    assert_eq!(
        token.balance(&owner),
        1_000_000_000 - balance + (available - fixed_b),
        "the unallocated primary remainder is refunded to the owner (#383)"
    );

    // The hashed reserve stays in the contract (will balance), not transferred.
    assert_eq!(
        token.balance(&contract_id),
        hashed_reserve,
        "hashed reserve must remain in the contract for reveal_and_claim"
    );
}

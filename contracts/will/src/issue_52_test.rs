#![cfg(test)]

//! Regression test for issue #52: `clone_will` previously set
//! `hashed_beneficiaries: Vec::new(&env)` unconditionally, dropping hashed
//! beneficiary slots from the source will. When the source had percentage
//! basis points reserved for hashed beneficiaries, the clone's visible
//! beneficiaries totalled less than 10,000 bps, violating the percentage
//! conservation invariant required at `release_inheritance` time.
//!
//! `clone_will` now deep-copies the source's hashed beneficiary slots into the
//! child will, resetting each `claimed` flag to `false` so that the clone's
//! commitments are fresh and independently claimable.

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    vec, Address, Bytes, Env,
};

use crate::{Allocation, Beneficiary, WillContract, WillContractClient, WillStatus};

const DAY: u64 = 86_400;

/// Creates a will with one visible Percentage beneficiary consuming 5,000 bps
/// and one hashed beneficiary consuming the remaining 5,000 bps. Returns the
/// env, client, owner, token address and the will id.
fn setup_will_with_hashed_beneficiary() -> (Env, Address, Address, Address, u64, Bytes) {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_700_000_000);

    let owner = Address::generate(&env);
    let token_address = env
        .register_stellar_asset_contract_v2(owner.clone())
        .address();
    StellarAssetClient::new(&env, &token_address).mint(&owner, &10_000_000);

    let contract_id = env.register(WillContract, ());
    let client = WillContractClient::new(&env, &contract_id);

    let visible_beneficiary = Address::generate(&env);

    let source_id = client.create_will(
        &owner,
        &vec![&env, (token_address.clone(), 1_000_000_i128)],
        &vec![
            &env,
            Beneficiary {
                address: visible_beneficiary.clone(),
                allocation: Allocation::Percentage(5_000),
            },
        ],
        &90,
        &7,
        &vec![&env],
        &0,
        &None,
        &0,
    );

    // Reserve 5,000 bps for a hashed beneficiary (brings visible + hashed to
    // exactly 10,000 bps).
    let preimage = soroban_sdk::Bytes::from_array(&env, &[0xab_u8; 64]);
    let commitment_hash = env.crypto().sha256(&preimage);
    let commitment = Bytes::from_array(&env, &commitment_hash.to_array());
    client.add_hashed_beneficiary(&source_id, &owner, &commitment, &5_000);

    (env, contract_id, owner, token_address, source_id, commitment)
}

#[test]
fn clone_will_deep_copies_hashed_beneficiaries() {
    let (env, contract_id, owner, token_address, source_id, commitment) =
        setup_will_with_hashed_beneficiary();
    let client = WillContractClient::new(&env, &contract_id);

    // Cloning a will with hashed beneficiaries must succeed (no
    // InvalidPercentages panic) because `clone_will` now copies the hashed
    // slots into the child.
    let clone_id = client.clone_will(
        &source_id,
        &owner,
        &vec![&env, (token_address, 1_000_000_i128)],
    );

    let source = client.get_will(&source_id);
    let cloned = client.get_will(&clone_id);

    // The clone should carry the same number of hashed beneficiary slots.
    assert_eq!(
        cloned.hashed_beneficiaries.len(),
        source.hashed_beneficiaries.len(),
        "clone must have the same number of hashed beneficiary slots as the source"
    );

    // The commitment must be preserved exactly.
    assert_eq!(
        cloned.hashed_beneficiaries.get_unchecked(0).commitment,
        commitment,
        "cloned hashed beneficiary commitment must match the source"
    );

    // The percentage must be preserved exactly.
    assert_eq!(
        cloned.hashed_beneficiaries.get_unchecked(0).percentage,
        5_000,
        "cloned hashed beneficiary percentage must match the source"
    );

    // The claimed flag must be reset to false on the clone regardless of the
    // source's state — the clone's commitments are independently claimable.
    assert!(
        !cloned.hashed_beneficiaries.get_unchecked(0).claimed,
        "cloned hashed beneficiary claimed flag must be reset to false"
    );
}

#[test]
fn clone_will_hashed_beneficiary_preserves_percentage_invariant() {
    let (env, contract_id, owner, token_address, source_id, _commitment) =
        setup_will_with_hashed_beneficiary();
    let client = WillContractClient::new(&env, &contract_id);

    let clone_id = client.clone_will(
        &source_id,
        &owner,
        &vec![&env, (token_address.clone(), 1_000_000_i128)],
    );

    // Advance time past the check-in deadline to trigger the will, then past
    // the grace period to allow release. The release must succeed without
    // panicking on InvalidPercentages — proving the 10,000 bps invariant holds.
    env.ledger().with_mut(|l| l.timestamp += 91 * DAY);
    client.trigger_will(&clone_id);
    env.ledger().with_mut(|l| l.timestamp += 8 * DAY);
    client.release_inheritance(&clone_id, &None);

    let cloned = client.get_will(&clone_id);
    assert_eq!(
        cloned.status,
        WillStatus::Released,
        "clone must release cleanly, confirming the percentage invariant is satisfied"
    );
}

#[test]
fn clone_will_with_no_hashed_beneficiaries_still_works() {
    // Baseline: cloning a source with no hashed beneficiaries must still
    // produce a clone with an empty hashed_beneficiaries list, as before.
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_700_000_000);

    let owner = Address::generate(&env);
    let token_address = env
        .register_stellar_asset_contract_v2(owner.clone())
        .address();
    StellarAssetClient::new(&env, &token_address).mint(&owner, &2_000_000);

    let contract_id = env.register(WillContract, ());
    let client = WillContractClient::new(&env, &contract_id);

    let source_id = client.create_will(
        &owner,
        &vec![&env, (token_address.clone(), 1_000_000_i128)],
        &vec![
            &env,
            Beneficiary {
                address: Address::generate(&env),
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

    let clone_id = client.clone_will(
        &source_id,
        &owner,
        &vec![&env, (token_address, 500_000_i128)],
    );

    let cloned = client.get_will(&clone_id);
    assert_eq!(
        cloned.hashed_beneficiaries.len(),
        0,
        "clone with no hashed beneficiaries on source must have empty hashed_beneficiaries"
    );
}

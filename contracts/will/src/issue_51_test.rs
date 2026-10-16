#![cfg(test)]

//! Regression test for issue #51: `clone_will` used to transfer tokens from
//! the owner into contract custody *before* checking whether the owner or
//! cloned beneficiaries had reached `MAX_WILLS_PER_INDEX`. If the index
//! capacity check failed later, the call had already made an unnecessary
//! cross-contract token transfer.
//!
//! `assert_index_capacity` now runs immediately after `assert_valid_guardians`
//! and before any token transfer, matching the order enforced by `create_will`.
//!
//! Because a panic aborts the whole transaction the final on-chain state looks
//! identical either way (the transfer would be rolled back). This test
//! therefore uses a token whose `transfer` always panics with a distinct
//! failure, and confirms the observed error is `TooManyWills` — proving the
//! capacity check ran first and the transfer was never reached.

use soroban_sdk::{
    contract, contractimpl, testutils::Address as _, token::StellarAssetClient, vec, Address, Env,
};

use crate::{Allocation, Beneficiary, WillContract, WillContractClient, WillError};

/// A token that answers `decimals()` like a real SEP-41 token but panics
/// unconditionally inside `transfer`, so any call that reaches the transfer
/// step is unmistakably distinguishable from one rejected earlier.
#[contract]
pub struct PoisonToken;

#[contractimpl]
impl PoisonToken {
    pub fn decimals(_env: Env) -> u32 {
        7
    }

    pub fn transfer(_env: Env, _from: Address, _to: Address, _amount: i128) {
        panic!("PoisonToken::transfer must never be reached by this test");
    }
}

#[test]
fn clone_will_checks_index_capacity_before_any_token_transfer() {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let real_token = env
        .register_stellar_asset_contract_v2(owner.clone())
        .address();
    StellarAssetClient::new(&env, &real_token).mint(&owner, &10_000_000);

    let contract_id = env.register(WillContract, ());
    let client = WillContractClient::new(&env, &contract_id);

    // Fill the owner's index to the test-mode-lowered MAX_WILLS_PER_INDEX
    // (10) using a real token, so these all succeed normally.
    let mut last_source_id = 0u64;
    for _ in 0..10 {
        last_source_id = client.create_will(
            &owner,
            &vec![&env, (real_token.clone(), 100_i128)],
            &vec![
                &env,
                Beneficiary {
                    address: beneficiary.clone(),
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

    // The owner's index is now at capacity. A clone attempt is already doomed.
    // It uses PoisonToken so that, if the transfer were ever reached, we'd see
    // PoisonToken's panic instead of TooManyWills.
    let poison_token = env.register(PoisonToken, ());
    let result = client.try_clone_will(
        &last_source_id,
        &owner,
        &vec![&env, (poison_token, 1_000_i128)],
    );

    assert_eq!(
        result,
        Err(Ok(WillError::TooManyWills.into())),
        "expected the capacity check to reject clone_will before PoisonToken::transfer could run"
    );
}

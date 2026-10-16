#![cfg(test)]

//! Unit test for the extended `will_created` event payload.
//!
//! Previously `will_created` published only `beneficiaries_count`, so an
//! off-chain indexer building an activity feed purely from events (rather
//! than always falling back to `get_will`) could not reconstruct who a
//! will's beneficiaries actually are. The event now publishes the full
//! beneficiary list (address + allocation pairs), bounded by
//! `MAX_BENEFICIARIES` — see the doc comment on `events::will_created` for
//! the payload-size tradeoff.
//!
//! Self-contained module (its own `setup`) so it does not depend on the
//! large pre-existing `test.rs` suite, which still constructs `Beneficiary`
//! via the old `basis_points` field.

use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    vec, Address, Env, IntoVal,
};

use crate::event_test_support as ev;
use crate::{Allocation, Beneficiary, WillContract, WillContractClient};

/// The ledger timestamp both tests fix the environment to; `create_will`'s
/// `created` event publishes the first check-in deadline off this clock, so
/// the expected event is built from the same constant rather than a
/// hard-coded duplicate.
const CREATED_AT: u64 = 1_700_000_000;

/// The deadline `create_will` publishes for a `checkin_period_days`-day
/// period set at [`CREATED_AT`].
fn expected_deadline(checkin_period_days: u64) -> u64 {
    CREATED_AT + checkin_period_days * 86_400
}

#[test]
fn will_created_event_includes_full_beneficiary_list() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_700_000_000);

    let owner = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(owner.clone());
    let token_address = sac.address();
    StellarAssetClient::new(&env, &token_address).mint(&owner, &1_000_000_000);

    let contract_id = env.register(WillContract, ());
    let client = WillContractClient::new(&env, &contract_id);

    let beneficiary_a = Address::generate(&env);
    let beneficiary_b = Address::generate(&env);
    let beneficiaries = vec![
        &env,
        Beneficiary {
            address: beneficiary_a.clone(),
            allocation: Allocation::Percentage(7_000),
        },
        Beneficiary {
            address: beneficiary_b.clone(),
            allocation: Allocation::Percentage(3_000),
        },
    ];
    let tokens = vec![&env, (token_address, 1_000_000_i128)];

    let will_id = client.create_will(
        &owner,
        &tokens,
        &beneficiaries,
        &90,
        &7,
        &vec![&env],
        &0,
        &None,
        &0,
    );

    let event = ev::find_event(
        &env,
        symbol_short!("created"),
        Some(will_id),
        "will_created",
    );
    let expected = ev::expected_event(
        &env,
        &contract_id,
        ev::topics(&env, symbol_short!("created"), &[will_id]),
        (
            owner.clone(),
            1u32,
            beneficiaries.clone(),
            expected_deadline(90),
        )
            .into_val(&env),
    );
    assert_eq!(
        event, expected,
        "will_created payload must match what create_will was given"
    );
}

/// Verify that `will_created` event is correctly emitted when guardians are
/// specified at will creation time, and the event includes the full beneficiary list.
#[test]
fn will_created_event_with_guardians_included() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_700_000_000);

    let owner = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(owner.clone());
    let token_address = sac.address();
    StellarAssetClient::new(&env, &token_address).mint(&owner, &1_000_000_000);

    let contract_id = env.register(WillContract, ());
    let client = WillContractClient::new(&env, &contract_id);

    let beneficiary_a = Address::generate(&env);
    let beneficiary_b = Address::generate(&env);
    let guardian_1 = Address::generate(&env);
    let guardian_2 = Address::generate(&env);

    let beneficiaries = vec![
        &env,
        Beneficiary {
            address: beneficiary_a.clone(),
            allocation: Allocation::Percentage(6_000),
        },
        Beneficiary {
            address: beneficiary_b.clone(),
            allocation: Allocation::Percentage(4_000),
        },
    ];
    let tokens = vec![&env, (token_address, 1_000_000_i128)];
    let guardians = vec![&env, guardian_1.clone(), guardian_2.clone()];

    let will_id = client.create_will(
        &owner,
        &tokens,
        &beneficiaries,
        &90,
        &7,
        &guardians,
        &2,
        &None,
        &0,
    );

    let event = ev::find_event(
        &env,
        symbol_short!("created"),
        Some(will_id),
        "will_created",
    );
    let expected = ev::expected_event(
        &env,
        &contract_id,
        ev::topics(&env, symbol_short!("created"), &[will_id]),
        (
            owner.clone(),
            1u32,
            beneficiaries.clone(),
            expected_deadline(90),
        )
            .into_val(&env),
    );
    assert_eq!(
        event, expected,
        "will_created must carry the beneficiary list even when guardians are set"
    );
}

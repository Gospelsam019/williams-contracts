#![cfg(test)]

//! Regression coverage for issue #41: the `OwnerWills` reverse index could
//! expire during a long period of inactivity, stranding previously indexed
//! will ids.
//!
//! If an owner creates a will and then performs no further creates or removals
//! for more than `BUMP_AMOUNT` ledgers (roughly 60 days), the `OwnerWills`
//! entry's TTL lapses. When the owner later calls `create_will`, `index_push`
//! sees a missing key and initialises a fresh `Vec::new(env)`, silently
//! losing every previously indexed id.
//!
//! The fix: `check_in`, `trigger_will`, and `get_wills_by_owner` all refresh
//! the `OwnerWills` TTL when the entry exists, so normal owner activity keeps
//! the index alive without requiring a second `create_will`.

use soroban_sdk::{
    testutils::{storage::Persistent as _, Address as _, Ledger},
    token::StellarAssetClient,
    vec, Address, Env,
};

use crate::storage::DataKey;
use crate::{Allocation, Beneficiary, WillContract, WillContractClient};

/// Mirrors `DAY_IN_LEDGERS` in `storage.rs`: 86,400 s ÷ 5 s/ledger.
const DAY_IN_LEDGERS: u32 = 17_280;
const DAY: u64 = 86_400;

fn setup(env: &Env) -> (WillContractClient<'_>, Address, Address) {
    let owner = Address::generate(env);
    let sac = env.register_stellar_asset_contract_v2(owner.clone());
    let token_address = sac.address();
    StellarAssetClient::new(env, &token_address).mint(&owner, &100_000_000);

    let contract_id = env.register(WillContract, ());
    let client = WillContractClient::new(env, &contract_id);
    (client, owner, token_address)
}

fn create_will(
    env: &Env,
    client: &WillContractClient<'_>,
    owner: &Address,
    token_address: &Address,
) -> u64 {
    let beneficiary = Address::generate(env);
    client.create_will(
        owner,
        &vec![env, (token_address.clone(), 1_000_000_i128)],
        &vec![
            env,
            Beneficiary {
                address: beneficiary,
                allocation: Allocation::Percentage(10_000),
            },
        ],
        &90,
        &7,
        &vec![env],
        &0,
        &None,
        &0,
    )
}

/// Reads the TTL of the `OwnerWills` index entry for `owner` from within the
/// contract's own storage frame (required by Soroban's persistent storage API).
fn owner_index_ttl(env: &Env, contract_id: &Address, owner: &Address) -> u32 {
    env.as_contract(contract_id, || {
        env.storage()
            .persistent()
            .get_ttl(&DataKey::OwnerWills(owner.clone()))
    })
}

/// Advancing the ledger past 30 days of ledger closures and then calling
/// `check_in` must keep the `OwnerWills` TTL above the LIFETIME_THRESHOLD
/// mark, so a subsequent period of inactivity does not silently expire the
/// index before the next owner interaction.
#[test]
fn check_in_refreshes_owner_index_ttl() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_700_000_000);

    let (client, owner, token_address) = setup(&env);
    let contract_id = client.address.clone();
    let will_id = create_will(&env, &client, &owner, &token_address);

    // Age the ledger past LIFETIME_THRESHOLD (30 days) so the index TTL has
    // dropped below the bump target.
    env.ledger()
        .with_mut(|l| l.sequence_number += DAY_IN_LEDGERS * 40);
    let ttl_before = owner_index_ttl(&env, &contract_id, &owner);

    // check_in must refresh the owner index TTL.
    env.ledger().with_mut(|l| l.timestamp += 40 * DAY);
    client.check_in(&will_id, &owner);

    let ttl_after = owner_index_ttl(&env, &contract_id, &owner);
    assert!(
        ttl_after > ttl_before,
        "check_in must refresh the OwnerWills TTL: was {ttl_before}, still {ttl_after} after check_in"
    );
    assert!(
        ttl_after > DAY_IN_LEDGERS * 30,
        "renewed TTL must clear the ~30-day LIFETIME_THRESHOLD, got {ttl_after}"
    );
}

/// `get_wills_by_owner` must refresh the `OwnerWills` TTL so that a dashboard
/// query counts as owner activity, preventing the index from lapsing during a
/// period where the owner only reads but never creates or removes wills.
#[test]
fn get_wills_by_owner_refreshes_owner_index_ttl() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_700_000_000);

    let (client, owner, token_address) = setup(&env);
    let contract_id = client.address.clone();
    create_will(&env, &client, &owner, &token_address);

    // Age the ledger so the TTL has dropped from its freshly-bumped value.
    env.ledger()
        .with_mut(|l| l.sequence_number += DAY_IN_LEDGERS * 40);
    let ttl_before = owner_index_ttl(&env, &contract_id, &owner);

    // A read-only query must still refresh the TTL.
    client.get_wills_by_owner(&owner, &None, &50);

    let ttl_after = owner_index_ttl(&env, &contract_id, &owner);
    assert!(
        ttl_after > ttl_before,
        "get_wills_by_owner must refresh the OwnerWills TTL: was {ttl_before}, still {ttl_after}"
    );
    assert!(
        ttl_after > DAY_IN_LEDGERS * 30,
        "renewed TTL must clear the ~30-day LIFETIME_THRESHOLD, got {ttl_after}"
    );
}

/// A will that goes Triggered after months of inactivity must still be
/// discoverable under the owner's index after the trigger cycle resolves.
/// `trigger_will` refreshes the `OwnerWills` TTL so that a subsequent
/// `get_wills_by_owner` call finds the previously indexed will ids intact.
#[test]
fn trigger_will_refreshes_owner_index_ttl() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_700_000_000);

    let (client, owner, token_address) = setup(&env);
    let contract_id = client.address.clone();
    let will_id = create_will(&env, &client, &owner, &token_address);

    // Age the ledger past LIFETIME_THRESHOLD without any owner activity so the
    // index TTL decrements.
    env.ledger()
        .with_mut(|l| l.sequence_number += DAY_IN_LEDGERS * 40);
    let ttl_before = owner_index_ttl(&env, &contract_id, &owner);

    // Advance time past the check-in deadline so trigger_will succeeds.
    env.ledger().with_mut(|l| l.timestamp += 91 * DAY);
    client.trigger_will(&will_id);

    let ttl_after = owner_index_ttl(&env, &contract_id, &owner);
    assert!(
        ttl_after > ttl_before,
        "trigger_will must refresh the OwnerWills TTL: was {ttl_before}, still {ttl_after}"
    );
    assert!(
        ttl_after > DAY_IN_LEDGERS * 30,
        "renewed TTL must clear the ~30-day LIFETIME_THRESHOLD, got {ttl_after}"
    );
}

/// End-to-end scenario from the issue: create a will, advance the ledger past
/// the point where the OwnerWills entry would normally expire (more than
/// BUMP_AMOUNT ledgers without activity), check_in to refresh it, then create
/// a second will and confirm both appear in `get_wills_by_owner`.
#[test]
fn owner_index_survives_long_inactivity_when_checkin_refreshes_it() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_700_000_000);

    let (client, owner, token_address) = setup(&env);
    let will_id_1 = create_will(&env, &client, &owner, &token_address);

    // Advance 30 days in timestamp and 40 days in ledger sequence to age the
    // index entry past LIFETIME_THRESHOLD without a new create/remove touching it.
    env.ledger().with_mut(|l| {
        l.timestamp += 30 * DAY;
        l.sequence_number += DAY_IN_LEDGERS * 40;
    });

    // check_in renews both the will and the owner index TTL.
    client.check_in(&will_id_1, &owner);

    // Create a second will. Without the fix, OwnerWills would be missing (or
    // close to expiry and would initialise a fresh Vec), stranding will_id_1.
    let will_id_2 = create_will(&env, &client, &owner, &token_address);

    let wills = client.get_wills_by_owner(&owner, &None, &50);
    let mut found_1 = false;
    let mut found_2 = false;
    for w in wills.iter() {
        if w.id == will_id_1 {
            found_1 = true;
        }
        if w.id == will_id_2 {
            found_2 = true;
        }
    }
    assert!(
        found_1,
        "will_id_1 must still appear in the owner index after a long-inactivity check_in + create"
    );
    assert!(
        found_2,
        "will_id_2 must appear in the owner index after the second create"
    );
}

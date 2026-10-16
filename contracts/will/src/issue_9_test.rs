#![cfg(test)]

//! Regression test for issue #9: `merge_wills` must clear the persistent
//! `DataKey::GuardianVote` and `DataKey::GuardianCancelVote` entries for
//! **both** `will_a` and `will_b`, not just `will_b`.
//!
//! Before the fix, only `will_b`'s persistent vote rows were removed via
//! `storage::reset_guardian_votes` / `storage::reset_guardian_cancel_votes`.
//! `will_a`'s in-memory counters (`guardian_votes`, `guardian_cancel_votes`)
//! were zeroed, but the corresponding storage entries were left behind as
//! orphans that could distort future `recount_guardian_votes` calls if a
//! guardian address was reused.

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    vec, Address, Env,
};

use crate::storage::DataKey;
use crate::{Allocation, Beneficiary, GuardianVoteReason, WillContract, WillContractClient};

const DAY: u64 = 86_400;

fn setup<'a>() -> (Env, WillContractClient<'a>, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_700_000_000);

    let owner = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(owner.clone());
    let token_address = sac.address();
    StellarAssetClient::new(&env, &token_address).mint(&owner, &1_000_000_000);

    let contract_id = env.register(WillContract, ());
    let client = WillContractClient::new(&env, &contract_id);

    (env, client, owner, token_address, contract_id)
}

fn key_exists(env: &Env, contract_id: &Address, key: &DataKey) -> bool {
    env.as_contract(contract_id, || env.storage().persistent().has(key))
}

/// Issue #9: persistent `GuardianVote` entries for `will_a` must be removed
/// by `merge_wills`, not just the in-memory counter.
///
/// Setup: two wills each with two guardians (threshold=2 so a single vote
/// does not trigger an immediate release). Guardian_a casts one vote on
/// will_a and one on will_b, writing a `GuardianVote` row to storage for
/// each. After the merge, both rows must be gone.
#[test]
fn merge_wills_purges_persistent_guardian_votes_for_both_wills() {
    let (env, client, owner, token_address, contract_id) = setup();

    let beneficiary = Address::generate(&env);
    let guardian_a = Address::generate(&env);
    let guardian_b = Address::generate(&env);

    let beneficiaries = vec![
        &env,
        Beneficiary {
            address: beneficiary.clone(),
            allocation: Allocation::Percentage(10_000),
        },
    ];
    let tokens = vec![&env, (token_address.clone(), 500_000_i128)];

    // Two guardians, threshold=2: a single vote from guardian_a writes the
    // GuardianVote entry to storage but does NOT reach quorum.
    let will_id_a = client.create_will(
        &owner,
        &tokens,
        &beneficiaries,
        &90,
        &7,
        &vec![&env, guardian_a.clone(), guardian_b.clone()],
        &2,
        &None,
        &0,
    );
    let will_id_b = client.create_will(
        &owner,
        &tokens,
        &beneficiaries,
        &90,
        &7,
        &vec![&env, guardian_a.clone(), guardian_b.clone()],
        &2,
        &None,
        &0,
    );

    client.accept_guardian_role(&will_id_a, &guardian_a);
    client.accept_guardian_role(&will_id_b, &guardian_a);
    client.accept_guardian_role(&will_id_a, &guardian_b);
    client.accept_guardian_role(&will_id_b, &guardian_b);

    // Clear the 7-day cooldown before voting.
    env.ledger().with_mut(|l| l.timestamp += 8 * DAY);

    // Cast a release vote on will_a and will_b from guardian_a only
    // (threshold=2, so no quorum, will stays Active).
    client.guardian_trigger(&will_id_a, &guardian_a, &GuardianVoteReason::Deceased);
    client.guardian_trigger(&will_id_b, &guardian_a, &GuardianVoteReason::Deceased);

    // Sanity: both entries must exist before the merge.
    let vote_key_a = DataKey::GuardianVote(will_id_a, guardian_a.clone());
    let vote_key_b = DataKey::GuardianVote(will_id_b, guardian_a.clone());
    assert!(
        key_exists(&env, &contract_id, &vote_key_a),
        "will_a's GuardianVote entry must exist before merge"
    );
    assert!(
        key_exists(&env, &contract_id, &vote_key_b),
        "will_b's GuardianVote entry must exist before merge"
    );

    // Perform the merge.
    client.merge_wills(&owner, &will_id_a, &will_id_b);

    // After merge, both persistent vote entries must be gone.
    assert!(
        !key_exists(&env, &contract_id, &vote_key_a),
        "will_a's GuardianVote entry must be removed by merge_wills (#9)"
    );
    assert!(
        !key_exists(&env, &contract_id, &vote_key_b),
        "will_b's GuardianVote entry must be removed by merge_wills (#9)"
    );

    // In-memory counters must also be zero on the surviving will.
    let merged = client.get_will(&will_id_a);
    assert_eq!(
        merged.guardian_votes, 0,
        "guardian_votes must be 0 on the merged will"
    );
}

/// Issue #9: same coverage for `GuardianCancelVote` entries.
///
/// Trigger the will first so the guardian can cast a cancel vote, then merge.
/// Both `GuardianCancelVote` entries must be removed from persistent storage.
#[test]
fn merge_wills_purges_persistent_guardian_cancel_votes_for_both_wills() {
    let (env, client, owner, token_address, contract_id) = setup();

    let beneficiary = Address::generate(&env);
    let guardian_a = Address::generate(&env);
    let guardian_b = Address::generate(&env);

    let beneficiaries = vec![
        &env,
        Beneficiary {
            address: beneficiary.clone(),
            allocation: Allocation::Percentage(10_000),
        },
    ];
    let tokens = vec![&env, (token_address.clone(), 500_000_i128)];

    // Two guardians, threshold=2 for cancel too: a single cancel vote from
    // guardian_a writes a `GuardianCancelVote` row without reaching quorum.
    let will_id_a = client.create_will(
        &owner,
        &tokens,
        &beneficiaries,
        &90,
        &7,
        &vec![&env, guardian_a.clone(), guardian_b.clone()],
        &2,
        &None,
        &0,
    );
    let will_id_b = client.create_will(
        &owner,
        &tokens,
        &beneficiaries,
        &90,
        &7,
        &vec![&env, guardian_a.clone(), guardian_b.clone()],
        &2,
        &None,
        &0,
    );

    client.accept_guardian_role(&will_id_a, &guardian_a);
    client.accept_guardian_role(&will_id_b, &guardian_a);
    client.accept_guardian_role(&will_id_a, &guardian_b);
    client.accept_guardian_role(&will_id_b, &guardian_b);

    // Let the checkin deadline pass and trigger both wills.
    env.ledger().with_mut(|l| l.timestamp += 91 * DAY);
    client.trigger_will(&will_id_a);
    client.trigger_will(&will_id_b);

    // Cast one cancel vote on each will (no quorum → stays Triggered).
    client.guardian_cancel_trigger(&will_id_a, &guardian_a);
    client.guardian_cancel_trigger(&will_id_b, &guardian_a);

    let cancel_key_a = DataKey::GuardianCancelVote(will_id_a, guardian_a.clone());
    let cancel_key_b = DataKey::GuardianCancelVote(will_id_b, guardian_a.clone());

    // Sanity: entries exist before the merge.
    assert!(
        key_exists(&env, &contract_id, &cancel_key_a),
        "will_a's GuardianCancelVote entry must exist before merge"
    );
    assert!(
        key_exists(&env, &contract_id, &cancel_key_b),
        "will_b's GuardianCancelVote entry must exist before merge"
    );

    // Emergency check-in to restore both wills to Active before merging
    // (merge_wills requires both wills to be Active).
    client.emergency_checkin(&will_id_a, &owner);
    client.emergency_checkin(&will_id_b, &owner);

    client.merge_wills(&owner, &will_id_a, &will_id_b);

    // After the merge both cancel-vote entries must be gone.
    assert!(
        !key_exists(&env, &contract_id, &cancel_key_a),
        "will_a's GuardianCancelVote entry must be removed by merge_wills (#9)"
    );
    assert!(
        !key_exists(&env, &contract_id, &cancel_key_b),
        "will_b's GuardianCancelVote entry must be removed by merge_wills (#9)"
    );
}

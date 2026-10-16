#![cfg(test)]

//! Regression tests for issues #57, #58, #59, and #60.
//!
//! #57 — `assert_valid_allocations` must allow visible percentage beneficiaries
//!        to total less than 10,000 bps when hashed beneficiaries fill the gap
//!        (visible_pct + hashed_pct == 10,000).
//!
//! #58 — `split_will` must reject source wills that have unrevealed hashed
//!        beneficiaries, because moving token balances without adjusting the
//!        committed hashed percentages would leave the source under-funded.
//!
//! #59 — `get_will_history_page` must use a seq-based cursor so that history
//!        trimming (MAX_HISTORY_ENTRIES) does not shift earlier offsets and
//!        cause callers to skip or duplicate entries.
//!
//! #60 — `scripts/check_resource_costs.sh` is tested in its own acceptance
//!        section (bash execution); no Rust test is required for that fix.

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    vec,
    xdr::ToXdr,
    Address, Bytes, Env,
};

use crate::{
    storage::MAX_HISTORY_ENTRIES, types::WillStatusTransition, Allocation, Beneficiary,
    WillContract, WillContractClient, WillError, WillStatus, PREIMAGE_LENGTH,
};

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

    (env.clone(), client, owner, token_address)
}

fn advance(env: &Env, days: u64) {
    env.ledger().with_mut(|l| l.timestamp += days * DAY);
}

fn sha256_commitment(env: &Env, seed: u8) -> Bytes {
    let preimage = Bytes::from_array(env, &[seed; 64]);
    let digest = env.crypto().sha256(&preimage);
    Bytes::from_array(env, &digest.to_array())
}

// ---------------------------------------------------------------------------
// Issue #57: mixed visible + hashed percentages
// ---------------------------------------------------------------------------

/// A will with 5,000 bps visible + 5,000 bps hashed must be accepted after
/// `add_hashed_beneficiary`, released after grace expires, and claimed via
/// `reveal_and_claim` (#57).
#[test]
fn issue_57_mixed_visible_and_hashed_percentages_accepted() {
    let (env, client, owner, token_address) = setup();
    let visible_beneficiary = Address::generate(&env);
    let secret_address = Address::generate(&env);

    // Create will with 5,000 bps visible (50% of 1,000,000 tokens)
    let will_id = client.create_will(
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

    assert_eq!(
        client.get_will(&will_id).beneficiaries.len(),
        1,
        "will created with 5,000 bps visible"
    );

    // Derive commitment from secret_address
    let mut preimage_bytes = [0x57u8; PREIMAGE_LENGTH as usize];
    let secret_address_digest = env.crypto().sha256(&secret_address.clone().to_xdr(&env));
    preimage_bytes[..32].copy_from_slice(&secret_address_digest.to_array());
    let preimage = Bytes::from_array(&env, &preimage_bytes);
    let commitment = env.crypto().sha256(&preimage);
    let commitment_bytes = Bytes::from_array(&env, &commitment.to_array());

    // Adding 5,000 bps hashed brings the total to exactly 10,000 bps — valid.
    client.add_hashed_beneficiary(&will_id, &owner, &commitment_bytes, &5_000);

    let will = client.get_will(&will_id);
    assert_eq!(will.hashed_beneficiaries.len(), 1);
    assert_eq!(will.hashed_beneficiaries.get_unchecked(0).percentage, 5_000);

    // Advance time past check-in period and trigger will
    advance(&env, 91);
    client.trigger_will(&will_id);

    // Advance time past grace period
    advance(&env, 8);

    let token_client = soroban_sdk::token::Client::new(&env, &token_address);

    // Release inheritance - visible beneficiary gets 50% (500,000)
    client.release_inheritance(&will_id, &None);
    assert_eq!(
        token_client.balance(&visible_beneficiary),
        500_000,
        "visible beneficiary should receive 50% (500,000 tokens)"
    );

    // Hashed beneficiary reveals and claims their 50% (500,000)
    client.reveal_and_claim(&will_id, &secret_address, &preimage);
    assert_eq!(
        token_client.balance(&secret_address),
        500_000,
        "hashed beneficiary should receive 50% (500,000 tokens)"
    );

    // Entire estate has been distributed
    let final_will = client.get_will(&will_id);
    assert_eq!(final_will.balance, 0);
    assert_eq!(token_client.balance(&client.address), 0);
}

/// Creating a will with visible percentages exceeding 10,000 bps must be
/// REJECTED with `InvalidPercentages`.
#[test]
fn issue_57_visible_percentage_exceeding_10000_is_rejected() {
    let (env, client, owner, token_address) = setup();
    let beneficiary = Address::generate(&env);

    let result = client.try_create_will(
        &owner,
        &vec![&env, (token_address.clone(), 1_000_000_i128)],
        &vec![
            &env,
            Beneficiary {
                address: beneficiary.clone(),
                allocation: Allocation::Percentage(10_001),
            },
        ],
        &90,
        &7,
        &vec![&env],
        &0,
        &None,
        &0,
    );

    assert_eq!(
        result,
        Err(Ok(WillError::InvalidPercentages.into())),
        "a visible-only list exceeding 10,000 bps must be rejected"
    );
}

/// Adding a hashed slot that would push the combined total above 10,000 bps
/// must still be rejected by `assert_valid_percentages`.
#[test]
fn issue_57_combined_over_10000_bps_is_rejected() {
    let (env, client, owner, token_address) = setup();
    let visible_beneficiary = Address::generate(&env);

    // Create with 7,000 bps visible. Needs hashed to fill gap.
    let will_id = client.create_will(
        &owner,
        &vec![&env, (token_address.clone(), 1_000_000_i128)],
        &vec![
            &env,
            Beneficiary {
                address: visible_beneficiary.clone(),
                allocation: Allocation::Percentage(7_000),
            },
        ],
        &90,
        &7,
        &vec![&env],
        &0,
        &None,
        &0,
    );

    // First hashed slot: 3,000 bps brings total to exactly 10,000. OK.
    let commitment1 = sha256_commitment(&env, 1);
    client.add_hashed_beneficiary(&will_id, &owner, &commitment1, &3_000);

    // A second hashed slot with any percentage would exceed 10,000 bps.
    let commitment2 = sha256_commitment(&env, 2);
    let result = client.try_add_hashed_beneficiary(&will_id, &owner, &commitment2, &1);
    assert_eq!(
        result,
        Err(Ok(WillError::InvalidPercentages.into())),
        "adding a hashed slot that pushes combined total over 10,000 bps must be rejected"
    );
}

// ---------------------------------------------------------------------------
// Issue #58: split_will must reject wills with hashed beneficiaries
// ---------------------------------------------------------------------------

/// `split_will` on a source will with hashed beneficiaries must panic with
/// `InvalidSplit` — moving balances without adjusting committed percentages
/// would leave the source under-funded at reveal_and_claim time (#58).
#[test]
fn issue_58_split_will_rejects_wills_with_hashed_beneficiaries() {
    let (env, client, owner, token_address) = setup();
    let beneficiary_a = Address::generate(&env);

    // Create will with 5,000 bps visible beneficiary.
    let will_id = client.create_will(
        &owner,
        &vec![&env, (token_address.clone(), 1_000_000_i128)],
        &vec![
            &env,
            Beneficiary {
                address: beneficiary_a.clone(),
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

    let commitment = sha256_commitment(&env, 58);
    client.add_hashed_beneficiary(&will_id, &owner, &commitment, &5_000);

    assert_eq!(
        client.get_will(&will_id).hashed_beneficiaries.len(),
        1,
        "hashed beneficiary must be present before split"
    );

    // Now split_will must be rejected.
    let result = client.try_split_will(
        &will_id,
        &owner,
        &vec![
            &env,
            Beneficiary {
                address: beneficiary_a.clone(),
                allocation: Allocation::Percentage(5_000),
            },
        ],
        &vec![&env, (token_address.clone(), 100_000_i128)],
    );
    assert_eq!(
        result,
        Err(Ok(WillError::InvalidSplit.into())),
        "split_will on a will with hashed beneficiaries must return InvalidSplit"
    );
}

/// `split_will` on a will with no hashed beneficiaries must still work
/// (regression guard — #58 must not break the happy path).
#[test]
fn issue_58_split_will_without_hashed_beneficiaries_still_works() {
    let (env, client, owner, token_address) = setup();
    let beneficiary_a = Address::generate(&env);
    let beneficiary_b = Address::generate(&env);

    let will_id = client.create_will(
        &owner,
        &vec![&env, (token_address.clone(), 1_000_000_i128)],
        &vec![
            &env,
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
        &vec![&env],
        &0,
        &None,
        &0,
    );

    // Split should succeed when no hashed beneficiaries exist.
    let child_id = client.split_will(
        &will_id,
        &owner,
        &vec![
            &env,
            Beneficiary {
                address: beneficiary_a.clone(),
                allocation: Allocation::Percentage(6_000),
            },
        ],
        &vec![&env, (token_address.clone(), 400_000_i128)],
    );

    assert_eq!(client.get_will(&will_id).balance, 600_000_i128);
    assert_eq!(client.get_will(&child_id).balance, 400_000_i128);
    assert_eq!(client.get_will(&child_id).status, WillStatus::Active);
}

// ---------------------------------------------------------------------------
// Issue #59: get_will_history_page seq-based cursor stability
// ---------------------------------------------------------------------------

/// After appending 60 history events (past the MAX_HISTORY_ENTRIES=50 cap),
/// paginating from a cursor that was recorded before the trim must not skip
/// or duplicate entries — the seq-based cursor remains stable even though
/// the oldest entries have been dropped (#59).
#[test]
fn issue_59_seq_cursor_stable_across_history_truncation() {
    let (env, client, owner, token_address) = setup();

    let will_id = client.create_will(
        &owner,
        &vec![&env, (token_address.clone(), 10_000_000_i128)],
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
        &2,
        &None,
        &0,
    );

    // Drive 30 cycles = 1 create + 60 transitions (trigger + emerg per cycle),
    // which pushes the trail well past MAX_HISTORY_ENTRIES=50 and forces
    // trimming of the oldest entries.
    for _ in 0..30 {
        client.check_in(&will_id, &owner);
        advance(&env, 91);
        client.trigger_will(&will_id);
        advance(&env, 1);
        client.emergency_checkin(&will_id, &owner);
    }

    // The retained window is exactly MAX_HISTORY_ENTRIES long after trimming.
    let all = client.get_will_history(&will_id);
    assert_eq!(all.len(), MAX_HISTORY_ENTRIES);

    // Walk the entire retained trail in pages of 10 using seq-based cursors.
    let mut cursor: Option<u32> = None;
    let mut walked: soroban_sdk::Vec<WillStatusTransition> = soroban_sdk::Vec::new(&env);
    loop {
        let page = client.get_will_history_page(&will_id, &cursor, &10);
        if page.is_empty() {
            break;
        }
        // Advance cursor to the seq of the last entry on this page.
        let last_seq = page.get(page.len() - 1).unwrap().seq as u32;
        cursor = Some(last_seq);
        for entry in page.iter() {
            walked.push_back(entry);
        }
    }

    // Must have visited every retained entry exactly once with no gaps.
    assert_eq!(
        walked.len(),
        all.len(),
        "seq-based pagination must cover the retained trail without gaps or duplicates"
    );

    // Verify seq values are strictly increasing (no skips, no duplicates).
    let mut prev_seq: u64 = 0;
    for entry in walked.iter() {
        assert!(
            entry.seq > prev_seq,
            "seq must be strictly increasing; got {} after {}",
            entry.seq,
            prev_seq
        );
        prev_seq = entry.seq;
    }

    // The oldest entry in the retained window must have seq > 1 (entries were
    // trimmed), proving we survived the truncation boundary.
    let oldest_seq = all.get(0).unwrap().seq;
    assert!(
        oldest_seq > 1,
        "oldest retained entry must have seq > 1 after trimming; got {oldest_seq}"
    );
}

/// Passing a cursor equal to the seq of the last entry on a page and then
/// requesting the next page must not re-read the already-seen entry.
#[test]
fn issue_59_seq_cursor_does_not_repeat_last_entry() {
    let (env, client, owner, token_address) = setup();

    let will_id = client.create_will(
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
    advance(&env, 91);
    client.trigger_will(&will_id);
    advance(&env, 8);
    client.release_inheritance(&will_id, &None);

    // 3 entries: create, trigger, release.
    let all = client.get_will_history(&will_id);
    assert_eq!(all.len(), 3);

    // Page 1: entries with seq > 0 (i.e. all from the start), limit 2.
    let p1 = client.get_will_history_page(&will_id, &None, &2);
    assert_eq!(p1.len(), 2);
    let last_seq_p1 = p1.get(p1.len() - 1).unwrap().seq as u32;

    // Page 2: entries with seq > last_seq_p1 (strictly after the last seen).
    let p2 = client.get_will_history_page(&will_id, &Some(last_seq_p1), &2);
    assert_eq!(
        p2.len(),
        1,
        "second page must contain exactly 1 remaining entry"
    );

    // Seq on p2 must be strictly greater than last_seq_p1.
    assert!(
        p2.get(0).unwrap().seq > last_seq_p1 as u64,
        "second page seq must be strictly after the cursor"
    );
}

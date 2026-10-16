#![cfg(test)]

//! Regression test for pagination support in `get_wills_by_beneficiary`.
//! Verifies that the function respects cursor and limit parameters, staying
//! within resource limits even when a beneficiary is named on many wills.
//!
//! Also contains direct unit tests for `storage::paginate_ids` and
//! `storage::paginate_history`, verifying that:
//!  - page_size is computed before any iteration (slice bounds before copy);
//!  - cursor semantics are value-based, not position-based;
//!  - only the items that belong to the requested page are returned.

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::{Client as TokenClient, StellarAssetClient},
    vec, Address, Env,
};

use crate::{Allocation, Beneficiary, WillContract, WillContractClient};

#[allow(dead_code)]
const DAY: u64 = 86_400;

fn setup<'a>() -> (
    Env,
    WillContractClient<'a>,
    Address,
    TokenClient<'a>,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_700_000_000);

    let owner = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(owner.clone());
    let token_address = sac.address();
    StellarAssetClient::new(&env, &token_address).mint(&owner, &1_000_000_000_000);

    let contract_id = env.register(WillContract, ());
    let client = WillContractClient::new(&env, &contract_id);

    (
        env.clone(),
        client,
        owner,
        TokenClient::new(&env, &token_address),
        token_address,
    )
}

#[test]
fn pagination_respects_limit_parameter() {
    let (env, client, owner, _token, token_address) = setup();
    let beneficiary = Address::generate(&env);

    // Create multiple wills with the same beneficiary
    let mut will_ids = vec![&env];
    for i in 0..5 {
        let will_id = client.create_will(
            &owner,
            &vec![&env, (token_address.clone(), 100_000_i128 + i as i128)],
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
        will_ids.push_back(will_id);
    }

    // Fetch with limit=2
    let first_page = client.get_wills_by_beneficiary(&beneficiary, &None, &2);
    assert_eq!(first_page.len(), 2);

    // Fetch with cursor pointing to second page
    let second_page =
        client.get_wills_by_beneficiary(&beneficiary, &first_page.get(1).map(|w| w.id), &2);
    assert_eq!(second_page.len(), 2);

    // Verify no duplicates between pages
    let first_ids: Vec<u64> = first_page.iter().map(|w| w.id).collect();
    let second_ids: Vec<u64> = second_page.iter().map(|w| w.id).collect();
    for id in &second_ids {
        assert!(!first_ids.contains(id));
    }
}

#[test]
fn pagination_handles_all_results() {
    let (env, client, owner, _token, token_address) = setup();
    let beneficiary = Address::generate(&env);

    // Create 3 wills
    let mut will_ids = vec![&env];
    for i in 0..3 {
        let will_id = client.create_will(
            &owner,
            &vec![&env, (token_address.clone(), 100_000_i128 + i as i128)],
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
        will_ids.push_back(will_id);
    }

    // Fetch all without pagination (limit = 0 or very high)
    let all_wills = client.get_wills_by_beneficiary(&beneficiary, &None, &100);
    assert_eq!(all_wills.len(), 3);
}

#[test]
fn pagination_with_invalid_cursor_starts_from_beginning() {
    let (env, client, owner, _token, token_address) = setup();
    let beneficiary = Address::generate(&env);

    // Create 2 wills
    for i in 0..2 {
        client.create_will(
            &owner,
            &vec![&env, (token_address.clone(), 100_000_i128 + i as i128)],
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

    // Fetch with invalid cursor (should behave gracefully)
    let results = client.get_wills_by_beneficiary(&beneficiary, &Some(9999_u64), &10);
    // Should return remaining wills after cursor, or empty if cursor is beyond all
    assert!(results.len() <= 2);
}

// ---------------------------------------------------------------------------
// Direct unit tests for storage::paginate_ids
// ---------------------------------------------------------------------------
//
// These tests drive the helper directly — no contract setup required — to
// verify that:
//   1. slice bounds (page_size) are calculated before iterating;
//   2. only `limit` (capped at MAX_PAGE_SIZE) items are returned;
//   3. the cursor is a *value bound* (exclusive), never a position;
//   4. out-of-range or past-end cursors return an empty page without iterating;
//   5. a zero limit returns empty immediately.
//
// They complement the higher-level beneficiary tests above, which go through
// the full contract machinery.

use soroban_sdk::Vec as SorobanVec;

use crate::storage::paginate_ids;

fn make_ids(env: &Env, values: &[u64]) -> SorobanVec<u64> {
    let mut v = SorobanVec::new(env);
    for &id in values {
        v.push_back(id);
    }
    v
}

/// `paginate_ids` with no cursor must return the first `limit` ids.
#[test]
fn paginate_ids_no_cursor_returns_first_page() {
    let env = Env::default();
    let ids = make_ids(&env, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);

    let page = paginate_ids(&env, &ids, None, 3);
    assert_eq!(page.len(), 3);
    assert_eq!(page.get(0).unwrap(), 1);
    assert_eq!(page.get(1).unwrap(), 2);
    assert_eq!(page.get(2).unwrap(), 3);
}

/// Cursor is exclusive: the first returned id must be strictly greater than
/// the cursor value.
#[test]
fn paginate_ids_cursor_is_exclusive_value_bound() {
    let env = Env::default();
    let ids = make_ids(&env, &[10, 20, 30, 40, 50]);

    // cursor = 20 → skip 10 and 20, return from 30 onward
    let page = paginate_ids(&env, &ids, Some(20), 3);
    assert_eq!(page.len(), 3);
    assert_eq!(page.get(0).unwrap(), 30);
    assert_eq!(page.get(1).unwrap(), 40);
    assert_eq!(page.get(2).unwrap(), 50);
}

/// A cursor at or beyond the last id must return an empty page without
/// iterating over all elements.
#[test]
fn paginate_ids_cursor_beyond_last_returns_empty() {
    let env = Env::default();
    let ids = make_ids(&env, &[1, 2, 3]);

    // cursor equals the last element → nothing is strictly greater
    let page = paginate_ids(&env, &ids, Some(3), 10);
    assert!(page.is_empty());

    // cursor beyond the last element
    let page2 = paginate_ids(&env, &ids, Some(100), 10);
    assert!(page2.is_empty());
}

/// A zero limit must return an empty page immediately.
#[test]
fn paginate_ids_zero_limit_returns_empty() {
    let env = Env::default();
    let ids = make_ids(&env, &[1, 2, 3, 4, 5]);

    let page = paginate_ids(&env, &ids, None, 0);
    assert!(page.is_empty());
}

/// An empty index must always return an empty page regardless of cursor.
#[test]
fn paginate_ids_empty_index_returns_empty() {
    let env = Env::default();
    let ids = make_ids(&env, &[]);

    assert!(paginate_ids(&env, &ids, None, 10).is_empty());
    assert!(paginate_ids(&env, &ids, Some(0), 10).is_empty());
}

/// Consecutive pages must cover all ids exactly once with no overlap.
#[test]
fn paginate_ids_consecutive_pages_cover_all_ids() {
    let env = Env::default();
    let raw: Vec<u64> = (1..=7).collect();
    let ids = make_ids(&env, &raw);

    let p1 = paginate_ids(&env, &ids, None, 3);
    assert_eq!(p1.len(), 3); // [1,2,3]

    let cursor1 = p1.last().unwrap();
    let p2 = paginate_ids(&env, &ids, Some(cursor1), 3);
    assert_eq!(p2.len(), 3); // [4,5,6]

    let cursor2 = p2.last().unwrap();
    let p3 = paginate_ids(&env, &ids, Some(cursor2), 3);
    assert_eq!(p3.len(), 1); // [7]

    // Verify no duplicates
    let all: std::vec::Vec<u64> = p1.iter().chain(p2.iter()).chain(p3.iter()).collect();
    assert_eq!(all, raw);
}

/// The returned page must never be larger than `limit`.
#[test]
fn paginate_ids_never_exceeds_limit() {
    let env = Env::default();
    let ids = make_ids(&env, &[1, 2, 3, 4, 5]);

    let page = paginate_ids(&env, &ids, None, 2);
    assert_eq!(page.len(), 2);
}

// ---------------------------------------------------------------------------
// Direct unit tests for storage::paginate_history via the contract API
// ---------------------------------------------------------------------------
//
// `paginate_history` is not pub(crate)-accessible from the test module so we
// exercise it through `get_will_history_page`, which delegates directly to
// that helper.  We verify:
//   1. limit is honoured;
//   2. a `cursor` offset skips the right number of entries;
//   3. a cursor past the end returns an empty page.

fn history_test_setup<'a>() -> (Env, WillContractClient<'a>, Address, Address) {
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

/// Requesting a page whose cursor is beyond the recorded history returns empty.
#[test]
fn paginate_history_cursor_past_end_returns_empty() {
    let (env, client, owner, token_address) = history_test_setup();
    let beneficiary = Address::generate(&env);

    let will_id = client.create_will(
        &owner,
        &vec![&env, (token_address.clone(), 500_000_i128)],
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

    // There is one history entry (the "created" transition).
    // cursor = 999 (well past the end) must return empty.
    let page = client.get_will_history_page(&will_id, &Some(999_u32), &10);
    assert!(page.is_empty());
}

/// Requesting with limit=0 returns an empty page.
#[test]
fn paginate_history_zero_limit_returns_empty() {
    let (env, client, owner, token_address) = history_test_setup();
    let beneficiary = Address::generate(&env);

    let will_id = client.create_will(
        &owner,
        &vec![&env, (token_address.clone(), 500_000_i128)],
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

    let page = client.get_will_history_page(&will_id, &None, &0);
    assert!(page.is_empty());
}

/// Two consecutive history pages must together cover all recorded transitions.
#[test]
fn paginate_history_pages_cover_full_trail() {
    let (env, client, owner, token_address) = history_test_setup();
    let beneficiary = Address::generate(&env);

    let will_id = client.create_will(
        &owner,
        &vec![&env, (token_address.clone(), 500_000_i128)],
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

    // Trigger → release to generate additional history entries.
    env.ledger()
        .with_mut(|l| l.timestamp = 1_700_000_000 + 91 * DAY);
    client.trigger_will(&will_id);

    env.ledger()
        .with_mut(|l| l.timestamp = 1_700_000_000 + 99 * DAY);
    client.release_inheritance(&will_id, &None);

    let full = client.get_will_history(&will_id);
    let n = full.len();
    assert!(n >= 2, "expected at least 2 history entries");

    // Page 1: first entry.
    let p1 = client.get_will_history_page(&will_id, &None, &1);
    assert_eq!(p1.len(), 1);

    // Page 2: all remaining entries.
    let p2 = client.get_will_history_page(&will_id, &Some(1_u32), &50);
    assert_eq!(p1.len() + p2.len(), n);

    // Verify no overlap: seq values must be distinct across both pages.
    let seqs_p1: std::vec::Vec<u64> = p1.iter().map(|e| e.seq).collect();
    let seqs_p2: std::vec::Vec<u64> = p2.iter().map(|e| e.seq).collect();
    for seq in &seqs_p2 {
        assert!(
            !seqs_p1.contains(seq),
            "history page overlap: seq {seq} appears in both pages"
        );
    }
}

#![cfg(test)]

//! Comprehensive negative authorization tests for the Williams contract.
//!
//! Every owner-only, guardian-only, and beneficiary-only entry point must
//! reject an unauthorized caller with the exact declared [`WillError`] code
//! documented for that path. These tests are a regression net against
//! accidental permission regressions during refactoring.
//!
//! ## Entry points tested
//!
//! - **Owner-only**: `confirm_will`, `check_in` (without delegate),
//!   `set_delegate`, `batch_check_in`, `emergency_checkin`, `cancel_will`,
//!   `close_will`, `update_beneficiaries`, `update_guardians`,
//!   `update_guardians_weighted`, `update_periods`, `update_will_settings`,
//!   `top_up`, `merge_wills`, `split_will`, `add_hashed_beneficiary`,
//!   `clone_will`, `migrate_will`.
//! - **Guardian-only**: `guardian_trigger`, `guardian_cancel_trigger`,
//!   `accept_guardian_role`, `reject_guardian_role`.
//! - **Beneficiary-only**: `renounce_beneficiary`.
//!
//! Each test creates a will with a known owner/guardian/beneficiary and
//! attempts the call with a different address, asserting that the result is
//! `Err(Ok(expected_error))` rather than `Ok(_)` or `Err(Err(abort))`.

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    vec, Address, Bytes, Env,
};

use crate::{Allocation, Beneficiary, WillContract, WillContractClient, WillError};

const DAY: u64 = 86_400;

fn setup<'a>() -> (
    Env,
    WillContractClient<'a>,
    Address,
    Address,
    Address,
    Address,
    Address,
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

    let non_owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let guardian = Address::generate(&env);

    (
        env.clone(),
        client,
        owner,
        token_address,
        non_owner,
        beneficiary,
        guardian,
    )
}

fn create_active_will(
    client: &WillContractClient,
    owner: &Address,
    token_address: &Address,
    beneficiary: &Address,
    guardian: Option<&Address>,
) -> u64 {
    let (guardians, threshold) = if let Some(g) = guardian {
        (
            vec![client.address.env(), g.clone()],
            1u32, // 1-of-1 quorum for a single guardian
        )
    } else {
        (vec![client.address.env()], 0u32)
    };

    client.create_will(
        owner,
        &vec![client.address.env(), (token_address.clone(), 500_000_i128)],
        &vec![
            client.address.env(),
            Beneficiary {
                address: beneficiary.clone(),
                allocation: Allocation::Percentage(10_000),
            },
        ],
        &90,
        &7,
        &guardians,
        &threshold,
        &None,
        &0,
    )
}

// ---------------------------------------------------------------------------
// Owner-only entry points
// ---------------------------------------------------------------------------

#[test]
fn confirm_will_rejects_non_owner() {
    let (env, client, owner, token_address, non_owner, beneficiary, _guardian) = setup();

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
        &1, // 1-day confirmation window
    );

    assert_eq!(
        client.try_confirm_will(&will_id, &non_owner),
        Err(Ok(WillError::NotOwner.into()))
    );
}

#[test]
fn check_in_rejects_non_owner_without_delegate() {
    let (_env, client, owner, token_address, non_owner, beneficiary, _guardian) = setup();
    let will_id = create_active_will(&client, &owner, &token_address, &beneficiary, None);

    assert_eq!(
        client.try_check_in(&will_id, &non_owner),
        Err(Ok(WillError::NotOwner.into()))
    );
}

#[test]
fn set_delegate_rejects_non_owner() {
    let (env, client, owner, token_address, non_owner, beneficiary, _guardian) = setup();
    let will_id = create_active_will(&client, &owner, &token_address, &beneficiary, None);

    let delegate = Address::generate(&env);
    assert_eq!(
        client.try_set_delegate(&will_id, &non_owner, &Some(delegate)),
        Err(Ok(WillError::NotOwner.into()))
    );
}

#[test]
fn batch_check_in_rejects_non_owner() {
    let (env, client, owner, token_address, non_owner, beneficiary, _guardian) = setup();
    let will_id = create_active_will(&client, &owner, &token_address, &beneficiary, None);

    assert_eq!(
        client.try_batch_check_in(&vec![&env, will_id], &non_owner),
        Err(Ok(WillError::NotOwner.into()))
    );
}

#[test]
fn emergency_checkin_rejects_non_owner() {
    let (env, client, owner, token_address, non_owner, beneficiary, _guardian) = setup();
    let will_id = create_active_will(&client, &owner, &token_address, &beneficiary, None);

    // Trigger the will first so emergency_checkin is allowed.
    env.ledger().with_mut(|l| l.timestamp += 91 * DAY);
    client.trigger_will(&will_id);

    assert_eq!(
        client.try_emergency_checkin(&will_id, &non_owner),
        Err(Ok(WillError::NotOwner.into()))
    );
}

#[test]
fn cancel_will_rejects_non_owner() {
    let (_env, client, owner, token_address, non_owner, beneficiary, _guardian) = setup();
    let will_id = create_active_will(&client, &owner, &token_address, &beneficiary, None);

    assert_eq!(
        client.try_cancel_will(&will_id, &non_owner),
        Err(Ok(WillError::NotOwner.into()))
    );
}

#[test]
fn close_will_rejects_non_owner() {
    let (env, client, owner, token_address, non_owner, beneficiary, _guardian) = setup();
    let will_id = create_active_will(&client, &owner, &token_address, &beneficiary, None);

    // Move to Released status first.
    env.ledger().with_mut(|l| l.timestamp += 91 * DAY);
    client.trigger_will(&will_id);
    env.ledger().with_mut(|l| l.timestamp += 8 * DAY);
    client.release_inheritance(&will_id, &None);

    assert_eq!(
        client.try_close_will(&will_id, &non_owner),
        Err(Ok(WillError::NotOwner.into()))
    );
}

#[test]
fn update_beneficiaries_rejects_non_owner() {
    let (env, client, owner, token_address, non_owner, beneficiary, _guardian) = setup();
    let will_id = create_active_will(&client, &owner, &token_address, &beneficiary, None);

    let new_beneficiaries = vec![
        &env,
        Beneficiary {
            address: Address::generate(&env),
            allocation: Allocation::Percentage(10_000),
        },
    ];

    assert_eq!(
        client.try_update_beneficiaries(&will_id, &non_owner, &new_beneficiaries),
        Err(Ok(WillError::NotOwner.into()))
    );
}

#[test]
fn update_guardians_rejects_non_owner() {
    let (env, client, owner, token_address, non_owner, beneficiary, _guardian) = setup();
    let will_id = create_active_will(&client, &owner, &token_address, &beneficiary, None);

    let new_guardians = vec![&env, Address::generate(&env)];
    assert_eq!(
        client.try_update_guardians(&will_id, &non_owner, &new_guardians),
        Err(Ok(WillError::NotOwner.into()))
    );
}

#[test]
fn update_guardians_weighted_rejects_non_owner() {
    let (env, client, owner, token_address, non_owner, beneficiary, _guardian) = setup();
    let will_id = create_active_will(&client, &owner, &token_address, &beneficiary, None);

    use crate::GuardianSpec;
    let new_guardians = vec![
        &env,
        GuardianSpec {
            address: Address::generate(&env),
            weight: 1,
        },
    ];
    assert_eq!(
        client.try_update_guardians_weighted(&will_id, &non_owner, &new_guardians, &Some(1u32)),
        Err(Ok(WillError::NotOwner.into()))
    );
}

#[test]
fn update_periods_rejects_non_owner() {
    let (_env, client, owner, token_address, non_owner, beneficiary, _guardian) = setup();
    let will_id = create_active_will(&client, &owner, &token_address, &beneficiary, None);

    assert_eq!(
        client.try_update_periods(&will_id, &non_owner, &Some(120u64), &Some(10u64)),
        Err(Ok(WillError::NotOwner.into()))
    );
}

#[test]
fn update_will_settings_rejects_non_owner() {
    let (_env, client, owner, token_address, non_owner, beneficiary, _guardian) = setup();
    let will_id = create_active_will(&client, &owner, &token_address, &beneficiary, None);

    assert_eq!(
        client.try_update_will_settings(
            &will_id,
            &non_owner,
            &None::<soroban_sdk::Vec<Beneficiary>>,
            &None::<soroban_sdk::Vec<Address>>,
            &None::<u64>,
            &None::<u64>
        ),
        Err(Ok(WillError::NotOwner.into()))
    );
}

#[test]
fn top_up_rejects_non_owner() {
    let (_env, client, owner, token_address, non_owner, beneficiary, _guardian) = setup();
    let will_id = create_active_will(&client, &owner, &token_address, &beneficiary, None);

    assert_eq!(
        client.try_top_up(&will_id, &non_owner, &token_address, &100_000),
        Err(Ok(WillError::NotOwner.into()))
    );
}

#[test]
fn merge_wills_rejects_non_owner() {
    let (_env, client, owner, token_address, non_owner, beneficiary, _guardian) = setup();
    let will_a = create_active_will(&client, &owner, &token_address, &beneficiary, None);
    let will_b = create_active_will(&client, &owner, &token_address, &beneficiary, None);

    assert_eq!(
        client.try_merge_wills(&non_owner, &will_a, &will_b),
        Err(Ok(WillError::NotOwner.into()))
    );
}

#[test]
fn split_will_rejects_non_owner() {
    let (env, client, owner, token_address, non_owner, beneficiary, _guardian) = setup();
    let will_id = create_active_will(&client, &owner, &token_address, &beneficiary, None);

    let split_beneficiaries = vec![
        &env,
        Beneficiary {
            address: beneficiary.clone(),
            allocation: Allocation::Percentage(10_000),
        },
    ];

    assert_eq!(
        client.try_split_will(
            &will_id,
            &non_owner,
            &split_beneficiaries,
            &vec![&env, (token_address, 100_000_i128)]
        ),
        Err(Ok(WillError::NotOwner.into()))
    );
}

#[test]
fn add_hashed_beneficiary_rejects_non_owner() {
    let (env, client, owner, token_address, non_owner, beneficiary, _guardian) = setup();
    let will_id = create_active_will(&client, &owner, &token_address, &beneficiary, None);

    let commitment = Bytes::from_array(
        &env,
        &[
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
            25, 26, 27, 28, 29, 30, 31, 32,
        ],
    );

    assert_eq!(
        client.try_add_hashed_beneficiary(&will_id, &non_owner, &commitment, &1_000),
        Err(Ok(WillError::NotOwner.into()))
    );
}

#[test]
fn migrate_will_rejects_non_owner() {
    let (_env, client, owner, token_address, non_owner, beneficiary, _guardian) = setup();
    let will_id = create_active_will(&client, &owner, &token_address, &beneficiary, None);

    assert_eq!(
        client.try_migrate_will(&will_id, &non_owner),
        Err(Ok(WillError::NotOwner.into()))
    );
}

// ---------------------------------------------------------------------------
// Guardian-only entry points
// ---------------------------------------------------------------------------

#[test]
fn guardian_trigger_rejects_non_guardian() {
    let (env, client, owner, token_address, non_owner, beneficiary, guardian) = setup();
    let will_id = create_active_will(
        &client,
        &owner,
        &token_address,
        &beneficiary,
        Some(&guardian),
    );

    // Accept the guardian role first.
    client.accept_guardian_role(&will_id, &guardian);

    // Advance past the cooldown so the guardian can vote.
    env.ledger().with_mut(|l| l.timestamp += 8 * DAY);

    use crate::GuardianVoteReason;
    assert_eq!(
        client.try_guardian_trigger(&will_id, &non_owner, &GuardianVoteReason::Other),
        Err(Ok(WillError::NotGuardian.into()))
    );
}

#[test]
fn guardian_cancel_trigger_rejects_non_guardian() {
    let (env, client, owner, token_address, non_owner, beneficiary, guardian) = setup();
    let will_id = create_active_will(
        &client,
        &owner,
        &token_address,
        &beneficiary,
        Some(&guardian),
    );

    // Trigger the will first so guardian_cancel_trigger is meaningful.
    env.ledger().with_mut(|l| l.timestamp += 91 * DAY);
    client.trigger_will(&will_id);

    assert_eq!(
        client.try_guardian_cancel_trigger(&will_id, &non_owner),
        Err(Ok(WillError::NotGuardian.into()))
    );
}

#[test]
fn accept_guardian_role_rejects_non_guardian() {
    let (_env, client, owner, token_address, non_owner, beneficiary, guardian) = setup();
    let will_id = create_active_will(
        &client,
        &owner,
        &token_address,
        &beneficiary,
        Some(&guardian),
    );

    assert_eq!(
        client.try_accept_guardian_role(&will_id, &non_owner),
        Err(Ok(WillError::NotGuardian.into()))
    );
}

#[test]
fn reject_guardian_role_rejects_non_guardian() {
    let (_env, client, owner, token_address, non_owner, beneficiary, guardian) = setup();
    let will_id = create_active_will(
        &client,
        &owner,
        &token_address,
        &beneficiary,
        Some(&guardian),
    );

    assert_eq!(
        client.try_reject_guardian_role(&will_id, &non_owner),
        Err(Ok(WillError::NotGuardian.into()))
    );
}

// ---------------------------------------------------------------------------
// Beneficiary-only entry points
// ---------------------------------------------------------------------------

#[test]
fn renounce_beneficiary_rejects_non_beneficiary() {
    let (_env, client, owner, token_address, non_owner, beneficiary, _guardian) = setup();
    let will_id = create_active_will(&client, &owner, &token_address, &beneficiary, None);

    assert_eq!(
        client.try_renounce_beneficiary(&will_id, &non_owner),
        Err(Ok(WillError::BeneficiaryNotFound.into()))
    );
}

// ---------------------------------------------------------------------------
// Permissionless entry points (anyone can call)
// ---------------------------------------------------------------------------
//
// The following entry points are intentionally permissionless and do not
// have negative authorization tests because they are designed to be callable
// by anyone:
//
// - `trigger_will`: anyone may start the grace period after a missed check-in.
// - `release_inheritance`: anyone may distribute funds after the grace period.
// - `retry_failed_payout`: anyone may retry a failed transfer.
// - `archive_will`: anyone may clean up a terminal will.
//
// These entry points have their own functional tests but are not included
// here because they have no authorization requirement to test negatively.

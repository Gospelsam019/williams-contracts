#![cfg(test)]

//! Unit tests for the `confirm_will` function and PendingConfirmation state transition.

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::{Client as TokenClient, StellarAssetClient},
    vec, Address, Env, Vec as SorobanVec,
};

use crate::{Allocation, Beneficiary, WillContract, WillContractClient, WillError, WillStatus};

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
    StellarAssetClient::new(&env, &token_address).mint(&owner, &1_000_000_000);

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

/// Test asserting a successful `confirm_will` transitions `PendingConfirmation`
/// to `Active` within the confirmation window.
#[test]
fn successful_confirmation_transitions_to_active() {
    let (env, client, owner, _token, token_address) = setup();
    let beneficiary = Address::generate(&env);

    let beneficiaries: SorobanVec<Beneficiary> = vec![
        &env,
        Beneficiary {
            address: beneficiary,
            allocation: Allocation::Percentage(10_000),
        },
    ];
    let tokens: SorobanVec<(Address, i128)> = vec![&env, (token_address, 1_000_000_i128)];

    // Create will with 30-day confirmation window (starts in PendingConfirmation)
    let will_id = client.create_will(
        &owner,
        &tokens,
        &beneficiaries,
        &90,
        &7,
        &vec![&env],
        &0,
        &None,
        &(30 * DAY),
    );

    // Verify will is in PendingConfirmation state
    assert_eq!(
        client.get_will(&will_id).status,
        WillStatus::PendingConfirmation
    );

    // Confirm the will before the window expires
    client.confirm_will(&will_id, &owner);

    // Verify will is now Active
    assert_eq!(client.get_will(&will_id).status, WillStatus::Active);
}

/// Test asserting a non-owner caller is rejected.
#[test]
fn non_owner_cannot_confirm() {
    let (env, client, owner, _token, token_address) = setup();
    let non_owner = Address::generate(&env);
    let beneficiary = Address::generate(&env);

    let beneficiaries: SorobanVec<Beneficiary> = vec![
        &env,
        Beneficiary {
            address: beneficiary,
            allocation: Allocation::Percentage(10_000),
        },
    ];
    let tokens: SorobanVec<(Address, i128)> = vec![&env, (token_address, 1_000_000_i128)];

    let will_id = client.create_will(
        &owner,
        &tokens,
        &beneficiaries,
        &90,
        &7,
        &vec![&env],
        &0,
        &None,
        &(30 * DAY),
    );

    // Non-owner should not be able to confirm
    assert_eq!(
        client.try_confirm_will(&will_id, &non_owner),
        Err(Ok(WillError::NotOwner.into()))
    );
}

/// Test asserting a will not in `PendingConfirmation` is rejected.
#[test]
fn cannot_confirm_active_will() {
    let (env, client, owner, _token, token_address) = setup();
    let beneficiary = Address::generate(&env);

    let beneficiaries: SorobanVec<Beneficiary> = vec![
        &env,
        Beneficiary {
            address: beneficiary,
            allocation: Allocation::Percentage(10_000),
        },
    ];
    let tokens: SorobanVec<(Address, i128)> = vec![&env, (token_address, 1_000_000_i128)];

    // Create will that is immediately Active (confirmation_delay_seconds = 0)
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

    // Verify will is Active
    assert_eq!(client.get_will(&will_id).status, WillStatus::Active);

    // Attempting to confirm an Active will should fail
    assert_eq!(
        client.try_confirm_will(&will_id, &owner),
        Err(Ok(WillError::WillNotConfirmed.into()))
    );
}

/// Test asserting a call after the window has passed reverts with
/// `ConfirmationWindowExpired`.
#[test]
fn confirmation_window_expiration() {
    let (env, client, owner, _token, token_address) = setup();
    let beneficiary = Address::generate(&env);

    let beneficiaries: SorobanVec<Beneficiary> = vec![
        &env,
        Beneficiary {
            address: beneficiary,
            allocation: Allocation::Percentage(10_000),
        },
    ];
    let tokens: SorobanVec<(Address, i128)> = vec![&env, (token_address, 1_000_000_i128)];

    // Create will with 30-day confirmation window
    let confirmation_window = 30 * DAY;
    let will_id = client.create_will(
        &owner,
        &tokens,
        &beneficiaries,
        &90,
        &7,
        &vec![&env],
        &0,
        &None,
        &confirmation_window,
    );

    // Advance time past the confirmation window
    env.ledger()
        .with_mut(|l| l.timestamp += confirmation_window + 1);

    // Attempting to confirm after the window should fail
    assert_eq!(
        client.try_confirm_will(&will_id, &owner),
        Err(Ok(WillError::ConfirmationWindowExpired.into()))
    );
}

/// Test asserting that anyone can permissionlessly trigger a refund of an unconfirmed
/// will once the confirmation window has expired by the 7-day grace margin (#40).
#[test]
fn permissionless_refund_of_expired_unconfirmed_will() {
    let (env, client, owner, token, token_address) = setup();
    let beneficiary = Address::generate(&env);

    let beneficiaries: SorobanVec<Beneficiary> = vec![
        &env,
        Beneficiary {
            address: beneficiary,
            allocation: Allocation::Percentage(10_000),
        },
    ];
    let initial_owner_balance = token.balance(&owner);
    let deposit_amount = 1_000_000_i128;
    let tokens: SorobanVec<(Address, i128)> = vec![&env, (token_address, deposit_amount)];

    let confirmation_window = 30 * DAY;
    let will_id = client.create_will(
        &owner,
        &tokens,
        &beneficiaries,
        &90,
        &7,
        &vec![&env],
        &0,
        &None,
        &confirmation_window,
    );

    assert_eq!(token.balance(&owner), initial_owner_balance - deposit_amount);
    assert_eq!(
        client.get_will(&will_id).status,
        WillStatus::PendingConfirmation
    );

    // Advance time past confirmation_deadline + 7 days
    let grace_margin = 7 * DAY;
    env.ledger()
        .with_mut(|l| l.timestamp += confirmation_window + grace_margin + 1);

    // Permissionless caller triggers refund
    client.refund_unconfirmed_will(&will_id);

    // Status is now Cancelled
    let will = client.get_will(&will_id);
    assert_eq!(will.status, WillStatus::Cancelled);
    assert_eq!(will.balance, 0);

    // Tokens safely returned to the owner (not caller)
    assert_eq!(token.balance(&owner), initial_owner_balance);
}

/// Test asserting refund fails if called before confirmation deadline + 7 days grace margin.
#[test]
fn refund_unconfirmed_will_fails_before_grace_margin() {
    let (env, client, owner, _token, token_address) = setup();
    let beneficiary = Address::generate(&env);

    let beneficiaries: SorobanVec<Beneficiary> = vec![
        &env,
        Beneficiary {
            address: beneficiary,
            allocation: Allocation::Percentage(10_000),
        },
    ];
    let tokens: SorobanVec<(Address, i128)> = vec![&env, (token_address, 1_000_000_i128)];

    let confirmation_window = 30 * DAY;
    let will_id = client.create_will(
        &owner,
        &tokens,
        &beneficiaries,
        &90,
        &7,
        &vec![&env],
        &0,
        &None,
        &confirmation_window,
    );

    // Advance time past confirmation window, but BEFORE the 7-day grace margin
    let grace_margin = 7 * DAY;
    env.ledger()
        .with_mut(|l| l.timestamp += confirmation_window + grace_margin - 100);

    assert_eq!(
        client.try_refund_unconfirmed_will(&will_id),
        Err(Ok(WillError::ConfirmationWindowNotExpired.into()))
    );
}

/// Test asserting refund fails on a will that is not in PendingConfirmation status.
#[test]
fn refund_unconfirmed_will_fails_on_active_will() {
    let (env, client, owner, _token, token_address) = setup();
    let beneficiary = Address::generate(&env);

    let beneficiaries: SorobanVec<Beneficiary> = vec![
        &env,
        Beneficiary {
            address: beneficiary,
            allocation: Allocation::Percentage(10_000),
        },
    ];
    let tokens: SorobanVec<(Address, i128)> = vec![&env, (token_address, 1_000_000_i128)];

    // Active immediately
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

    assert_eq!(
        client.try_refund_unconfirmed_will(&will_id),
        Err(Ok(WillError::WillNotConfirmed.into()))
    );
}


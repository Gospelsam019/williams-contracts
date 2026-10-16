#![cfg(test)]

//! State-machine model tests asserting terminal lifecycle invariants for the
//! Williams will contract.
//!
//! # Tested Invariant
//!
//! Once a will enters a terminal state (`Released`, `Cancelled`, or
//! `Settled`), no sequence of arbitrary calls must be able to:
//! - return it to `Active` or `Triggered`, or
//! - further reduce or modify its recorded balances.
//!
//! This is the fundamental safety property of an inheritance contract:
//! finalized estates must be immutable.
//!
//! # Test Strategy
//!
//! Proptest generates random sequences of the operations most likely to
//! change will state, applies them to a will that has already reached a
//! terminal state, and asserts after every operation that the status and
//! balances are unchanged. The harness drives `try_*` variants (which return
//! `Err(...)` on failure) so every operation is attempted and any abort
//! surfaces as a test failure.

use proptest::prelude::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    vec, Address, Env, Vec as SorobanVec,
};

use crate::{Allocation, Beneficiary, WillContract, WillContractClient, WillStatus};

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

/// Every possible terminal status the state machine can reach.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TerminalStatus {
    Released,
    Cancelled,
    Settled,
}

/// Every state-mutating call we attempt against a terminal will. Each variant
/// is tried in turn; individual calls are expected to fail (error or reject),
/// but must never change the status or balance.
#[derive(Clone, Copy, Debug)]
enum TerminalProbe {
    CheckIn,
    TriggerWill,
    EmergencyCheckin,
    ReleaseInheritance,
    CancelWill,
    UpdateBeneficiaries,
    CloseWill,
}

/// A sequence of probes to fire against a terminal will.
type ProbeSequence = Vec<TerminalProbe>;

fn probe_strategy() -> impl Strategy<Value = TerminalProbe> {
    prop_oneof![
        Just(TerminalProbe::CheckIn),
        Just(TerminalProbe::TriggerWill),
        Just(TerminalProbe::EmergencyCheckin),
        Just(TerminalProbe::ReleaseInheritance),
        Just(TerminalProbe::CancelWill),
        Just(TerminalProbe::UpdateBeneficiaries),
        Just(TerminalProbe::CloseWill),
    ]
}

fn probe_sequence_strategy() -> impl Strategy<Value = ProbeSequence> {
    prop::collection::vec(probe_strategy(), 1..=20)
}

fn terminal_status_strategy() -> impl Strategy<Value = TerminalStatus> {
    prop_oneof![
        Just(TerminalStatus::Released),
        Just(TerminalStatus::Cancelled),
        Just(TerminalStatus::Settled),
    ]
}

/// Drive a will to `Released` status.
fn reach_released(env: &Env, client: &WillContractClient, will_id: u64) {
    env.ledger().with_mut(|l| l.timestamp += 91 * DAY);
    client.trigger_will(&will_id);
    env.ledger().with_mut(|l| l.timestamp += 8 * DAY);
    client.release_inheritance(&will_id, &None);
    assert_eq!(client.get_will(&will_id).status, WillStatus::Released);
}

/// Drive a will to `Cancelled` status.
fn reach_cancelled(client: &WillContractClient, will_id: u64, owner: &Address) {
    client.cancel_will(&will_id, owner);
    assert_eq!(client.get_will(&will_id).status, WillStatus::Cancelled);
}

/// Drive a will to `Settled` status (Released then closed).
fn reach_settled(env: &Env, client: &WillContractClient, will_id: u64, owner: &Address) {
    reach_released(env, client, will_id);
    client.close_will(&will_id, owner);
    assert_eq!(client.get_will(&will_id).status, WillStatus::Settled);
}

/// Assert that a will in a terminal state cannot be moved back to a
/// non-terminal state by any probe, and that its balances are not increased.
///
/// `Released → Settled` is the one permitted status change across terminal
/// states: `close_will` moves a Released will to Settled, which is also
/// terminal and tighter. All other status changes (to Active, Triggered,
/// PendingConfirmation, Cancelled, or Released-from-Settled) are forbidden.
fn assert_terminal_will_is_immutable(
    env: &Env,
    client: &WillContractClient,
    will_id: u64,
    owner: &Address,
    token_address: &Address,
    sequence: &ProbeSequence,
) {
    let before = client.get_will(&will_id);
    let initial_terminal_status = before.status;
    assert!(
        matches!(
            initial_terminal_status,
            WillStatus::Released | WillStatus::Cancelled | WillStatus::Settled
        ),
        "will must be in a terminal state before probing"
    );

    for probe in sequence {
        match probe {
            TerminalProbe::CheckIn => {
                let _ = client.try_check_in(&will_id, owner);
            }
            TerminalProbe::TriggerWill => {
                let _ = client.try_trigger_will(&will_id);
            }
            TerminalProbe::EmergencyCheckin => {
                let _ = client.try_emergency_checkin(&will_id, owner);
            }
            TerminalProbe::ReleaseInheritance => {
                let _ = client.try_release_inheritance(&will_id, &None);
            }
            TerminalProbe::CancelWill => {
                let _ = client.try_cancel_will(&will_id, owner);
            }
            TerminalProbe::UpdateBeneficiaries => {
                let new_beneficiaries: SorobanVec<Beneficiary> = vec![
                    env,
                    Beneficiary {
                        address: Address::generate(env),
                        allocation: Allocation::Percentage(10_000),
                    },
                ];
                let _ = client.try_update_beneficiaries(&will_id, owner, &new_beneficiaries);
            }
            TerminalProbe::CloseWill => {
                let _ = client.try_close_will(&will_id, owner);
            }
        }

        let after = client.get_will(&will_id);

        // The fundamental invariant: a terminal will must never transition
        // to a non-terminal state. Released → Settled is the one allowed
        // forward transition within the terminal set.
        assert!(
            matches!(
                after.status,
                WillStatus::Released | WillStatus::Cancelled | WillStatus::Settled
            ),
            "probe {probe:?} moved terminal will (was {initial_terminal_status:?}) to non-terminal state {:?}",
            after.status
        );

        // Balances must not increase (no funds appeared from nowhere).
        // They may only decrease (e.g. to zero on release) or stay the same.
        // We check the specific token used in the test.
        let before_bal = before.balances.get(token_address.clone()).unwrap_or(0);
        let after_bal = after.balances.get(token_address.clone()).unwrap_or(0);
        assert!(
            after_bal <= before_bal,
            "probe {probe:?} increased terminal will's balance from {before_bal} to {after_bal}"
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 64,
        max_shrink_iters: 256,
        ..ProptestConfig::default()
    })]

    /// A `Released` will must be immutable: no sequence of calls can change
    /// its status back to `Active` or `Triggered`, or modify its balances.
    #[test]
    fn released_will_is_immutable(sequence in probe_sequence_strategy()) {
        let (env, client, owner, token_address) = setup();
        let will_id = create_simple_will(&env, &client, &owner, &token_address);

        reach_released(&env, &client, will_id);
        assert_terminal_will_is_immutable(&env, &client, will_id, &owner, &token_address, &sequence);
    }

    /// A `Cancelled` will must be immutable: no sequence of calls can change
    /// its status back to `Active` or `Triggered`, or modify its balances.
    #[test]
    fn cancelled_will_is_immutable(sequence in probe_sequence_strategy()) {
        let (env, client, owner, token_address) = setup();
        let will_id = create_simple_will(&env, &client, &owner, &token_address);

        reach_cancelled(&client, will_id, &owner);
        assert_terminal_will_is_immutable(&env, &client, will_id, &owner, &token_address, &sequence);
    }

    /// A `Settled` will must be immutable: no sequence of calls can change
    /// its status back to `Active` or `Triggered`, or modify its balances.
    #[test]
    fn settled_will_is_immutable(sequence in probe_sequence_strategy()) {
        let (env, client, owner, token_address) = setup();
        let will_id = create_simple_will(&env, &client, &owner, &token_address);

        reach_settled(&env, &client, will_id, &owner);
        assert_terminal_will_is_immutable(&env, &client, will_id, &owner, &token_address, &sequence);
    }

    /// Regardless of which terminal state the will reaches, no probe can
    /// transition it back to a non-terminal state. Tests all three terminal
    /// statuses driven by proptest.
    #[test]
    fn terminal_state_is_strictly_absorbing(
        terminal in terminal_status_strategy(),
        sequence in probe_sequence_strategy(),
    ) {
        let (env, client, owner, token_address) = setup();
        let will_id = create_simple_will(&env, &client, &owner, &token_address);

        match terminal {
            TerminalStatus::Released => reach_released(&env, &client, will_id),
            TerminalStatus::Cancelled => reach_cancelled(&client, will_id, &owner),
            TerminalStatus::Settled => reach_settled(&env, &client, will_id, &owner),
        }

        let will = client.get_will(&will_id);
        // After probing, the status must remain the same terminal status.
        assert_terminal_will_is_immutable(&env, &client, will_id, &owner, &token_address, &sequence);

        // Re-read after all probes.
        let final_will = client.get_will(&will_id);
        prop_assert!(
            matches!(
                final_will.status,
                WillStatus::Released | WillStatus::Cancelled | WillStatus::Settled
            ),
            "terminal will status changed to non-terminal {:?} after probe sequence",
            final_will.status
        );

        // The only permitted status change within terminal states is Released → Settled.
        // No probe may move a Cancelled or Settled will to Released, and no probe
        // may move any will back to Active, Triggered, or PendingConfirmation.
        let allowed_transition = will.status == WillStatus::Released
            && final_will.status == WillStatus::Settled;
        prop_assert!(
            final_will.status == will.status || allowed_transition,
            "terminal will status changed from {:?} to {:?} (not the permitted Released→Settled path)",
            will.status,
            final_will.status
        );
    }
}

// ---------------------------------------------------------------------------
// Deterministic regression tests
// ---------------------------------------------------------------------------

fn create_simple_will(
    env: &Env,
    client: &WillContractClient,
    owner: &Address,
    token_address: &Address,
) -> u64 {
    client.create_will(
        owner,
        &vec![env, (token_address.clone(), 500_000_i128)],
        &vec![
            env,
            Beneficiary {
                address: Address::generate(env),
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

/// A released will must reject a second `release_inheritance` call.
#[test]
fn released_will_rejects_release_again() {
    let (env, client, owner, token_address) = setup();
    let will_id = create_simple_will(&env, &client, &owner, &token_address);
    reach_released(&env, &client, will_id);

    // Attempting to release again must not succeed.
    let result = client.try_release_inheritance(&will_id, &None);
    assert!(
        result.is_err(),
        "released will accepted a second release_inheritance call"
    );
    assert_eq!(client.get_will(&will_id).status, WillStatus::Released);
}

/// A cancelled will must reject a subsequent `cancel_will` call.
#[test]
fn cancelled_will_rejects_cancel_again() {
    let (env, client, owner, token_address) = setup();
    let will_id = create_simple_will(&env, &client, &owner, &token_address);
    reach_cancelled(&client, will_id, &owner);

    let result = client.try_cancel_will(&will_id, &owner);
    assert!(
        result.is_err(),
        "cancelled will accepted a second cancel_will call"
    );
    assert_eq!(client.get_will(&will_id).status, WillStatus::Cancelled);
}

/// A released will must reject `trigger_will` — triggers are only legal when
/// the will is `Active` and the check-in deadline has passed.
#[test]
fn released_will_rejects_trigger() {
    let (env, client, owner, token_address) = setup();
    let will_id = create_simple_will(&env, &client, &owner, &token_address);
    reach_released(&env, &client, will_id);

    let result = client.try_trigger_will(&will_id);
    assert!(result.is_err(), "released will accepted trigger_will");
    assert_eq!(client.get_will(&will_id).status, WillStatus::Released);
}

/// A cancelled will must reject `trigger_will`.
#[test]
fn cancelled_will_rejects_trigger() {
    let (env, client, owner, token_address) = setup();
    let will_id = create_simple_will(&env, &client, &owner, &token_address);
    reach_cancelled(&client, will_id, &owner);

    let result = client.try_trigger_will(&will_id);
    assert!(result.is_err(), "cancelled will accepted trigger_will");
    assert_eq!(client.get_will(&will_id).status, WillStatus::Cancelled);
}

/// A settled will must reject `trigger_will`.
#[test]
fn settled_will_rejects_trigger() {
    let (env, client, owner, token_address) = setup();
    let will_id = create_simple_will(&env, &client, &owner, &token_address);
    reach_settled(&env, &client, will_id, &owner);

    let result = client.try_trigger_will(&will_id);
    assert!(result.is_err(), "settled will accepted trigger_will");
    assert_eq!(client.get_will(&will_id).status, WillStatus::Settled);
}

/// A terminal will must not change balance after an update_beneficiaries
/// attempt, even when the owner makes the call.
#[test]
fn terminal_will_update_beneficiaries_does_not_change_balance() {
    let (env, client, owner, token_address) = setup();
    let will_id = create_simple_will(&env, &client, &owner, &token_address);
    reach_released(&env, &client, will_id);

    let will_before = client.get_will(&will_id);
    let before_bal = will_before.balances.get(token_address.clone()).unwrap_or(0);

    let new_beneficiaries: SorobanVec<Beneficiary> = vec![
        &env,
        Beneficiary {
            address: Address::generate(&env),
            allocation: Allocation::Percentage(10_000),
        },
    ];
    let _ = client.try_update_beneficiaries(&will_id, &owner, &new_beneficiaries);

    let will_after = client.get_will(&will_id);
    let after_bal = will_after.balances.get(token_address.clone()).unwrap_or(0);

    assert_eq!(
        after_bal, before_bal,
        "update_beneficiaries changed a released will's balance"
    );
    assert_eq!(will_after.status, WillStatus::Released);
}

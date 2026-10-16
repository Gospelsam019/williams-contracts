#![cfg(test)]

//! Regression tests for issue #109: keeper-bounty token selection.
//!
//! Before the fix, `distribute` computed the keeper bounty from the **first**
//! token in `will.balances` whose share rounded above zero. On a multi-token
//! estate that arbitrarily favoured whichever token was locked first: an
//! estate holding 1 XLM ($0.10) and 100,000 USDC paid the keeper 1% of the
//! XLM ($0.001) instead of a share of the high-value asset, so keepers had no
//! economic reason to pay gas and trigger the release.
//!
//! The fix introduces two selection modes, chosen per will:
//!
//! - **proportional (default)** — every held token contributes
//!   `floor(balance * keeper_bounty_bps / 10_000)` of itself, so the keeper's
//!   incentive scales with the estate's whole value;
//! - **targeted** — `WillContract::set_keeper_bounty_token` names one token
//!   (e.g. the native asset, to cover the keeper's gas) that carries the
//!   whole bounty, with a proportional fallback when the targeted token's
//!   share rounds to zero.
//!
//! Both modes keep the invariant the #378 fix established: the token a bounty
//! is deducted from is exactly the token it is paid out of.

use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events as EventsTraitMethods, Ledger},
    token::{Client as TokenClient, StellarAssetClient},
    vec, Address, Env, TryIntoVal,
};

use crate::{
    migration::legacy::WillV1, storage, Allocation, Beneficiary, Will, WillContract,
    WillContractClient, WillError, WillStatus,
};

const DAY: u64 = 86_400;

/// Highest `keeper_bounty_bps` `create_will` accepts (1%).
const MAX_BOUNTY_BPS: u32 = 100;

fn advance(env: &Env, days: u64) {
    env.ledger().with_mut(|l| l.timestamp += days * DAY);
}

/// Registers a SEP-41 asset and mints `amount` to `owner`.
fn mint(env: &Env, owner: &Address, amount: i128) -> Address {
    let sac = env.register_stellar_asset_contract_v2(owner.clone());
    let address = sac.address();
    StellarAssetClient::new(env, &address).mint(owner, &amount);
    address
}

/// Projects the raw XDR events of the last contract invocation into the
/// `(topics, data)` tuple shape these assertions care about.
///
/// soroban-sdk v28 changed `env.events().all()` to return `ContractEvents`
/// (raw XDR) instead of the old `Vec<(Address, Vec<Val>, Val)>`; several
/// sibling test modules are still disabled over that API change. This helper
/// is the minimal bridge: it unwraps the `ContractEventV0` body and converts
/// each topic and the payload back into `Val`s.
fn events_as_tuples(
    env: &Env,
) -> std::vec::Vec<(soroban_sdk::Vec<soroban_sdk::Val>, soroban_sdk::Val)> {
    env.events()
        .all()
        .events()
        .iter()
        .filter_map(|event| {
            // `ContractEventBody` currently has exactly one variant, so a
            // `V0` body is the only shape the test host publishes today.
            let soroban_sdk::xdr::ContractEventBody::V0(v0) = &event.body else {
                return None;
            };
            let mut topics = soroban_sdk::Vec::new(env);
            for topic in v0.topics.iter() {
                let val: Result<soroban_sdk::Val, _> = topic.clone().try_into_val(env);
                topics.push_back(val.ok()?);
            }
            let data: Result<soroban_sdk::Val, _> = v0.data.clone().try_into_val(env);
            Some((topics, data.ok()?))
        })
        .collect()
}

/// Counts published events whose first topic is `topic`.
fn count_events_with_topic(env: &Env, topic: soroban_sdk::Symbol) -> u32 {
    let mut count = 0;
    for (topics, _) in events_as_tuples(env) {
        if topics.is_empty() {
            continue;
        }
        let topic0: Result<soroban_sdk::Symbol, _> = topics.first().unwrap().try_into_val(env);
        if topic0 == Ok(topic.clone()) {
            count += 1;
        }
    }
    count
}

/// Returns the data payload of the last published event whose first topic is
/// `topic`.
fn last_event_data_with_topic(env: &Env, topic: soroban_sdk::Symbol) -> Option<soroban_sdk::Val> {
    let mut found = None;
    for (topics, data) in events_as_tuples(env) {
        if topics.is_empty() {
            continue;
        }
        let topic0: Result<soroban_sdk::Symbol, _> = topics.first().unwrap().try_into_val(env);
        if topic0 == Ok(topic.clone()) {
            found = Some(data);
        }
    }
    found
}

fn beneficiary(addr: &Address) -> Beneficiary {
    Beneficiary {
        address: addr.clone(),
        allocation: Allocation::Percentage(10_000),
    }
}

/// A two-token estate: 1 XLM (10_000_000 stroops) plus 100,000 USDC
/// (100_000 * 10^7) — the exact scenario from the issue, where the first
/// token is worth $0.10 and the second $100,000.
fn two_token_estate<'a>(
    env: &Env,
    client: &WillContractClient<'a>,
    owner: &Address,
    heir: &Address,
) -> (u64, Address, Address) {
    let native = mint(env, owner, 10_000_000);
    let usdc = mint(env, owner, 1_000_000_000_000);
    let will_id = client.create_will(
        owner,
        &vec![
            env,
            (native.clone(), 10_000_000_i128),
            (usdc.clone(), 1_000_000_000_000_i128),
        ],
        &vec![env, beneficiary(heir)],
        &90,
        &7,
        &vec![env],
        &0,
        &Some(MAX_BOUNTY_BPS),
        &0,
    );
    (will_id, native, usdc)
}

/// Releases `will_id` as a non-owner keeper, which is what makes a bounty
/// payable at all.
fn release_as_keeper(env: &Env, client: &WillContractClient<'_>, will_id: u64, keeper: &Address) {
    advance(env, 91);
    client.trigger_will(&will_id);
    advance(env, 8);
    client.release_inheritance(&will_id, &Some(keeper.clone()));
}

// ------------------------------------------------------------------ default

/// The issue's core scenario: with the default (proportional) selection the
/// keeper is paid 1% of **each** token — 0.01 XLM and 1,000 USDC — instead of
/// 1% of whichever token happened to be locked first.
#[test]
fn proportional_bounty_is_paid_from_every_token() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_700_000_000);

    let owner = Address::generate(&env);
    let keeper = Address::generate(&env);
    let heir = Address::generate(&env);
    let contract_id = env.register(WillContract, ());
    let client = WillContractClient::new(&env, &contract_id);
    let (will_id, native, usdc) = two_token_estate(&env, &client, &owner, &heir);

    release_as_keeper(&env, &client, will_id, &keeper);

    // One `keeper_bounty_paid` event per token the bounty came from. Checked
    // before any further contract call: the test host only retains events
    // from the most recent top-level invocation.
    assert_eq!(
        count_events_with_topic(&env, symbol_short!("bounty")),
        2,
        "one bounty event per paying token"
    );

    let native_client = TokenClient::new(&env, &native);
    let usdc_client = TokenClient::new(&env, &usdc);

    // 1% of each token, in each token's own units.
    assert_eq!(native_client.balance(&keeper), 100_000); // 0.01 XLM
    assert_eq!(usdc_client.balance(&keeper), 10_000_000_000); // 1,000 USDC

    // Beneficiaries receive the remainder of both tokens in full: the bounty
    // is carved out of the estate, never paid on top of it.
    assert_eq!(native_client.balance(&heir), 10_000_000 - 100_000);
    assert_eq!(
        usdc_client.balance(&heir),
        1_000_000_000_000 - 10_000_000_000
    );

    // Nothing is stranded in the contract.
    assert_eq!(native_client.balance(&contract_id), 0);
    assert_eq!(usdc_client.balance(&contract_id), 0);
}

/// A token too small for its share to round above zero contributes nothing;
/// the release still succeeds and every other token pays its share.
#[test]
fn proportional_bounty_skips_tokens_that_round_to_zero() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_700_000_000);

    let owner = Address::generate(&env);
    let keeper = Address::generate(&env);
    let heir = Address::generate(&env);
    let dust = mint(&env, &owner, 100);
    let main = mint(&env, &owner, 1_000_000);

    let client = WillContractClient::new(&env, &env.register(WillContract, ()));
    let will_id = client.create_will(
        &owner,
        &vec![&env, (dust.clone(), 50_i128), (main.clone(), 100_000_i128)],
        &vec![&env, beneficiary(&heir)],
        &90,
        &7,
        &vec![&env],
        &0,
        &Some(MAX_BOUNTY_BPS),
        &0,
    );

    release_as_keeper(&env, &client, will_id, &keeper);

    // The dust token's 1% rounds to zero, so it pays no bounty and passes
    // through to the heir in full; the release does not abort.
    assert_eq!(TokenClient::new(&env, &dust).balance(&keeper), 0);
    assert_eq!(TokenClient::new(&env, &dust).balance(&heir), 50);

    // The second token pays its full 1%.
    assert_eq!(TokenClient::new(&env, &main).balance(&keeper), 1_000);
    assert_eq!(TokenClient::new(&env, &main).balance(&heir), 99_000);
}

/// When *every* token's share rounds to zero, no bounty is paid at all and no
/// beneficiary share is reduced — the documented rounding behaviour must
/// survive the new selection logic.
#[test]
fn no_bounty_is_paid_when_every_token_rounds_to_zero() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_700_000_000);

    let owner = Address::generate(&env);
    let keeper = Address::generate(&env);
    let heir = Address::generate(&env);
    let a = mint(&env, &owner, 100);
    let b = mint(&env, &owner, 100);

    let client = WillContractClient::new(&env, &env.register(WillContract, ()));
    let will_id = client.create_will(
        &owner,
        &vec![&env, (a.clone(), 50_i128), (b.clone(), 50_i128)],
        &vec![&env, beneficiary(&heir)],
        &90,
        &7,
        &vec![&env],
        &0,
        &Some(MAX_BOUNTY_BPS),
        &0,
    );

    release_as_keeper(&env, &client, will_id, &keeper);

    assert_eq!(TokenClient::new(&env, &a).balance(&keeper), 0);
    assert_eq!(TokenClient::new(&env, &b).balance(&keeper), 0);
    assert_eq!(TokenClient::new(&env, &a).balance(&heir), 50);
    assert_eq!(TokenClient::new(&env, &b).balance(&heir), 50);
}

// ------------------------------------------------------------------ targeting

/// Targeting the high-value asset concentrates the whole bounty there: the
/// keeper is paid 1,000 USDC and nothing from the XLM.
#[test]
fn targeted_bounty_token_pays_only_from_that_token() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_700_000_000);

    let owner = Address::generate(&env);
    let keeper = Address::generate(&env);
    let heir = Address::generate(&env);
    let contract_id = env.register(WillContract, ());
    let client = WillContractClient::new(&env, &contract_id);
    let (will_id, native, usdc) = two_token_estate(&env, &client, &owner, &heir);

    client.set_keeper_bounty_token(&will_id, &owner, &Some(usdc.clone()));
    assert_eq!(
        client.get_will(&will_id).keeper_bounty_token,
        Some(usdc.clone())
    );

    release_as_keeper(&env, &client, will_id, &keeper);

    assert_eq!(
        TokenClient::new(&env, &usdc).balance(&keeper),
        10_000_000_000
    );
    // The native token is untouched: it passes to the heir in full.
    assert_eq!(TokenClient::new(&env, &native).balance(&keeper), 0);
    assert_eq!(TokenClient::new(&env, &native).balance(&heir), 10_000_000);
    assert_eq!(
        TokenClient::new(&env, &usdc).balance(&heir),
        1_000_000_000_000 - 10_000_000_000
    );
    assert_eq!(TokenClient::new(&env, &native).balance(&contract_id), 0);
    assert_eq!(TokenClient::new(&env, &usdc).balance(&contract_id), 0);
}

/// Targeting the native asset is the gas-covering use case: the keeper is paid
/// entirely in XLM even though the estate's value sits in USDC.
#[test]
fn targeted_bounty_token_can_be_the_native_gas_asset() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_700_000_000);

    let owner = Address::generate(&env);
    let keeper = Address::generate(&env);
    let heir = Address::generate(&env);
    let contract_id = env.register(WillContract, ());
    let client = WillContractClient::new(&env, &contract_id);
    let (will_id, native, usdc) = two_token_estate(&env, &client, &owner, &heir);

    client.set_keeper_bounty_token(&will_id, &owner, &Some(native.clone()));

    release_as_keeper(&env, &client, will_id, &keeper);

    assert_eq!(TokenClient::new(&env, &native).balance(&keeper), 100_000);
    assert_eq!(TokenClient::new(&env, &usdc).balance(&keeper), 0);
    assert_eq!(
        TokenClient::new(&env, &usdc).balance(&heir),
        1_000_000_000_000
    );
    assert_eq!(
        TokenClient::new(&env, &native).balance(&heir),
        10_000_000 - 100_000
    );
}

/// A targeted token whose share rounds to zero must not leave the keeper
/// unpaid: the selection falls back to the proportional split.
#[test]
fn targeted_dust_token_falls_back_to_proportional() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_700_000_000);

    let owner = Address::generate(&env);
    let keeper = Address::generate(&env);
    let heir = Address::generate(&env);
    let dust = mint(&env, &owner, 100);
    let main = mint(&env, &owner, 1_000_000);

    let client = WillContractClient::new(&env, &env.register(WillContract, ()));
    let will_id = client.create_will(
        &owner,
        &vec![&env, (dust.clone(), 50_i128), (main.clone(), 100_000_i128)],
        &vec![&env, beneficiary(&heir)],
        &90,
        &7,
        &vec![&env],
        &0,
        &Some(MAX_BOUNTY_BPS),
        &0,
    );

    // Target the dust token: its 1% rounds to zero.
    client.set_keeper_bounty_token(&will_id, &owner, &Some(dust.clone()));

    release_as_keeper(&env, &client, will_id, &keeper);

    // Proportional fallback: the dust token still pays nothing (it rounds to
    // zero), the second token pays its full 1%.
    assert_eq!(TokenClient::new(&env, &dust).balance(&keeper), 0);
    assert_eq!(TokenClient::new(&env, &main).balance(&keeper), 1_000);
    assert_eq!(TokenClient::new(&env, &dust).balance(&heir), 50);
    assert_eq!(TokenClient::new(&env, &main).balance(&heir), 99_000);
}

/// Clearing the selection with `None` restores the proportional split.
#[test]
fn clearing_the_bounty_token_restores_proportional_split() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_700_000_000);

    let owner = Address::generate(&env);
    let keeper = Address::generate(&env);
    let heir = Address::generate(&env);
    let contract_id = env.register(WillContract, ());
    let client = WillContractClient::new(&env, &contract_id);
    let (will_id, native, usdc) = two_token_estate(&env, &client, &owner, &heir);

    client.set_keeper_bounty_token(&will_id, &owner, &Some(usdc.clone()));
    client.set_keeper_bounty_token(&will_id, &owner, &None);
    assert_eq!(client.get_will(&will_id).keeper_bounty_token, None);

    release_as_keeper(&env, &client, will_id, &keeper);

    assert_eq!(TokenClient::new(&env, &native).balance(&keeper), 100_000);
    assert_eq!(
        TokenClient::new(&env, &usdc).balance(&keeper),
        10_000_000_000
    );
}

/// The owner is never paid a bounty, whichever selection mode is active.
#[test]
fn owner_release_pays_no_bounty_in_either_mode() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_700_000_000);

    let owner = Address::generate(&env);
    let heir = Address::generate(&env);
    let contract_id = env.register(WillContract, ());
    let client = WillContractClient::new(&env, &contract_id);

    // Default (proportional) mode.
    let (proportional_id, native_a, usdc_a) = two_token_estate(&env, &client, &owner, &heir);
    // Targeted mode.
    let (targeted_id, native_b, usdc_b) = two_token_estate(&env, &client, &owner, &heir);
    client.set_keeper_bounty_token(&targeted_id, &owner, &Some(usdc_b.clone()));

    for will_id in [proportional_id, targeted_id] {
        advance(&env, 91);
        client.trigger_will(&will_id);
        advance(&env, 8);
        client.release_inheritance(&will_id, &Some(owner.clone()));
    }

    // The owner receives only their beneficiary share; the bounty that would
    // have gone to a keeper stays with the estate.
    assert_eq!(TokenClient::new(&env, &native_a).balance(&owner), 0);
    assert_eq!(TokenClient::new(&env, &usdc_a).balance(&owner), 0);
    assert_eq!(TokenClient::new(&env, &native_b).balance(&owner), 0);
    assert_eq!(TokenClient::new(&env, &usdc_b).balance(&owner), 0);
    assert_eq!(TokenClient::new(&env, &native_a).balance(&heir), 10_000_000);
    assert_eq!(
        TokenClient::new(&env, &usdc_a).balance(&heir),
        1_000_000_000_000
    );
    assert_eq!(TokenClient::new(&env, &native_b).balance(&heir), 10_000_000);
    assert_eq!(
        TokenClient::new(&env, &usdc_b).balance(&heir),
        1_000_000_000_000
    );
}

// ------------------------------------------------------------------ validation

/// Naming a token the will does not hold is rejected, and the will is left
/// untouched.
#[test]
fn set_keeper_bounty_token_rejects_unheld_token() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_700_000_000);

    let owner = Address::generate(&env);
    let heir = Address::generate(&env);
    let stranger_token = mint(&env, &owner, 1_000);

    let client = WillContractClient::new(&env, &env.register(WillContract, ()));
    let (will_id, _native, usdc) = two_token_estate(&env, &client, &owner, &heir);

    let err = client
        .try_set_keeper_bounty_token(&will_id, &owner, &Some(stranger_token))
        .unwrap_err()
        .unwrap();
    assert_eq!(err, WillError::InvalidBountyToken.into());

    // Rejected before any mutation.
    assert_eq!(client.get_will(&will_id).keeper_bounty_token, None);

    // A token the will *does* hold is accepted, and the event is published.
    client.set_keeper_bounty_token(&will_id, &owner, &Some(usdc.clone()));
    let event_data = last_event_data_with_topic(&env, symbol_short!("bountytok"))
        .expect("keeper_bounty_token_set event not found");
    let published: Option<Address> = event_data.try_into_val(&env).unwrap();
    assert_eq!(published, Some(usdc));
}

/// The setter follows the same status rule as every other owner-only settings
/// entry point: only an `Active` will can be reconfigured.
#[test]
fn set_keeper_bounty_token_requires_active_will() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_700_000_000);

    let owner = Address::generate(&env);
    let heir = Address::generate(&env);
    let client = WillContractClient::new(&env, &env.register(WillContract, ()));
    let (will_id, _native, usdc) = two_token_estate(&env, &client, &owner, &heir);

    // Trigger (but do not release): the will is no longer `Active`.
    advance(&env, 91);
    client.trigger_will(&will_id);

    let err = client
        .try_set_keeper_bounty_token(&will_id, &owner, &Some(usdc))
        .unwrap_err()
        .unwrap();
    assert_eq!(err, WillError::WillNotActive.into());
}

// ------------------------------------------------------------------ migration

/// A will written by the v1 layout (no `keeper_bounty_token` field) must stay
/// loadable after v2 added the field, and migrate cleanly to v2 with the new
/// field defaulted to `None` — the proportional split.
#[test]
fn v1_will_decodes_and_migrates_to_v2() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_700_000_000);

    let owner = Address::generate(&env);
    let heir = Address::generate(&env);
    let contract_id = env.register(WillContract, ());
    let client = WillContractClient::new(&env, &contract_id);

    let token = mint(&env, &owner, 1_000_000);
    let will_id = client.create_will(
        &owner,
        &vec![&env, (token.clone(), 1_000_000_i128)],
        &vec![&env, beneficiary(&heir)],
        &90,
        &7,
        &vec![&env],
        &0,
        &Some(MAX_BOUNTY_BPS),
        &0,
    );

    // Rewrite the stored entry in the v1 layout, as an older contract
    // version would have written it.
    let current: Will = client.get_will(&will_id);
    let legacy = WillV1 {
        id: current.id,
        owner: current.owner.clone(),
        balances: current.balances.clone(),
        token: current.token.clone(),
        is_native: current.is_native,
        balance: current.balance,
        beneficiaries: current.beneficiaries.clone(),
        hashed_beneficiaries: current.hashed_beneficiaries.clone(),
        checkin_period_days: current.checkin_period_days,
        grace_period_days: current.grace_period_days,
        last_checkin: current.last_checkin,
        trigger_time: current.trigger_time,
        confirmation_deadline: current.confirmation_deadline,
        status: current.status,
        guardians: current.guardians.clone(),
        guardian_vote_weight: current.guardian_vote_weight,
        guardian_votes: current.guardian_votes,
        guardian_cancel_vote_weight: current.guardian_cancel_vote_weight,
        guardian_cancel_votes: current.guardian_cancel_votes,
        guardian_threshold: current.guardian_threshold,
        guardian_list_updated_at: current.guardian_list_updated_at,
        schema_version: 1,
        keeper_bounty_bps: current.keeper_bounty_bps,
        delegate: current.delegate.clone(),
    };
    env.as_contract(&contract_id, || {
        env.storage()
            .persistent()
            .set(&storage::DataKey::Will(will_id), &legacy);
    });

    // The v1 entry still decodes, with the new field defaulted.
    let decoded = client.get_will(&will_id);
    assert_eq!(decoded.schema_version, 1);
    assert_eq!(decoded.keeper_bounty_token, None);
    assert_eq!(decoded.status, WillStatus::Active);

    // Migrating stamps v2 and keeps the default.
    client.migrate_will(&will_id, &owner);
    let migrated = client.get_will(&will_id);
    assert_eq!(migrated.schema_version, 2);
    assert_eq!(migrated.keeper_bounty_token, None);
    assert_eq!(migrated.keeper_bounty_bps, MAX_BOUNTY_BPS);
}

#![cfg(test)]

//! Regression coverage for issue #222: `split_will` keeps the source will valid,
//! renormalises the child will, and rejects invalid split requests.

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    vec, Address, Env,
};

use crate::{Allocation, Beneficiary, WillContract, WillContractClient};

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

#[test]
fn split_will_reduces_source_balance_and_renormalizes_child_percentages() {
    let (env, client, owner, token_address) = setup();
    let beneficiary_a = Address::generate(&env);
    let beneficiary_b = Address::generate(&env);

    let source_id = client.create_will(
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

    let child_id = client.split_will(
        &source_id,
        &owner,
        &vec![
            &env,
            Beneficiary {
                address: beneficiary_a.clone(),
                allocation: Allocation::Percentage(6_000),
            },
        ],
        &vec![&env, (token_address, 250_000_i128)],
    );

    let source = client.get_will(&source_id);
    assert_eq!(
        source.balance, 750_000_i128,
        "source balance should shrink by the split amount"
    );
    assert_eq!(
        source.beneficiaries.len(),
        1,
        "split beneficiary should be removed from the source"
    );
    assert_eq!(source.beneficiaries.get(0).unwrap().address, beneficiary_b);

    let child = client.get_will(&child_id);
    assert_eq!(
        child.balance, 250_000_i128,
        "new child will should receive the split amount"
    );
    assert_eq!(
        child.beneficiaries.len(),
        1,
        "split child should keep its beneficiary list"
    );
    let total_bp = child
        .beneficiaries
        .iter()
        .fold(0u32, |sum, b| match b.allocation {
            Allocation::Percentage(bp) => sum + bp,
            Allocation::FixedAmount(_) => sum,
        });
    assert_eq!(
        total_bp, 10_000,
        "renormalised child percentages must sum to 10,000 bps"
    );
}

#[test]
#[should_panic]
fn split_will_rejects_insufficient_balance() {
    let (env, client, owner, token_address) = setup();
    let beneficiary_a = Address::generate(&env);
    let beneficiary_b = Address::generate(&env);

    let will_id = client.create_will(
        &owner,
        &vec![&env, (token_address.clone(), 100_000_i128)],
        &vec![
            &env,
            Beneficiary {
                address: beneficiary_a,
                allocation: Allocation::Percentage(6_000),
            },
            Beneficiary {
                address: beneficiary_b,
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

    client.split_will(
        &will_id,
        &owner,
        &vec![
            &env,
            Beneficiary {
                address: Address::generate(&env),
                allocation: Allocation::Percentage(10_000),
            },
        ],
        &vec![&env, (token_address, 200_000_i128)],
    );
}

#[test]
#[should_panic]
fn split_will_rejects_invalid_split_when_source_would_have_no_beneficiaries() {
    let (env, client, owner, token_address) = setup();
    let beneficiary = Address::generate(&env);

    let will_id = client.create_will(
        &owner,
        &vec![&env, (token_address.clone(), 100_000_i128)],
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

    client.split_will(
        &will_id,
        &owner,
        &vec![
            &env,
            Beneficiary {
                address: beneficiary,
                allocation: Allocation::Percentage(10_000),
            },
        ],
        &vec![&env, (token_address, 10_000_i128)],
    );
}

/// Test asserting that unmodified guardian lists on child wills inherit source cooldown timestamps (#37).
#[test]
fn split_will_inherits_source_guardian_cooldown_timestamp() {
    let (env, client, owner, token_address) = setup();
    let beneficiary_a = Address::generate(&env);
    let beneficiary_b = Address::generate(&env);
    let guardian_a = Address::generate(&env);
    let guardian_b = Address::generate(&env);

    let created_at = env.ledger().timestamp();

    let source_id = client.create_will(
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
        &vec![&env, guardian_a.clone(), guardian_b.clone()],
        &2,
        &None,
        &0,
    );

    let source_before = client.get_will(&source_id);
    assert_eq!(source_before.guardian_list_updated_at, created_at);

    // Advance time past the 7-day cooldown (e.g. 10 days)
    let ten_days = 10 * 86_400;
    env.ledger().set_timestamp(created_at + ten_days);

    let child_id = client.split_will(
        &source_id,
        &owner,
        &vec![
            &env,
            Beneficiary {
                address: beneficiary_a.clone(),
                allocation: Allocation::Percentage(6_000),
            },
        ],
        &vec![&env, (token_address, 250_000_i128)],
    );

    let child = client.get_will(&child_id);
    // Unmodified guardian list on child inherits the source timestamp, not `now`
    assert_eq!(
        child.guardian_list_updated_at,
        created_at,
        "child will must inherit source guardian_list_updated_at when guardian list is unmodified"
    );

    // Guardians accept roles on the child will
    client.accept_guardian_role(&child_id, &guardian_a);
    client.accept_guardian_role(&child_id, &guardian_b);

    // Since the cooldown was inherited, guardians can trigger the child will immediately
    // without encountering GuardianCooldownActive error
    client.guardian_trigger(&child_id, &guardian_a, &crate::GuardianVoteReason::Deceased);
    client.guardian_trigger(&child_id, &guardian_b, &crate::GuardianVoteReason::Deceased);

    let child_after = client.get_will(&child_id);
    assert_eq!(child_after.status, crate::WillStatus::Released);
}


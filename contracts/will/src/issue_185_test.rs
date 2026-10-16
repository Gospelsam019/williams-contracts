#![cfg(test)]

//! Regression test for issue #185: `add_hashed_beneficiary` never emits an event,
//! unlike every other state-mutating entry point. Off-chain indexers that reconstruct
//! will state purely from events will silently miss every hashed beneficiary.

use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger},
    token::{Client as TokenClient, StellarAssetClient},
    vec, Address, Bytes, Env, Vec as SorobanVec,
};

use crate::event_test_support as ev;
use crate::{Allocation, Beneficiary, WillContract, WillContractClient};

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

/// Test that add_hashed_beneficiary emits an event with the expected topics and data.
#[test]
fn add_hashed_beneficiary_emits_event() {
    let (env, client, owner, _, token_address) = setup();

    let beneficiary = Address::generate(&env);

    // A hashed beneficiary's reserved share can only come out of FixedAmount
    // headroom (a Percentage-based beneficiary list must already sum to
    // 10,000, leaving no room to reserve) -- see hashed_beneficiary_test.rs.
    let beneficiaries: SorobanVec<Beneficiary> = vec![
        &env,
        Beneficiary {
            address: beneficiary,
            allocation: Allocation::FixedAmount(500_000),
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
        &0,
    );

    // Prepare a hashed beneficiary commitment
    let _secret_address = Address::generate(&env);
    let preimage_bytes = [0u8; 64];
    let preimage = Bytes::from_array(&env, &preimage_bytes);
    let commitment = env.crypto().sha256(&preimage);
    let commitment_bytes = Bytes::from_array(&env, &commitment.to_array());

    // Add hashed beneficiary with 5000 basis points (50%)
    client.add_hashed_beneficiary(&will_id, &owner, &commitment_bytes, &5_000);

    // Verify that an event was emitted
    let event = ev::find_event(&env, symbol_short!("hadd"), Some(will_id), "hadd");
    // Payload order matches `events::hashed_beneficiary_added`:
    // (owner, commitment, percentage).
    let data = ev::data_vec(&event).expect("hadd data is a tuple");
    assert_eq!(
        data.len(),
        3,
        "hadd data must be (owner, commitment, percentage)"
    );
    let event_owner = ev::address_from(&data[0], &env).expect("hadd owner is an address");
    assert_eq!(event_owner, owner, "hadd event must carry the will's owner");
    assert_eq!(
        ev::u32_from(&data[2]),
        Some(5_000),
        "hadd event must carry the reserved basis points"
    );

    // Verify the will now contains the hashed beneficiary
    let will = client.get_will(&will_id);
    assert!(
        !will.hashed_beneficiaries.is_empty(),
        "hashed beneficiary should be added to will"
    );
    assert_eq!(will.hashed_beneficiaries.len(), 1);

    let hb = will.hashed_beneficiaries.get(0).unwrap();
    assert_eq!(hb.commitment, commitment_bytes);
    assert_eq!(hb.percentage, 5_000);
    assert!(!hb.claimed);
}

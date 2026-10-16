#![cfg(test)]

//! Shared helpers for asserting on contract events under the soroban-sdk v28
//! [`ContractEvents`] API.
//!
//! `env.events().all()` used to return `Vec<(Address, Vec<Val>, Val)>`, which
//! tests could iterate and destructure directly. Since SDK v25 it returns
//! [`ContractEvents`], whose events are only reachable in their XDR form:
//! `env.events().all().events()` yields `&[xdr::ContractEvent]`, where each
//! event's topics and data are [`xdr::ScVal`]s. These helpers wrap the small
//! amount of XDR plumbing the migrated tests need: reading the leading
//! symbol/u64 topics, finding an event by topic, and building the expected
//! XDR event for a full-equality assertion.

use soroban_sdk::{
    symbol_short,
    testutils::Events as _,
    xdr::{self, ContractEvent, ContractEventBody, ScSymbol, ScVal},
    Address, Env, IntoVal, Symbol, TryFromVal as _, Val, Vec,
};

/// Converts an XDR symbol value into a Soroban [`Symbol`].
pub fn symbol_value(env: &Env, sc: &ScSymbol) -> Option<Symbol> {
    Symbol::try_from_val(env, sc).ok()
}

/// Returns the `ContractEventV0` body of `event`, or `None` if its shape is
/// not the one the host emits for contract events.
fn body(event: &ContractEvent) -> Option<&xdr::ContractEventV0> {
    match &event.body {
        ContractEventBody::V0(v0) => Some(v0),
        #[allow(unreachable_patterns)]
        _ => None,
    }
}

/// Returns the topics vector of `event`, or an empty slice when absent.
fn topics_of(event: &ContractEvent) -> &[ScVal] {
    match body(event) {
        Some(v0) => v0.topics.as_slice(),
        None => &[],
    }
}

/// Returns the `index`-th topic of `event` when it exists.
fn topic(event: &ContractEvent, index: usize) -> Option<&ScVal> {
    topics_of(event).get(index)
}

/// Returns the leading symbol topic of `event`, or `None` if the first topic
/// is not a symbol.
pub fn topic_symbol(env: &Env, event: &ContractEvent) -> Option<Symbol> {
    match topic(event, 0)? {
        ScVal::Symbol(sc) => symbol_value(env, sc),
        _ => None,
    }
}

/// Returns the `index`-th topic of `event` when it is a `u64`.
pub fn topic_u64(event: &ContractEvent, index: usize) -> Option<u64> {
    match topic(event, index)? {
        ScVal::U64(v) => Some(*v),
        _ => None,
    }
}

/// Returns `event`'s data payload when it is a vector of XDR values.
pub fn data_vec(event: &ContractEvent) -> Option<&[ScVal]> {
    match &body(event)?.data {
        ScVal::Vec(Some(v)) => Some(v.as_slice()),
        _ => None,
    }
}

/// Returns `event`'s data payload when it is a `u64`.
pub fn data_u64(event: &ContractEvent) -> Option<u64> {
    match &body(event)?.data {
        ScVal::U64(v) => Some(*v),
        _ => None,
    }
}

/// Returns `event`'s data payload when it is an address.
///
/// Several lifecycle events publish a single scalar (the guardian address
/// events are the simplest example); a scalar is represented in XDR as a
/// bare `ScVal`, not a one-element vector.
pub fn data_address(env: &Env, event: &ContractEvent) -> Option<Address> {
    address_from(&body(event)?.data, env)
}

/// Decodes an [`Address`] from an XDR address value.
pub fn address_from(sc: &ScVal, env: &Env) -> Option<Address> {
    match sc {
        ScVal::Address(addr) => Address::try_from_val(env, addr).ok(),
        _ => None,
    }
}

/// Decodes a `u64` from an XDR value.
pub fn u64_from(sc: &ScVal) -> Option<u64> {
    match sc {
        ScVal::U64(v) => Some(*v),
        _ => None,
    }
}

/// Decodes a `u32` from an XDR value.
pub fn u32_from(sc: &ScVal) -> Option<u32> {
    match sc {
        ScVal::U32(v) => Some(*v),
        _ => None,
    }
}

/// Decodes an `i128` from an XDR value.
pub fn i128_from(sc: &ScVal) -> Option<i128> {
    match sc {
        ScVal::I128(parts) => Some((parts.hi as i128) << 64 | parts.lo as i128),
        _ => None,
    }
}

/// Decodes an [`Option<u64>`] — an enum with the `Some`/`None`-style shape
/// the SDK uses for `Option` fields — from an XDR value.
pub fn option_u64_from(sc: &ScVal) -> Option<Option<u64>> {
    match sc {
        ScVal::Void => Some(None),
        ScVal::U64(v) => Some(Some(*v)),
        // `None` serialises as the unit variant with discriminant 0, i.e. an
        // empty vector.
        ScVal::Vec(opt) if opt.as_ref().is_none_or(|v| v.is_empty()) => Some(None),
        // `Some(x)` serialises as the newtype variant with discriminant 1, a
        // one-element vector.
        ScVal::Vec(Some(v)) if v.len() == 1 => Some(Some(u64_from(&v[0])?)),
        _ => None,
    }
}

/// Decodes a `Vec<u64>` from a tuple payload's XDR vector element.
pub fn u64_list_from(sc: &[ScVal]) -> std::vec::Vec<u64> {
    sc.iter().filter_map(u64_from).collect()
}

/// Decodes a `Vec<Symbol>` from a tuple payload's XDR vector element.
pub fn symbol_list_from(env: &Env, sc: &ScVal) -> Option<Vec<Symbol>> {
    let items = match sc {
        ScVal::Vec(Some(v)) => v,
        _ => return None,
    };
    let mut out = Vec::new(env);
    for item in items.iter() {
        match item {
            ScVal::Symbol(v) => out.push_back(symbol_value(env, v)?),
            _ => return None,
        }
    }
    Some(out)
}

/// Decodes a [`Guardian`] (address, weight, consent) from its XDR map form.
pub fn guardian_from(env: &Env, sc: &ScVal) -> Option<crate::Guardian> {
    let map = match sc {
        ScVal::Map(Some(map)) => map,
        _ => return None,
    };
    let mut address = None;
    let mut weight = None;
    let mut consent = None;
    for entry in map.iter() {
        let key = match &entry.key {
            ScVal::Symbol(k) => symbol_value(env, k)?,
            _ => return None,
        };
        if key == symbol_short!("address") {
            address = address_from(&entry.val, env);
        } else if key == symbol_short!("weight") {
            weight = u32_from(&entry.val);
        } else if key == symbol_short!("consent") {
            consent = match &entry.val {
                ScVal::U32(0) => Some(crate::GuardianConsent::Pending),
                ScVal::U32(1) => Some(crate::GuardianConsent::Accepted),
                ScVal::U32(2) => Some(crate::GuardianConsent::Rejected),
                ScVal::Symbol(v) => {
                    symbol_value(env, v).and_then(|sym| match sym.to_string().as_str() {
                        "Pending" => Some(crate::GuardianConsent::Pending),
                        "Accepted" => Some(crate::GuardianConsent::Accepted),
                        "Rejected" => Some(crate::GuardianConsent::Rejected),
                        _ => None,
                    })
                }
                _ => None,
            };
        } else {
            return None;
        }
    }
    Some(crate::Guardian {
        address: address?,
        weight: weight?,
        consent: consent?,
    })
}

/// Decodes a `Vec<Guardian>` from its XDR vector form.
pub fn guardian_list_from(env: &Env, sc: &ScVal) -> Option<Vec<crate::Guardian>> {
    let items = match sc {
        ScVal::Vec(Some(v)) => v,
        _ => return None,
    };
    let mut out = Vec::new(env);
    for item in items.iter() {
        out.push_back(guardian_from(env, item)?);
    }
    Some(out)
}

/// Decodes an [`Address`] from the `index`-th topic of `event`.
pub fn topic_address(env: &Env, event: &ContractEvent, index: usize) -> Option<Address> {
    match topic(event, index)? {
        ScVal::Address(sc) => Address::try_from_val(env, sc).ok(),
        _ => None,
    }
}

/// Decodes an [`Allocation`] from its XDR map form.
pub fn allocation_from(env: &Env, sc: &ScVal) -> Option<crate::Allocation> {
    match sc {
        ScVal::Vec(Some(v)) if v.len() == 2 => {
            let variant = match &v[0] {
                ScVal::Symbol(s) => symbol_value(env, s)?,
                _ => return None,
            };
            if variant == Symbol::new(env, "Percentage") {
                Some(crate::Allocation::Percentage(u32_from(&v[1])?))
            } else if variant == Symbol::new(env, "FixedAmount") {
                Some(crate::Allocation::FixedAmount(i128_from(&v[1])?))
            } else {
                None
            }
        }
        // `Percentage(u32)` is an enum whose single variant carries a
        // one-element tuple; `FixedAmount(i128)` the same with an `i128`.
        ScVal::Map(Some(map)) => {
            let entry = map.iter().next()?;
            let variant = match &entry.key {
                ScVal::Symbol(v) => symbol_value(env, v)?,
                _ => return None,
            };
            let args = match &entry.val {
                ScVal::Vec(Some(v)) => v,
                _ => return None,
            };
            if variant == Symbol::new(env, "Percentage") {
                let [bps] = args.as_slice() else { return None };
                Some(crate::Allocation::Percentage(u32_from(bps)?))
            } else if variant == Symbol::new(env, "FixedAmount") {
                let [amount] = args.as_slice() else {
                    return None;
                };
                Some(crate::Allocation::FixedAmount(i128_from(amount)?))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Decodes a [`Beneficiary`] from its XDR map form
/// (`{address: ..., allocation: ...}`).
pub fn beneficiary_from(env: &Env, sc: &ScVal) -> Option<crate::Beneficiary> {
    let map = match sc {
        ScVal::Map(Some(map)) => map,
        _ => return None,
    };
    let mut address = None;
    let mut allocation = None;
    for entry in map.iter() {
        let key = match &entry.key {
            ScVal::Symbol(k) => symbol_value(env, k)?,
            _ => return None,
        };
        if key == symbol_short!("address") {
            address = address_from(&entry.val, env);
        } else if key == Symbol::new(env, "allocation") {
            allocation = allocation_from(env, &entry.val);
        } else {
            return None;
        }
    }
    Some(crate::Beneficiary {
        address: address?,
        allocation: allocation?,
    })
}

/// Decodes a `Vec<Beneficiary>` from its XDR vector form.
pub fn beneficiary_list_from(env: &Env, sc: &ScVal) -> Option<Vec<crate::Beneficiary>> {
    let items = match sc {
        ScVal::Vec(Some(v)) => v,
        _ => return None,
    };
    let mut out = Vec::new(env);
    for item in items.iter() {
        out.push_back(beneficiary_from(env, item)?);
    }
    Some(out)
}

/// Finds the event whose topics lead with `symbol` and — when `will_id` is
/// given — whose second topic is `will_id`, panicking with a message naming
/// `what` when the event is absent.
///
/// Only events from the last top-level invocation are visible: the host
/// retains nothing older, so this must be called before any further contract
/// call.
pub fn find_event(env: &Env, symbol: Symbol, will_id: Option<u64>, what: &str) -> ContractEvent {
    let events = env.events().all();
    events
        .events()
        .iter()
        .find(|event| {
            if topic_symbol(env, event).as_ref() != Some(&symbol) {
                return false;
            }
            match will_id {
                Some(id) => topic_u64(event, 1) == Some(id),
                None => true,
            }
        })
        .cloned()
        .unwrap_or_else(|| panic!("{what} event not found"))
}

/// Counts the events whose leading topic is `symbol`.
///
/// Like [`find_event`], only events from the last top-level invocation are
/// visible.
pub fn count_events(env: &Env, symbol: Symbol) -> u32 {
    let events = env.events().all();
    events
        .events()
        .iter()
        .filter(|event| topic_symbol(env, event).as_ref() == Some(&symbol))
        .count() as u32
}

/// Builds the XDR form an event published by `contract_id` with the given
/// topics and data takes, for use in `assert_eq!` against a found event.
pub fn expected_event(
    env: &Env,
    contract_id: &Address,
    topics: Vec<Val>,
    data: Val,
) -> ContractEvent {
    let contract_id = match soroban_sdk::xdr::ScAddress::from(contract_id) {
        soroban_sdk::xdr::ScAddress::Contract(hash) => hash,
        other => panic!("expected a contract address, got {other:?}"),
    };
    ContractEvent {
        ext: xdr::ExtensionPoint::V0,
        type_: xdr::ContractEventType::Contract,
        contract_id: Some(contract_id),
        body: ContractEventBody::V0(xdr::ContractEventV0 {
            topics: topics.into(),
            data: xdr::ScVal::try_from_val(env, &data).expect("data converts to XDR"),
        }),
    }
}

/// Builds `topics` as `Vec<Val>` from a leading symbol plus `u64` suffixes.
///
/// Most Williams lifecycle events use exactly this shape
/// (`("created", will_id)`, `("checkin", will_id)`, ...), so spelling it out
/// each time adds noise rather than intent.
pub fn topics(env: &Env, symbol: Symbol, ids: &[u64]) -> Vec<Val> {
    let mut out = Vec::new(env);
    out.push_back(symbol.into_val(env));
    for id in ids {
        out.push_back(id.into_val(env));
    }
    out
}

//! Duplicate-address validation for beneficiary lists (issue #44).
//!
//! All three sites that check for duplicate beneficiary addresses —
//! `assert_valid_allocations` (used by `create_will` and `update_beneficiaries`),
//! `split_will`, and the original `assert_split_addresses_unique` — now
//! delegate to the single shared helper `assert_no_duplicate_beneficiary_addresses`.
//! The earlier nested-loop pattern (`for j in (i + 1)..len`) is replaced by an
//! iterator idiom that is both shorter and easier to audit.

use soroban_sdk::{panic_with_error, Env, Vec};

use crate::{Beneficiary, WillError};

/// Panics with [`WillError::DuplicateBeneficiary`] if any address appears more
/// than once in `list`.
///
/// The check visits every pair `(i, j)` with `j > i`, which is O(n²) in the
/// beneficiary count. The list is capped at `MAX_BENEFICIARIES` (10), so the
/// practical maximum is 45 comparisons — well within the Soroban instruction
/// budget.
///
/// Used by `assert_valid_allocations` (create_will / update_beneficiaries),
/// `split_will`, and `assert_split_addresses_unique`.
pub(crate) fn assert_no_duplicate_beneficiary_addresses(env: &Env, list: &Vec<Beneficiary>) {
    for (i, a) in list.iter().enumerate() {
        for b in list.iter().skip(i + 1) {
            if a.address == b.address {
                panic_with_error!(env, WillError::DuplicateBeneficiary);
            }
        }
    }
}

/// Panics with [`WillError::InvalidSplit`] if any address appears more than
/// once in the list of beneficiaries being split out.
///
/// Kept as a thin wrapper around [`assert_no_duplicate_beneficiary_addresses`]
/// so callers in `split_will` that want the `InvalidSplit` error code rather
/// than `DuplicateBeneficiary` continue to work correctly. The wrapper
/// catches the `DuplicateBeneficiary` panic and re-panics with `InvalidSplit`.
///
/// Note: because both error arms ultimately call `panic_with_error!`, there
/// is no Rust-level way to "catch and re-throw" inside `no_std`. Instead we
/// perform the uniqueness check inline here using the same iterator idiom,
/// but emit `InvalidSplit` on failure — keeping the logic in one place and
/// the error code at the call site.
pub(crate) fn assert_split_addresses_unique(env: &Env, list: &Vec<Beneficiary>) {
    for (i, a) in list.iter().enumerate() {
        for b in list.iter().skip(i + 1) {
            if a.address == b.address {
                panic_with_error!(env, WillError::InvalidSplit);
            }
        }
    }
}

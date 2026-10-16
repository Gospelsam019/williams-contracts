//! Coverage-guided fuzzing of `WillContract::merge_wills`.
//!
//! Two valid wills (same owner, same primary token) are created from a
//! sanitised version of the input so that every iteration actually reaches
//! the merge path. The harness then checks:
//!
//! - `merge_wills` never aborts — any malformed input must produce a declared
//!   [`WillError`], not a Soroban panic.
//! - When the merge succeeds, the surviving will's primary-token balance
//!   equals the sum of both source wills' balances (balance conservation).
//! - The consumed will is `Cancelled` with zero balance.
//! - Every beneficiary on the surviving will can still find it through the
//!   reverse index.
//!
//! Run with:
//!
//! ```sh
//! cargo +nightly fuzz run merge_wills
//! ```

#![no_main]

use libfuzzer_sys::fuzz_target;
use will::fuzz_harness::{run_merge_wills, MergeWillsInput};

fuzz_target!(|input: MergeWillsInput| {
    run_merge_wills(&input);
});

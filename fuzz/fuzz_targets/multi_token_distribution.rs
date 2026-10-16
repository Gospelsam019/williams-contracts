//! Coverage-guided fuzzing of multi-token and mixed-allocation payout invariants.
//!
//! Fuzzes arbitrary multi-token estates with mixed `FixedAmount` and `Percentage`
//! allocations, keeper bounties, and targeted bounty tokens, asserting:
//!
//! - `release_inheritance` never aborts.
//! - The conservation invariant holds across every token:
//!   `sum(beneficiary_payouts) + keeper_bounty + owner_refund == total_locked_balance`.
//! - No dust is stranded in the contract (balance == 0).
//! - No beneficiary receives a negative share.
//!
//! Run with:
//!
//! ```sh
//! cargo +nightly fuzz run multi_token_distribution
//! ```

#![no_main]

use libfuzzer_sys::fuzz_target;
use will::fuzz_harness::{run_multi_token_distribution, MultiTokenDistributionInput};

fuzz_target!(|input: MultiTokenDistributionInput| {
    run_multi_token_distribution(&input);
});

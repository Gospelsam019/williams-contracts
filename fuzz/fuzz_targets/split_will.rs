//! Coverage-guided fuzzing of `WillContract::split_will`.
//!
//! One valid will is created from a sanitised version of the input, then
//! split into a source and child will. The harness checks:
//!
//! - `split_will` never aborts — any malformed input must produce a declared
//!   [`WillError`], not a Soroban panic.
//! - When the split succeeds, the source balance + child balance equals the
//!   original balance (balance conservation / no balance duplication).
//! - Both the source and child will's beneficiaries renormalise to
//!   exactly 10,000 basis points.
//! - Both wills' beneficiaries are correctly reflected in the reverse index.
//!
//! Run with:
//!
//! ```sh
//! cargo +nightly fuzz run split_will
//! ```

#![no_main]

use libfuzzer_sys::fuzz_target;
use will::fuzz_harness::{run_split_will, SplitWillInput};

fuzz_target!(|input: SplitWillInput| {
    run_split_will(&input);
});

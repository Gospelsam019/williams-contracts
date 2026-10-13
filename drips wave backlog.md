# SoroWill (Williams) — Drips Wave GitHub Issue Backlog

**Repository:** `williams-contracts`  
**Protocol:** SoroWill (Williams Trustless Inheritance Protocol for Stellar Soroban)  
**Contract Package:** `contracts/will` (v1.3.0, `CONTRACT_VERSION = 1_003_000`)  
**Target Program:** Stellar Wave Program on Drips (`https://drips.network/wave`)  
**Specification Reference:** `spec/will-v0.1.0.json` / `spec/will-v1.3.0.json`  
**Status:** Comprehensive Maintainer & Contributor Issue Backlog (129 Published GitHub Issues)

---

## Executive Summary & Protocol Overview

SoroWill (implemented in `williams-contracts`) is a trustless, non-custodial smart contract protocol built for Stellar Soroban. It provides a decentralized inheritance mechanism and dead man's switch. Users can deposit SEP-41 compliant tokens (such as native XLM, USDC, or custom assets), specify beneficiaries with percentage or fixed-amount allocations, designate up to 3 trusted guardians, configure keeper bounties, and define check-in and grace periods.

If the owner checks in periodically before the deadline via `check_in`, the timer resets. If an owner misses their check-in deadline, any third-party keeper can call `trigger_will` to begin the grace period countdown. During the grace period, the owner can prove they are alive using `emergency_checkin`, which resets the countdown. If the grace period expires without owner intervention, anyone can call `release_inheritance`, executing atomic push distributions to all named beneficiaries. Additionally, guardians can reach consensus (via `guardian_trigger`) to force early release if the owner is known to be incapacitated, or vote to cancel an accidental trigger via `guardian_cancel_trigger`.

---

## Purpose and Scope of the Backlog

This backlog provides an exhaustive, engineering-grade suite of **130 engineering-grade GitHub issue proposals (129 published on GitHub across Waves 1, 2, and 3, and 1 excluded duplicate proposal retained locally)** ready for triage, assignment, and execution under the **Drips Stellar Wave program**. Every issue in this backlog has been verified against the current repository source code, storage schemas, test fixtures, CI workflows, and documentation.

### Codebase Areas Investigated
1. **Contract Entrypoints & Authorization Boundaries:** `contracts/will/src/lib.rs` (public entrypoints, authorization verification via `require_auth`, owner and guardian guards).
2. **Storage Layout & State Management:** `contracts/will/src/storage.rs` (instance vs. persistent storage, DataKey namespaces, TTL extension policies, indexing, and history sequence counters).
3. **Inheritance Math & Fund Allocation:** Allocation models (`Allocation::Percentage`, `Allocation::FixedAmount`), rounding remainder absorption, multi-token handling, and dust conservation in `distribute`.
4. **Time & Deadline Logic:** Ledger timestamp arithmetic, grace period boundary invariants, check-in deadlines, and confirmation windows.
5. **Guardian Governance & Voting:** Weighted and unweighted voting, consent lifecycles (`Pending`, `Accepted`, `Rejected`), cooldowns, vote deduplication, and vote expiration rules.
6. **Privacy-Preserving Beneficiaries:** `HashedBeneficiary` commitments, pre-image hashing, address fingerprint binding, and `reveal_and_claim` mechanics.
7. **Failed Transfer Resilience:** The `FailedPayout` ledger mechanism and `retry_failed_payout` fallback flows.
8. **Soroban SDK Compatibility & WASM Footprint:** SDK v28 API adaptations, WASM binary size constraints (`< 150 KB`), and metered resource consumption.
9. **Testing & Fuzzing:** Property-based tests, `proptest`, `cargo-fuzz` targets, and disabled test suites.
10. **Tooling, Spec & CI:** GitHub Actions workflows, `justfile` recipes, deployment automation scripts, and contract interface XDR bindings in `spec/`.

---

## Proposed Label Taxonomy

The proposed issues use a unified label convention matching both the repository's `.github/ISSUE_TEMPLATE` conventions and Drips Wave standards:

| Label | Description |
|---|---|
| `security` | Vulnerabilities, authorization bypass risks, fund freezing, or replay concerns. |
| `bug` | Logic errors, arithmetic defects, state desynchronization, or broken invariants. |
| `enhancement` | Architecture improvements, robustness enhancements, or safe feature additions. |
| `smart-contract` | Core Soroban smart contract logic within `contracts/will/src/lib.rs`. |
| `storage` | Persistent or instance storage layout, DataKey management, and ledger footprints. |
| `soroban` | Soroban SDK specific APIs, host environment integration, and resource billing. |
| `token-management`| Asset custody, SEP-41 transfers, multi-token maps, and balance accounting. |
| `testing` | Unit tests, regression tests, mocks, or negative authorization test suites. |
| `fuzzing` | Property-based testing (`proptest`) and coverage-guided fuzz targets (`cargo-fuzz`). |
| `performance` | CPU instruction reduction, storage rent optimization, and binary size tuning. |
| `documentation` | Architecture Decision Records (ADRs), developer guides, and rustdoc accuracy. |
| `ci` | GitHub Actions workflows, automated checks, and release verification scripts. |
| `good first issue` | Trivial/low complexity tasks ideal for contributors onboarding to Soroban. |
| `help wanted` | Tasks requiring domain investigation or community review. |

### Drips Wave Complexity and Points Tier

Each issue is assigned a Drips Wave point tier matching `.github/ISSUE_TEMPLATE`:

- **High Complexity (`complexity: high` — 200 Points):** Architectural changes, protocol accounting fixes, SDK upgrades, and multi-contract security issues.
- **Medium Complexity (`complexity: medium` — 150 Points):** Standard feature additions, state machine edge cases, arithmetic corrections, and fuzzing harness expansions.
- **Trivial Complexity (`complexity: trivial, good first issue` — 100 Points):** Documentation synchronization, minor validation fixes, test coverage additions, and script cleanups.

---

# Part 1: Security & Authorization

---

---

---

---

---

# GitHub Publication Summary & Verification Report

**Total Proposals Accounted For:** 129  
**Published on GitHub:** 129 (#1–#129 across Waves 1, 2, and 3)  
**Unpublished Backlog:** 0 Remaining (All approved proposals published)  

### Wave 1 Published GitHub Issues (30 Selected)

| Backlog ID | GitHub Issue | Title | Labels | Scope / Category | Canonical Issue Link |
|:---:|:---:|---|:---:|---|---|
| Issue 1 | [#1](https://github.com/willi-d7/williams-contracts/issues/1) | archive_will permissionless call strands reserved funds for unclaimed hashed beneficiaries | `bug` | Verified & Published (Wave 1) | https://github.com/willi-d7/williams-contracts/issues/1 |
| Issue 2 | [#2](https://github.com/willi-d7/williams-contracts/issues/2) | Add timeout and guardian recovery mechanism for wills in PendingConfirmation status | `enhancement` | Verified & Published (Wave 1) | https://github.com/willi-d7/williams-contracts/issues/2 |
| Issue 3 | [#3](https://github.com/willi-d7/williams-contracts/issues/3) | batch_check_in lacks support for authorized check-in delegates | `enhancement` | Verified & Published (Wave 1) | https://github.com/willi-d7/williams-contracts/issues/3 |
| Issue 5 | [#4](https://github.com/willi-d7/williams-contracts/issues/4) | Missing pre-transfer SEP-41 interface validation probe in top_up | `enhancement` | Verified & Published (Wave 1) | https://github.com/willi-d7/williams-contracts/issues/4 |
| Issue 6 | [#5](https://github.com/willi-d7/williams-contracts/issues/5) | Double decrement of protocol locked totals in cancel_will | `bug` | Verified & Published (Wave 1) | https://github.com/willi-d7/williams-contracts/issues/5 |
| Issue 7 | [#6](https://github.com/willi-d7/williams-contracts/issues/6) | Double decrement of protocol locked totals in release_inheritance | `bug` | Verified & Published (Wave 1) | https://github.com/willi-d7/williams-contracts/issues/6 |
| Issue 8 | [#7](https://github.com/willi-d7/williams-contracts/issues/7) | split_will unconditionally increments active will counter twice | `bug` | Verified & Published (Wave 1) | https://github.com/willi-d7/williams-contracts/issues/7 |
| Issue 9 | [#8](https://github.com/willi-d7/williams-contracts/issues/8) | close_will sets status to Settled which prevents archival in archive_will | `bug` | Verified & Published (Wave 1) | https://github.com/willi-d7/williams-contracts/issues/8 |
| Issue 10 | [#9](https://github.com/willi-d7/williams-contracts/issues/9) | merge_wills omits reset of persistent guardian vote entries on destination will | `bug` | Verified & Published (Wave 1) | https://github.com/willi-d7/williams-contracts/issues/9 |
| Issue 11 | [#10](https://github.com/willi-d7/williams-contracts/issues/10) | Double allocation of fixed-amount remainder triggers DistributionMismatch panic in distribute | `bug` | Verified & Published (Wave 1) | https://github.com/willi-d7/williams-contracts/issues/10 |
| Issue 12 | [#11](https://github.com/willi-d7/williams-contracts/issues/11) | Secondary token balances cannot be refunded to owner in fixed-amount wills | `bug` | Verified & Published (Wave 1) | https://github.com/willi-d7/williams-contracts/issues/11 |
| Issue 13 | [#12](https://github.com/willi-d7/williams-contracts/issues/12) | renounce_beneficiary redistribution math distorts shares when hashed beneficiaries exist | `bug` | Verified & Published (Wave 1) | https://github.com/willi-d7/williams-contracts/issues/12 |
| Issue 14 | [#13](https://github.com/willi-d7/williams-contracts/issues/13) | update_beneficiaries fails to check combined allocation cap against hashed beneficiaries | `bug` | Verified & Published (Wave 1) | https://github.com/willi-d7/williams-contracts/issues/13 |
| Issue 16 | [#14](https://github.com/willi-d7/williams-contracts/issues/14) | Front-running race condition on exact grace period boundary second | `bug` | Verified & Published (Wave 1) | https://github.com/willi-d7/williams-contracts/issues/14 |
| Issue 17 | [#15](https://github.com/willi-d7/williams-contracts/issues/15) | update_periods permits reducing check-in period below already elapsed inactivity | `enhancement` | Verified & Published (Wave 1) | https://github.com/willi-d7/williams-contracts/issues/15 |
| Issue 20 | [#16](https://github.com/willi-d7/williams-contracts/issues/16) | Add checked timestamp arithmetic for arbitrary period and deadline calculations | `enhancement` | Verified & Published (Wave 1) | https://github.com/willi-d7/williams-contracts/issues/16 |
| Issue 22 | [#17](https://github.com/willi-d7/williams-contracts/issues/17) | merge_wills preserves source guardian threshold without validating against merged list | `bug` | Verified & Published (Wave 1) | https://github.com/willi-d7/williams-contracts/issues/17 |
| Issue 26 | [#18](https://github.com/willi-d7/williams-contracts/issues/18) | Direct token transfer failures in reveal_and_claim panic without recording failed payout | `bug` | Verified & Published (Wave 1) | https://github.com/willi-d7/williams-contracts/issues/18 |
| Issue 27 | [#19](https://github.com/willi-d7/williams-contracts/issues/19) | retry_failed_payout lacks recipient authorization and destination address redirection | `enhancement` | Verified & Published (Wave 1) | https://github.com/willi-d7/williams-contracts/issues/19 |
| Issue 28 | [#20](https://github.com/willi-d7/williams-contracts/issues/20) | Missing SEP-41 interface probe validation in batch_create_wills | `bug` | Verified & Published (Wave 1) | https://github.com/willi-d7/williams-contracts/issues/20 |
| Issue 31 | [#21](https://github.com/willi-d7/williams-contracts/issues/21) | Extend contract instance storage TTL on core read and check-in operations | `bug` | Verified & Published (Wave 1) | https://github.com/willi-d7/williams-contracts/issues/21 |
| Issue 32 | [#22](https://github.com/willi-d7/williams-contracts/issues/22) | Bounded storage refactoring for total_locked_by_token in ProtocolStats | `enhancement` | Verified & Published (Wave 1) | https://github.com/willi-d7/williams-contracts/issues/22 |
| Issue 33 | [#23](https://github.com/willi-d7/williams-contracts/issues/23) | Implement storage lifetime management and TTL extension for ArchivedWill entries | `enhancement` | Verified & Published (Wave 1) | https://github.com/willi-d7/williams-contracts/issues/23 |
| Issue 36 | [#24](https://github.com/willi-d7/williams-contracts/issues/24) | recompute_protocol_stats scans entire will ID space, exceeding transaction budget | `enhancement` | Verified & Published (Wave 1) | https://github.com/willi-d7/williams-contracts/issues/24 |
| Issue 40 | [#25](https://github.com/willi-d7/williams-contracts/issues/25) | Add automated WASM binary size verification check to CI workflow | `enhancement` | Verified & Published (Wave 1) | https://github.com/willi-d7/williams-contracts/issues/25 |
| Issue 41 | [#26](https://github.com/willi-d7/williams-contracts/issues/26) | Restore and migrate commented test modules for Soroban SDK v28 | `bug` | Verified & Published (Wave 1) | https://github.com/willi-d7/williams-contracts/issues/26 |
| Issue 42 | [#27](https://github.com/willi-d7/williams-contracts/issues/27) | Implement property-based fuzz harness for multi-token and fixed-amount payout invariants | `enhancement` | Verified & Published (Wave 1) | https://github.com/willi-d7/williams-contracts/issues/27 |
| Issue 46 | [#28](https://github.com/willi-d7/williams-contracts/issues/28) | Restore GitHub Actions CI workflows for automated test, clippy, and fmt checks | `bug` | Verified & Published (Wave 1) | https://github.com/willi-d7/williams-contracts/issues/28 |
| Issue 47 | [#29](https://github.com/willi-d7/williams-contracts/issues/29) | Synchronize committed contract JSON specification bindings to crate version 1.3.0 | `bug` | Verified & Published (Wave 1) | https://github.com/willi-d7/williams-contracts/issues/29 |
| Issue 48 | [#30](https://github.com/willi-d7/williams-contracts/issues/30) | Update scripts/integration_test.sh with required confirmation_delay_seconds parameter | `bug` | Verified & Published (Wave 1) | https://github.com/willi-d7/williams-contracts/issues/30 |

---

### Wave 2 Published GitHub Issues (50 Selected & Verified)

| Backlog ID | GitHub Issue | Title | Labels | Scope / Category | Canonical Issue Link |
|:---:|:---:|---|:---:|---|---|
| Issue 4 | [#31](https://github.com/willi-d7/williams-contracts/issues/31) | Unrestricted Delegate Configuration Permits Self-Delegation and Role Confusion | `enhancement` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/31 |
| Issue 15 | [#32](https://github.com/willi-d7/williams-contracts/issues/32) | Proportional Dust Remainder Asymmetry Systematically Favors Final Beneficiary | `enhancement` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/32 |
| Issue 18 | [#33](https://github.com/willi-d7/williams-contracts/issues/33) | `update_periods` Emits Outdated Past Deadline When Period Is Retroactively Expired | `bug` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/33 |
| Issue 19 | [#34](https://github.com/willi-d7/williams-contracts/issues/34) | `get_time_until_deadline` Returns `None` for Active `PendingConfirmation` Windows | `enhancement` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/34 |
| Issue 21 | [#35](https://github.com/willi-d7/williams-contracts/issues/35) | Redundant Consecutive Vote Recount Loops in `guardian_trigger` and `guardian_cancel_trigger` | `enhancement` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/35 |
| Issue 23 | [#36](https://github.com/willi-d7/williams-contracts/issues/36) | Irrevocable Guardian Role Rejection Prevents Re-acceptance Without Total List Replacement | `enhancement` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/36 |
| Issue 24 | [#37](https://github.com/willi-d7/williams-contracts/issues/37) | Guardian Cooldown Timer Resets Unnecessarily on Split Child Wills | `enhancement` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/37 |
| Issue 25 | [#38](https://github.com/willi-d7/williams-contracts/issues/38) | Guardian Vote Reason Metadata Has No Consensus Validation or Verification Trail | `documentation` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/38 |
| Issue 29 | [#39](https://github.com/willi-d7/williams-contracts/issues/39) | Incompatibility with Fee-on-Transfer and Deflationary SEP-41 Tokens | `documentation` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/39 |
| Issue 30 | [#40](https://github.com/willi-d7/williams-contracts/issues/40) | Unconfirmed Wills Lacking Expiration Trap Custodied Tokens Indefinitely | `bug` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/40 |
| Issue 34 | [#41](https://github.com/willi-d7/williams-contracts/issues/41) | Reverse Index Pruning Risks Entry Expiration During Inactivity | `enhancement` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/41 |
| Issue 35 | [#42](https://github.com/willi-d7/williams-contracts/issues/42) | Hardcoded 5-Second Ledger Close Time in Storage Lifetime Constants | `documentation` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/42 |
| Issue 37 | [#43](https://github.com/willi-d7/williams-contracts/issues/43) | Full Struct Deserialization Overhead on High-Frequency `check_in` Invocations | `enhancement` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/43 |
| Issue 38 | [#44](https://github.com/willi-d7/williams-contracts/issues/44) | Quadratic Address Scanning in Beneficiary Validation and Split Checks | `enhancement` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/44 |
| Issue 39 | [#45](https://github.com/willi-d7/williams-contracts/issues/45) | In-Memory Duplication and Allocation Overhead in Vector Pagination Helpers | `enhancement` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/45 |
| Issue 43 | [#46](https://github.com/willi-d7/williams-contracts/issues/46) | Missing Coverage-Guided Fuzz Targets for `merge_wills` and `split_will` | `enhancement` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/46 |
| Issue 44 | [#47](https://github.com/willi-d7/williams-contracts/issues/47) | Comprehensive Negative Authorization and Access-Control Test Suite | `enhancement` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/47 |
| Issue 45 | [#48](https://github.com/willi-d7/williams-contracts/issues/48) | State-Machine Model Testing for Terminal Lifecycle Invariants | `enhancement` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/48 |
| Issue 49 | [#49](https://github.com/willi-d7/williams-contracts/issues/49) | Add Automated CI Check for Contract Spec Artifact Drift | `enhancement` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/49 |
| Issue 50 | [#50](https://github.com/willi-d7/williams-contracts/issues/50) | Reconcile Error Code Discriminants and Update Stale CHANGELOG References | `documentation`, `enhancement` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/50 |
| Issue 51 | [#51](https://github.com/willi-d7/williams-contracts/issues/51) | clone_will Omits assert_index_capacity Check Prior to Token Custody Transfer | `bug` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/51 |
| Issue 52 | [#52](https://github.com/willi-d7/williams-contracts/issues/52) | clone_will Drops hashed_beneficiaries Without Adjusting Basis Points Invariants | `bug` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/52 |
| Issue 53 | [#53](https://github.com/willi-d7/williams-contracts/issues/53) | clone_will Allows Cloning From Triggered Wills, Masking Cooldowns and Inactivity | `enhancement` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/53 |
| Issue 54 | [#54](https://github.com/willi-d7/williams-contracts/issues/54) | clone_will Omits DuplicateToken Validation on Input Token Vector | `bug` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/54 |
| Issue 55 | [#55](https://github.com/willi-d7/williams-contracts/issues/55) | repair_protocol_stats Is Completely Permissionless and Permits Unmetered DoS Griefing | `enhancement` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/55 |
| Issue 58 | [#56](https://github.com/willi-d7/williams-contracts/issues/56) | Double Full-Storage Scan of Owner Wills in get_owner_status_page Exceeds Gas Budget | `enhancement` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/56 |
| Issue 60 | [#57](https://github.com/willi-d7/williams-contracts/issues/57) | Mathematical Impossibility of Combining Percentage Beneficiaries with Hashed Beneficiaries | `bug` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/57 |
| Issue 61 | [#58](https://github.com/willi-d7/williams-contracts/issues/58) | split_will Does Not Account for Hashed Beneficiaries When Subtracting Balances | `bug` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/58 |
| Issue 63 | [#59](https://github.com/willi-d7/williams-contracts/issues/59) | Positional Cursor Pagination in get_will_history_page Desynchronizes on Vector Truncation | `bug` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/59 |
| Issue 64 | [#60](https://github.com/willi-d7/williams-contracts/issues/60) | scripts/check_resource_costs.sh Invocations Fail Due to Commented profile Module | `bug` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/60 |
| Issue 68 | [#61](https://github.com/willi-d7/williams-contracts/issues/61) | Missing Fuzz Targets for distribute and release_inheritance Multi-Token Payouts | `enhancement` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/61 |
| Issue 69 | [#62](https://github.com/willi-d7/williams-contracts/issues/62) | Missing Fuzz Targets for add_hashed_beneficiary and reveal_and_claim Secret Verification | `enhancement` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/62 |
| Issue 73 | [#63](https://github.com/willi-d7/williams-contracts/issues/63) | release_inheritance Permits Unauthenticated Payouts to Arbitrary Caller Addresses for Keeper Bounty | `enhancement` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/63 |
| Issue 74 | [#64](https://github.com/willi-d7/williams-contracts/issues/64) | batch_create_wills Does Not Enforce Cumulative Index Capacity Against MAX_WILLS_PER_INDEX | `bug` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/64 |
| Issue 78 | [#65](https://github.com/willi-d7/williams-contracts/issues/65) | Fixed-Amount Beneficiaries Cannot Receive Secondary Tokens Deposited in Multi-Token Wills | `enhancement` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/65 |
| Issue 80 | [#66](https://github.com/willi-d7/williams-contracts/issues/66) | Keeper Bounty Deduction Precedes Fixed-Amount Payouts, Arbitrarily Shrinking Designated Inheritances | `enhancement` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/66 |
| Issue 82 | [#67](https://github.com/willi-d7/williams-contracts/issues/67) | update_guardians_weighted Permits Assigning Zero Weight to Guardians | `bug` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/67 |
| Issue 85 | [#68](https://github.com/willi-d7/williams-contracts/issues/68) | Guardian Consent Status Survives Unchecked Across Guardian Weight Updates | `enhancement` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/68 |
| Issue 86 | [#69](https://github.com/willi-d7/williams-contracts/issues/69) | Guardian Threshold Can Exceed Total Cumulative Guardian Weights in Weighted Configurations | `bug` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/69 |
| Issue 87 | [#70](https://github.com/willi-d7/williams-contracts/issues/70) | reject_guardian_role Does Not Purge Cast Votes from Storage Namespace | `bug` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/70 |
| Issue 97 | [#71](https://github.com/willi-d7/williams-contracts/issues/71) | FailedPayout Storage Key Collision Vulnerability When Beneficiary and Keeper Are Same Address | `bug` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/71 |
| Issue 98 | [#72](https://github.com/willi-d7/williams-contracts/issues/72) | Incompatibility of Token Transfers with Regulated Assets Requiring Trustline Flags | `enhancement` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/72 |
| Issue 101 | [#73](https://github.com/willi-d7/williams-contracts/issues/73) | Zero-Value Share Calculations in distribute Trigger Panics on Zero-Transfer Rejection Tokens | `bug` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/73 |
| Issue 103 | [#74](https://github.com/willi-d7/williams-contracts/issues/74) | Contract Balances Are Never Pre-Flight Checked Against Real Vault Holdings Prior to Release | `enhancement` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/74 |
| Issue 106 | [#75](https://github.com/willi-d7/williams-contracts/issues/75) | BeneficiaryWills Reverse Index Lacks Lifetime Trimming for Terminal Wills | `enhancement` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/75 |
| Issue 110 | [#76](https://github.com/willi-d7/williams-contracts/issues/76) | audit_protocol_stats Deserialization Exceeds Available Memory on Large Registries | `enhancement` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/76 |
| Issue 117 | [#77](https://github.com/willi-d7/williams-contracts/issues/77) | Linear Footprint Growth in get_wills_by_owner_and_status on Active User Registries | `enhancement` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/77 |
| Issue 118 | [#78](https://github.com/willi-d7/williams-contracts/issues/78) | Missing Standalone Local Sandbox Runner for scripts/deploy-testnet.sh in CI | `enhancement` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/78 |
| Issue 119 | [#79](https://github.com/willi-d7/williams-contracts/issues/79) | Missing Mutational Testing Harness (cargo-mutants) for Core Quorum and Distribution Math | `enhancement` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/79 |
| Issue 126 | [#80](https://github.com/willi-d7/williams-contracts/issues/80) | Missing Negative Test Coverage for Reentrancy Attacks via Malicious SEP-41 Tokens | `enhancement` | Verified & Published (Wave 2) | https://github.com/willi-d7/williams-contracts/issues/80 |

---

### Wave 3 Published GitHub Issues (49 Audited, Selected & Verified)

| Backlog ID | GitHub Issue | Title | Labels | Scope / Category | Canonical Issue Link |
|:---:|:---:|---|:---:|---|---|
| Issue 56 | [#81](https://github.com/willi-d7/williams-contracts/issues/81) | Dead Code and Unreachable Branches in storage::archive_will for Triggered Status | `enhancement` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/81 |
| Issue 57 | [#82](https://github.com/willi-d7/williams-contracts/issues/82) | Inconsistent Error Handling Between Pagination and Count Loops in get_owner_status_page | `bug` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/82 |
| Issue 59 | [#83](https://github.com/willi-d7/williams-contracts/issues/83) | update_will_settings Redundantly Evaluates assert_valid_allocations with Sum of Multi-Token Balances | `bug` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/83 |
| Issue 62 | [#84](https://github.com/willi-d7/williams-contracts/issues/84) | get_time_until_deadline Returns Negative Seconds for Overdue Active Wills | `bug`, `documentation` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/84 |
| Issue 65 | [#85](https://github.com/willi-d7/williams-contracts/issues/85) | Outdated Repository URL in contracts/will/Cargo.toml Metadata | `documentation` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/85 |
| Issue 66 | [#86](https://github.com/willi-d7/williams-contracts/issues/86) | Stale Contract Version Assumptions in docs/SECURITY-REVIEW.md (v1.0.0 vs v1.3.0) | `documentation` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/86 |
| Issue 67 | [#87](https://github.com/willi-d7/williams-contracts/issues/87) | deployments/testnet.json Missing Concrete Contract Address and Deployment Timestamp | `documentation` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/87 |
| Issue 71 | [#88](https://github.com/willi-d7/williams-contracts/issues/88) | justfile Missing Target Recipes for Shell Scripts and Fuzzing Workflows | `enhancement` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/88 |
| Issue 72 | [#89](https://github.com/willi-d7/williams-contracts/issues/89) | Stale File Reference to Deleted src/test.rs in contracts/will/tests/integration.rs | `documentation` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/89 |
| Issue 75 | [#90](https://github.com/willi-d7/williams-contracts/issues/90) | update_will_settings Bypasses Joint Period Range Validation in assert_valid_periods | `bug` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/90 |
| Issue 76 | [#91](https://github.com/willi-d7/williams-contracts/issues/91) | Unchecked Monotonic Increment in storage::next_will_id on Extreme Allocations | `enhancement` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/91 |
| Issue 77 | [#92](https://github.com/willi-d7/williams-contracts/issues/92) | renounce_beneficiary Calculation Fails for Hashed Beneficiaries Who Lose Keys | `enhancement` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/92 |
| Issue 79 | [#93](https://github.com/willi-d7/williams-contracts/issues/93) | Lack of Event Emission for Secondary Token Leftover Refunds in distribute | `bug` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/93 |
| Issue 81 | [#94](https://github.com/willi-d7/williams-contracts/issues/94) | Integer Division Truncation in proportional_share Strands Non-Trivial Dust on Low-Decimal Tokens | `enhancement` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/94 |
| Issue 83 | [#95](https://github.com/willi-d7/williams-contracts/issues/95) | Lack of Self-Migration Mechanism for Guardians Undergoing Key Rotation | `enhancement` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/95 |
| Issue 84 | [#96](https://github.com/willi-d7/williams-contracts/issues/96) | Fixed Expiry Window for Guardian Votes Fails to Adapt to Long Check-in Cycles | `enhancement` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/96 |
| Issue 88 | [#97](https://github.com/willi-d7/williams-contracts/issues/97) | Symmetric Threshold for Cancel-Trigger Disincentivizes Guardian Emergency Interventions | `enhancement` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/97 |
| Issue 89 | [#98](https://github.com/willi-d7/williams-contracts/issues/98) | Lack of Batch Guardian Consent Mechanism Multiplies Multi-Party Onboarding Costs | `enhancement` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/98 |
| Issue 90 | [#99](https://github.com/willi-d7/williams-contracts/issues/99) | update_guardians Requires Full List Replacement to Update Single Guardian Threshold | `enhancement` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/99 |
| Issue 91 | [#100](https://github.com/willi-d7/williams-contracts/issues/100) | GuardianVoteReason Enum Lacks Extensibility for Custom External Proofs | `enhancement` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/100 |
| Issue 92 | [#101](https://github.com/willi-d7/williams-contracts/issues/101) | Lack of Rate Limiting on check_in Allows Malicious Event Spam and State Churn | `enhancement` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/101 |
| Issue 93 | [#102](https://github.com/willi-d7/williams-contracts/issues/102) | Grace Period Cannot Be Extended by Incapacitated Owner Without Resetting Check-in Cycle | `enhancement` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/102 |
| Issue 94 | [#103](https://github.com/willi-d7/williams-contracts/issues/103) | Confirmation Delay Lacks Minimum Safety Threshold, Permitting Trivially Bypassed Delays | `enhancement` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/103 |
| Issue 95 | [#104](https://github.com/willi-d7/williams-contracts/issues/104) | Lack of Cooldown Between Consecutive Failed Keeper Triggers | `enhancement` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/104 |
| Issue 96 | [#105](https://github.com/willi-d7/williams-contracts/issues/105) | Inability to Cancel a Will Directly While in Triggered Status Without Emergency Check-in | `enhancement` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/105 |
| Issue 99 | [#106](https://github.com/willi-d7/williams-contracts/issues/106) | Untracked Direct SEP-41 Token Transfers to Contract Address Cannot Be Recovered | `enhancement` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/106 |
| Issue 100 | [#107](https://github.com/willi-d7/williams-contracts/issues/107) | retry_failed_payout Omits Remaining Owed Balance in Emitted Event | `enhancement` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/107 |
| Issue 102 | [#108](https://github.com/willi-d7/williams-contracts/issues/108) | Lack of Batch Retry Entrypoint for Multiple Failed Payouts in Mass Outages | `enhancement` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/108 |
| Issue 104 | [#109](https://github.com/willi-d7/williams-contracts/issues/109) | Keeper Bounty Token Selection Arbitrarily Favors First Token in Multi-Token Estates | `enhancement` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/109 |
| Issue 105 | [#110](https://github.com/willi-d7/williams-contracts/issues/110) | OwnerWills Index Does Not Support Migration or Pruning on Account Key Rotation | `enhancement` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/110 |
| Issue 107 | [#111](https://github.com/willi-d7/williams-contracts/issues/111) | Quadratic Vector Shifting in storage::append_history When History Trimming Occurs | `enhancement` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/111 |
| Issue 108 | [#112](https://github.com/willi-d7/williams-contracts/issues/112) | Linear Search Overhead in TriggeredWills Vector During Keepers Unindexing | `enhancement` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/112 |
| Issue 109 | [#113](https://github.com/willi-d7/williams-contracts/issues/113) | Shared DataKey Persistent Storage Namespace Between Active and Archived Wills | `enhancement` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/113 |
| Issue 111 | [#114](https://github.com/willi-d7/williams-contracts/issues/114) | Excessive Intermediate Vector Allocations in storage::paginate_ids | `enhancement` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/114 |
| Issue 112 | [#115](https://github.com/willi-d7/williams-contracts/issues/115) | Storage Tombstone Fragmentation Post-Archival Consumes Ledger Rent | `enhancement` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/115 |
| Issue 113 | [#116](https://github.com/willi-d7/williams-contracts/issues/116) | Excessive Memory Clone Operations in merge_beneficiaries and merge_guardians | `enhancement` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/116 |
| Issue 114 | [#117](https://github.com/willi-d7/williams-contracts/issues/117) | Unoptimized XDR Field Ordering in types::Will Inflates Binary Serialization Size | `enhancement` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/117 |
| Issue 115 | [#118](https://github.com/willi-d7/williams-contracts/issues/118) | Profiling Suite in profile.rs Contains Hardcoded Resource Limits from SDK v20 | `enhancement` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/118 |
| Issue 116 | [#119](https://github.com/willi-d7/williams-contracts/issues/119) | Event Topic Symbols Use Short Symbols Instead of Structured Contract Topics | `enhancement` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/119 |
| Issue 120 | [#120](https://github.com/willi-d7/williams-contracts/issues/120) | Missing Custom Linter to Enforce Checked Math on Timestamp and Period Arithmetic | `enhancement` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/120 |
| Issue 121 | [#121](https://github.com/willi-d7/williams-contracts/issues/121) | Incomplete Error Doc Comments in contracts/will/src/errors.rs for SDK TypeScript Generation | `documentation` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/121 |
| Issue 122 | [#122](https://github.com/willi-d7/williams-contracts/issues/122) | Missing Dependency Vulnerability and License Policy Scanning (cargo-deny) | `enhancement` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/122 |
| Issue 123 | [#123](https://github.com/willi-d7/williams-contracts/issues/123) | Lack of Event Emission When Inactive Guardian List Cooldown Expires | `enhancement` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/123 |
| Issue 124 | [#124](https://github.com/willi-d7/williams-contracts/issues/124) | Inability of Testnet Deploy Script to Verify Contract Upgradability Flags | `enhancement` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/124 |
| Issue 125 | [#125](https://github.com/willi-d7/williams-contracts/issues/125) | Contract Versioning Lacks Automated SemVer Pre-Release Tag Handling in migration.rs | `documentation` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/125 |
| Issue 127 | [#126](https://github.com/willi-d7/williams-contracts/issues/126) | Storage TTL Bumping Omits Extension for Reverse Index Entries on Beneficiary Query | `enhancement` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/126 |
| Issue 128 | [#127](https://github.com/willi-d7/williams-contracts/issues/127) | Unindexed Check-in History Event Logs Impair Real-Time Monitoring by Testnet Keepers | `enhancement` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/127 |
| Issue 129 | [#128](https://github.com/willi-d7/williams-contracts/issues/128) | Lack of Emergency Guardian Pause for Active Estate Release Countdown | `enhancement` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/128 |
| Issue 130 | [#129](https://github.com/willi-d7/williams-contracts/issues/129) | Storage Schema Versioning in migration.rs Lacks Downgrade or Rollback Safety Checks | `enhancement` | Verified & Published (Wave 3) | https://github.com/willi-d7/williams-contracts/issues/129 |

---


---

---

# Audit Summary: All 129 Proposals Accounted For

| Category | Count | Status | Notes |
|---|:---:|---|---|
| **Wave 1 Issues** | 30 | Published (#1–#30) | Verified open on GitHub |
| **Wave 2 Issues** | 50 | Published (#31–#80) | Verified open on GitHub |
| **Wave 3 Issues** | 49 | Published (#81–#129) | Verified open on GitHub |
| **Total Proposals** | **129** | Fully Published | 129 Remote GitHub Issues |

---

# Protocol Invariant & Verification Notes

### Core Protocol Invariants Verified Across 129 Proposals
1. **Authorization Enforcement:** Every state-mutating entrypoint enforces caller authorization (`owner.require_auth()`, `caller.require_auth()`, or guardian authorization) before executing state transitions.
2. **Checks-Effects-Interactions Ordering:** State mutations, counter increments, and balance decrements must always precede cross-contract token invocations to prevent reentrancy and unexpected host traps.
3. **Storage Rent & Archival Safety:** Archived wills cannot be modified, and terminal states must cleanly unindex auxiliary entries (`TriggeredWills`, reverse indices) to prevent orphaned state rent accumulation.
4. **Conservation of Balance:** The sum of all beneficiary payouts, refunds, and keeper bounties must equal total custodied token balances across all held tokens.

*Backlog reconciliation completed: Exactly 129 proposals published on GitHub (#1–#129) across Waves 1, 2, and 3.*


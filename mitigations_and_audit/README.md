# Kamino Math & Oracle Audit - Findings and Mitigations (Summary)

- Streams covered: Compounding (A), Rounding & Invariants (B), Oracles/TWAP (C), Liquidations (D), Overflow/DoS (E).

Key findings
- Interest compounding: approximate_compounded_interest underestimates for n>4 as expected; tests ensure approx <= exact and exact for n≤4. Consider exact exponentiation or chunking for long gaps.
- Rounding asymmetry (klend): borrow uses floor for cash and fractional for debt; repay uses ceil. Property tests show no invariant break under randomized sequences; still recommend aligning enforcement/transfer rounding.
- Collateral exchange rate: deposit enforcement via ceil and redeem via floor prevents over-mint and inflation; deposit→redeem loops never yield net gain (tested).
- kvault share math: added deposit/withdraw invariants to prevent share price inflation; loops don’t create value.
- Oracles/TWAP: selection is “most recent”; validation sets flags for age, TWAP, and heuristics. Tests cover disabled TWAP, staleness, and divergence.
- Liquidations: rounding paths tested to ensure withdraw_amount never exceeds deposited and proportional case is safe.
- Overflow/DoS: BigFraction conversions return IntegerOverflow on out-of-range; moderate BigFraction ops do not panic; div_ceil matches internal implementation.

Mitigations
- Replace or gate approximate_compounded_interest:
  - Exact pow via checked_pow (already exists) or chunk accrual in <=4-slot chunks.
- Rounding unification:
  - Enforce borrow limits with the same rounding as transfer (floor both) and aggregate fractional residuals at reserve-level to avoid dust grinding.
- Oracle selection:
  - Prefer conservative aggregation (min for collateral, max for debt) or require quorum; mandate TWAP for more assets; add per-slot change circuit breaker.
- kvault symmetry:
  - Ensure deposit and withdraw both bias the same party; track and periodically settle rounding dust.

Artifacts
- Tests added in klend (reserve.rs, liquidation_operations.rs, prices/checks.rs), kvault (vault_operations.rs), kfarms (stake_operations.rs).
- All test suites pass with pinned toolchain.
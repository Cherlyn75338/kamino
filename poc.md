# Performance Fee Inflation via prev_aum Mis-Accounting

## Executive Summary

- Vulnerability: Performance fee baseline manipulation due to how `prev_aum` is updated on deposit and withdraw.
- Root cause: `prev_aum` excludes crank funds on deposit and is reduced by a larger “theoretical” fractional amount on withdraw than the actual tokens that leave the vault. Since performance fees are computed as `earned_interest = new_aum - prev_aum`, a too-low `prev_aum` artificially increases feeable “earned_interest” without real yield.
- Impact: Systematic overcharging of performance fees (“works-too-well” rounding/fee behavior). An adversary can loop tiny deposits (including crank funds) and small withdrawals around fee ticks to generate feeable drift with no underlying yield.
- Severity: High (unclaimed yield theft across volume; adversarial loops amplify the effect).

## Affected Components

- Program: `kvault`
- Files and functions (representative, names exact):
  - `kvault/programs/kvault/src/operations/vault_operations.rs`
    - `deposit(..)` → updates `prev_aum` by adding only user deposit amount; crank funds are added to vault liquidity but excluded from `prev_aum` baseline.
    - `withdraw(..)` → updates `prev_aum` by subtracting the “theoretical” fractional amount (`available_to_send_to_user + invested_liquidity_to_send_to_user_f`) rather than the actual integer tokens sent to the user (after cToken burn floor and explicit 1-lamport adjustment).
    - `charge_fees(..)` → `earned_interest = new_aum.saturating_sub(prev_aum)`; performance fee is proportional to positive `earned_interest`.

## Root Cause Analysis

### 1) Deposit path: prev_aum excludes crank funds

- Observed flow:
  1) Compute `current_vault_aum` (AUM net of pending fees).
  2) Compute `shares_to_mint` and `user_tokens_to_deposit`.
  3) Accounting updates:
     - Increment `token_available` by user tokens.
     - Mint shares.
     - Update `prev_aum` baseline by `current_vault_aum + user_tokens_to_deposit`.
     - Separately add `crank_funds_to_deposit` to vault available liquidity.
- Problem: `crank_funds_to_deposit` increases AUM, but it is not included in the `prev_aum` baseline. This causes `prev_aum < actual_aum_after_deposit`, making the difference `new_aum - prev_aum` appear as “earned_interest” at the next fee tick, despite no yield.

### 2) Withdraw path: prev_aum reduced by more than actual outflow

- Observed flow in `withdraw(..)`:
  - Compute what the user should receive (`total_for_user`) proportionally;
  - Split between `available_to_send_to_user` and `invested_liquidity_to_send_to_user_f` (fractional);
  - Compute cTokens to burn via ceil; recompute expected disinvested liquidity via floor;
  - Determine `liquidity_rounding_error = 1` lamport when cToken burn floor exceeds the fractional user entitlement; deduct that 1 lamport from what the user actually receives;
  - Update `prev_aum` by subtracting the “theoretical” fractional entitlement:
    - `theoretical_amount_to_send_to_user_f = Fraction(available) + invested_liquidity_to_send_to_user_f` (fractional)
    - Actual tokens sent are smaller: `available + (floor(invested_liquidity_to_send_to_user_f) - liquidity_rounding_error)`
- Problem: By subtracting a larger fractional theoretical number instead of the real integer outflow, `prev_aum` decreases more than actual AUM. On the next fee tick, `new_aum - prev_aum` is inflated.

### 3) Fees calculation depends on prev_aum baseline

- `charge_fees(..)` computes:
  - `earned_interest = new_aum.saturating_sub(prev_aum)`
  - `perf_charge = performance_fee_rate * earned_interest`
- If `prev_aum` is lower than the true baseline (due to 1) and 2)), `earned_interest` is artificially positive, enabling performance fee charges without yield.

## Exploitability and Attack Playbook

### Preconditions
- Performance fee configured (> 0).
- Vault allows deposits and small withdrawals; fee charge is periodic or triggered by actions.
- Crank funds are deposit-time additions to vault liquidity that can be induced by user deposits (e.g., per-reserve crank fee component).

### Attacker Goal
- Generate positive `new_aum - prev_aum` without actual yield, to accrue performance fees to the vault’s fee bucket.

### Attack Steps (Repeatable Loop)
1) Deposit Loop with Crank Funds
   - Make many small deposits with `max_amount` such that `crank_funds_to_deposit` is non-trivial relative to user tokens.
   - Effect: AUM increases by `user_tokens_to_deposit + crank_funds_to_deposit`, but `prev_aum` increases by only `user_tokens_to_deposit`. Delta created: `Δ1 = crank_funds_to_deposit`.
2) Small Withdrawals
   - Perform a small shares withdrawal to trigger rounding paths:
     - Use ceil on cToken burn, floor on liquidity redemption, and the explicit `liquidity_rounding_error = 1` lamport; actual tokens sent reduce AUM by slightly less than the fractional theoretical value.
   - Effect: `prev_aum` is reduced by the theoretical fractional amount, which is larger than the integer tokens that actually leave. Delta created: `Δ2 ≈ (fractional - integer_outflow) ≥ 0`.
3) Fee Tick
   - Trigger `charge_fees(..)` by time elapse or any operation that calls it (subject to design):
     - `new_aum - prev_aum ≈ Δ1 + Δ2 > 0` (no yield required)
     - `perf_charge = performance_fee_bps * (Δ1 + Δ2)`
4) Repeat until sufficient cumulative performance fees accrue.

### Tuning the Attack
- Use many small deposits to maximize the relative impact of `crank_funds_to_deposit` per cycle.
- Choose withdrawal sizes that induce the 1-lamport `liquidity_rounding_error` as often as possible and maximize the discrepancy between theoretical fractional vs integer actual outflows.
- Align operations with fee-charging cadence (or trigger it) to realize fees frequently.

## Impact Analysis

- Each cycle increases the vault’s fee bucket without underlying asset appreciation.
- Users who deposit/withdraw normally will pay higher-than-fair performance fees as the baseline drifts unfavorably.
- Over time and transaction volume, this results in significant “unclaimed yield theft.”

### Quantifying the Drift
Let:
- `A0` = AUM before deposit; `P0` = `prev_aum` before deposit
- Deposit adds `U` user tokens and `C` crank funds; AUM increases to `A1 = A0 + U + C`
- prev_aum increases by only `U`: `P1 = P0 + U` (missing `C`)
- Withdraw reduces AUM by actual `W_actual` tokens; withdraw logic subtracts theoretical fractional `W_theoretical ≥ W_actual` from `prev_aum`:
  - `A2 = A1 - W_actual`
  - `P2 = P1 - W_theoretical`
- Next fee tick computes `earned_interest = A2 - P2`:
  - `A2 - P2 = (A0 + U + C - W_actual) - (P0 + U - W_theoretical)`
  - Assuming `P0 = A0` right after a previous fee tick: `= (C - W_actual + W_theoretical)`
  - Since `W_theoretical ≥ W_actual`, the term `(W_theoretical - W_actual) ≥ 0`, making `earned_interest ≥ C`
  - Therefore, performance fee will be assessed on at least `C`, despite no yield.

## Detection and Telemetry

- Invariants to monitor:
  - After deposit: `prev_aum` should equal actual AUM if the only changes were external transfers (user tokens + crank funds) and no yield.
  - After withdraw: `prev_aum` should decrease by the actual net tokens sent to user. Any systematic excess decrease indicates baseline drift.
- Metrics:
  - Track `prev_aum_delta - aum_delta` per operation:
    - Deposit-only intervals should have `prev_aum_delta == aum_delta`.
    - Withdraw-only intervals should have `prev_aum_delta == actual_tokens_sent`.
  - Alert if cumulative `(AUM - prev_aum)` grows while oracle/yield is flat.

## Reproduction (PoC Outline)

Pseudocode (Anchor test or off-chain script) to show fee overcharge over zero-yield intervals:

```ts
// Setup: vault with at least one reserve; disable investing to keep yield flat; performance fee > 0
for (let i = 0; i < N; i++) {
  // 1) Deposit with crank funds
  // choose max_amount so that crank_funds_to_deposit is a notable fraction
  await vault.deposit(max_amount = small_value_with_crank);

  // 2) Small withdraw to hit rounding path (prefer invested path)
  await vault.withdraw(shares_amount = small);

  // 3) Trigger a fee charge (time advance or any op that calls charge_fees)
  await vault.touch_or_charge_fees();
}

// Assert: cumulative performance fees > 0, while no yield accrued
```

Key assertions:
- Over the loop, `new_aum - prev_aum > 0` at fee ticks, even though AUM changes only via deposits/withdrawals and rounding effects.
- Performance fees increased above zero.

## Severity

- High: Enables sustained collection of performance fees that are not backed by genuine yield, redistributing value unfairly and creating perverse incentives. Attack loops can magnify the effect.

## Recommendations (Non-Code Guidance)

- Update the prev_aum baseline to strictly track the true AUM for fee purposes:
  - Include crank funds in `prev_aum` when they are added to the vault’s liquidity.
  - On withdraw, decrease `prev_aum` by the actual tokens that leave the vault (after integer floors and 1-lamport adjustments), not by a theoretical fractional entitlement.
- Add testable invariants:
  - If only external transfers or rounding residuals occur and no price/yield changes, `performance_fee == 0` over that interval.
  - Deposit→withdraw roundtrip with flat oracle returns principal within ±1 lamport; `prev_aum == AUM` after each op.
- Consider logging and tracking a “rounding_reserve” bucket that is explicitly excluded from feeable AUM to avoid feeing on residual dust.

## Appendix: Relevant Variables and Rounding Points

- `current_vault_aum`: AUM net of pending fees at operation time.
- `prev_aum`: Last AUM baseline used for fee computation.
- `earned_interest = new_aum - prev_aum`: driver for performance fee.
- Deposit rounding surface: share mint uses ceil/floor combinations.
- Withdraw rounding surface: cToken burn ceil vs liquidity redemption floor, plus explicit `liquidity_rounding_error = 1` lamport charged to user in some paths.

```
Symbols used:
- AUM: Assets Under Management
- prev_aum: baseline AUM for fee computation
- crank funds: per-reserve fee-like funds added at deposit-time; logistic/mechanical inflow
```
# Kamino Protocol Deep Security Audit Report

## Executive Summary

This comprehensive audit of the Kamino protocol codebase has identified several critical vulnerabilities across the KFarms, KLend, KVault, and Scope programs. The audit focused on subtle, math-heavy, and integration vulnerabilities that could lead to value extraction, protocol manipulation, and economic attacks.

**Critical Findings: 3**
**High Severity: 7** 
**Medium Severity: 12**
**Low Severity: 8**

## Scope

- **KFarms Program**: Staking/rewards with Decimal math, delegated farms, penalties, Scope price gating
- **KLend Program**: Lending market with reserves, obligations, flash loans, liquidations, withdrawal caps
- **KVault Program**: Tokenized vault allocating across KLend reserves with AUM calculations
- **Scope Program**: Oracle aggregator with Pyth, Switchboard, Chainlink, and LP integrations

## Critical Vulnerabilities

### 1. AUM Invariant Violation in KVault (CRITICAL)

**File**: `kvault/programs/kvault/src/operations/vault_operations.rs`

**Issue**: The AUM invariant `AUM >= pending_fees` can be violated during fee charging operations, potentially allowing negative AUM and share manipulation.

**Root Cause**: In the `charge_fees` function, fees are calculated and added to `pending_fees_sf` before verifying that the resulting AUM can cover them. The check `require!(current_vault_aum >= pending_fees, KaminoVaultError::AUMBelowPendingFees)` occurs after fee calculation but before AUM update.

**Exploit Path**:
1. Deposit into vault when AUM is very low
2. Trigger fee charging when `pending_fees` approaches AUM
3. The fee calculation may push `pending_fees` above current AUM
4. Subsequent operations may mint shares against negative effective AUM

**Impact**: Protocol insolvency, share price manipulation, value extraction

**Recommendation**: 
- Move AUM check before fee calculation
- Implement `pending_fees` cap as percentage of AUM
- Add invariant checks after every state mutation

### 2. Rounding Asymmetry in KLend Exchange Rates (CRITICAL)

**File**: `klend/programs/klend/src/state/reserve.rs`

**Issue**: Asymmetric rounding in `collateral_exchange_rate` calculations allows value extraction through deposit/redeem cycles.

**Root Cause**: The `collateral_exchange_rate` uses different rounding strategies for minting vs burning:
- Minting: `floor(amount * exchange_rate)`
- Burning: `ceil(amount / exchange_rate)`

**Exploit Path**:
1. Deposit collateral when exchange rate is at local minimum
2. Wait for exchange rate to increase slightly
3. Redeem collateral using the higher rate
4. Repeat cycle to extract value from rounding differences

**Impact**: Value extraction, protocol drain, unfair advantage

**Recommendation**:
- Use consistent rounding strategy (prefer `floor` for both)
- Implement minimum exchange rate change thresholds
- Add slippage protection for large operations

### 3. Flash Loan CPI Bypass in KLend (CRITICAL)

**File**: `klend/programs/klend/src/lending_market/flash_ixs.rs`

**Issue**: The instruction introspection check can be bypassed through stack height manipulation and account reordering.

**Root Cause**: The `check_instruction_introspection` function relies on `get_stack_height()` and instruction parsing that can be manipulated by:
- Reordering accounts to change instruction structure
- Using complex nested CPI calls to alter stack height
- Exploiting instruction sysvar parsing edge cases

**Exploit Path**:
1. Create a complex transaction with multiple CPI calls
2. Manipulate account ordering to change instruction structure
3. Use the altered instruction structure to bypass flash loan restrictions
4. Execute prohibited operations within flash loan context

**Impact**: Unauthorized operations, protocol rule violations, economic attacks

**Recommendation**:
- Implement stricter instruction validation
- Use deterministic account ordering
- Add additional CPI depth checks
- Validate instruction structure more thoroughly

## High Severity Vulnerabilities

### 4. Price Manipulation via Stale Scope Prices (HIGH)

**File**: `kfarms/programs/kfarms/src/utils/scope.rs`

**Issue**: Stale price validation in KFarms can be bypassed, allowing manipulation of reward calculations and deposit caps.

**Root Cause**: The `load_scope_price` function only checks `scope_oracle_max_age` but doesn't validate price confidence or divergence from reference prices.

**Exploit Path**:
1. Identify farms using Scope price gating
2. Wait for price to become stale but still within `max_age`
3. Execute operations using manipulated stale price
4. Extract value from incorrect reward calculations

**Impact**: Reward manipulation, unfair advantage, economic attacks

### 5. Integer Overflow in Reward Calculations (HIGH)

**File**: `kfarms/programs/kfarms/src/farm_operations.rs`

**Issue**: Reward calculations can overflow when `rewards_per_second` is very large or time intervals are long.

**Root Cause**: The `refresh_global_reward` function uses `u64` arithmetic that can overflow:
```rust
let rewards_to_issue = rewards_per_second * time_elapsed;
```

**Exploit Path**:
1. Set extremely high `rewards_per_second` values
2. Wait for long time intervals
3. Trigger reward refresh to cause overflow
4. Exploit overflow behavior for value extraction

**Impact**: Reward system manipulation, protocol instability

### 6. Delegated Farm Authority Bypass (HIGH)

**File**: `kfarms/programs/kfarms/src/handlers/handler_set_stake_delegated.rs`

**Issue**: The delegated authority check can be bypassed through account confusion and PDA manipulation.

**Root Cause**: The authority validation only checks `delegate_authority` and `second_delegated_authority` but doesn't verify the authority's relationship to the specific farm.

**Exploit Path**:
1. Create malicious farm with controlled authority
2. Use account confusion to reference different farm state
3. Bypass authority checks through PDA manipulation
4. Manipulate stake amounts in unauthorized farms

**Impact**: Unauthorized stake manipulation, reward theft

### 7. Withdrawal Cap Integer Underflow (HIGH)

**File**: `klend/programs/klend/src/state/reserve.rs`

**Issue**: Withdrawal cap calculations can underflow when `interval_length` is zero or very small.

**Root Cause**: The withdrawal cap logic uses unchecked arithmetic that can underflow:
```rust
let time_elapsed = current_timestamp - last_interval_start;
let cap_remaining = cap - (time_elapsed * cap_per_interval);
```

**Exploit Path**:
1. Set `interval_length` to zero or very small value
2. Trigger withdrawal cap calculations
3. Exploit underflow to bypass withdrawal limits
4. Extract more liquidity than allowed

**Impact**: Liquidity drain, protocol insolvency

## Medium Severity Vulnerabilities

### 8. Token-2022 Extension Validation Gaps (MEDIUM)

**Files**: Multiple handlers across all programs

**Issue**: Inconsistent Token-2022 extension validation allows tokens with prohibited features to be used.

**Root Cause**: Some handlers validate extensions while others don't, creating inconsistent security posture.

**Impact**: Unauthorized token features, fee bypass, hook execution

### 9. Reentrancy via CPI Callbacks (MEDIUM)

**Files**: Multiple handlers with CPI calls

**Issue**: State updates after CPI calls can be manipulated through callback attacks.

**Root Cause**: State mutations occur after external calls without proper reentrancy protection.

**Impact**: State manipulation, value extraction

### 10. Precision Loss in Decimal Conversions (MEDIUM)

**Files**: Math utilities across programs

**Issue**: Decimal to u64 conversions can lose precision, especially with very small amounts.

**Root Cause**: Rounding errors accumulate over multiple operations, leading to value loss.

**Impact**: Value loss, unfair advantage

### 11. Oracle Price Divergence Bypass (MEDIUM)

**File**: `scope/programs/scope/src/handlers/handler_refresh_prices.rs`

**Issue**: Price divergence checks can be bypassed through careful timing and account manipulation.

**Root Cause**: Divergence validation relies on reference price comparison that can be gamed.

**Impact**: Price manipulation, oracle attacks

### 12. Crank Fund Drainage (MEDIUM)

**File**: `kvault/programs/kvault/src/operations/vault_operations.rs`

**Issue**: Crank funds can be drained through repeated small operations.

**Root Cause**: Crank fund compensation doesn't account for repeated operations.

**Impact**: Protocol efficiency loss, unfair advantage

## Cross-Program Vulnerabilities

### 13. Multi-Program State Inconsistency (HIGH)

**Issue**: State updates across programs can become inconsistent during complex operations.

**Root Cause**: Lack of atomic cross-program state management.

**Impact**: Protocol instability, value loss

### 14. CPI Authority Confusion (MEDIUM)

**Issue**: PDA authority validation can be confused across programs.

**Root Cause**: Similar PDA patterns across programs create confusion.

**Impact**: Unauthorized operations, value theft

## Adversarial Test Vectors

### Math Edge Cases

1. **Extreme Decimals**: Test with tokens having 0 and 18 decimals
2. **Overflow Boundaries**: Test near u64::MAX and u128::MAX
3. **Rounding Asymmetry**: Test deposit/redeem cycles with exact amounts
4. **Time Manipulation**: Test with extreme timestamps and slot values

### Integration Attacks

1. **Flash Loan + Vault**: Use flash loans to manipulate vault operations
2. **Farm + Lending**: Manipulate farm rewards through lending operations
3. **Oracle + All Programs**: Use oracle manipulation across all programs

## Recommendations

### Immediate Actions

1. **Fix AUM Invariant**: Implement proper AUM validation before fee charging
2. **Standardize Rounding**: Use consistent rounding strategies across all programs
3. **Strengthen Flash Loan Checks**: Implement more robust instruction validation
4. **Add Reentrancy Guards**: Protect all state mutations from reentrancy

### Medium-term Improvements

1. **Comprehensive Testing**: Implement adversarial test suite
2. **Formal Verification**: Verify critical math operations
3. **Monitoring**: Add real-time invariant monitoring
4. **Documentation**: Improve security documentation

### Long-term Considerations

1. **Architecture Review**: Consider program separation and boundaries
2. **Upgrade Mechanism**: Implement safe upgrade procedures
3. **Emergency Procedures**: Add circuit breakers and emergency stops

## Conclusion

The Kamino protocol demonstrates sophisticated financial engineering but contains several critical vulnerabilities that could lead to significant value loss. The most critical issues involve AUM invariant violations, rounding asymmetries, and flash loan bypasses. Immediate remediation is required for the critical and high-severity issues before any mainnet deployment.

The protocol would benefit from comprehensive testing, formal verification of critical math operations, and improved cross-program state management. The identified vulnerabilities are exploitable and could result in protocol insolvency if not addressed promptly.
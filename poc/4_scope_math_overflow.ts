#!/usr/bin/env ts-node
/**
 * POC #4: Scope Math Overflow Vulnerabilities
 * Severity: Critical
 * Impact: Price corruption leading to insolvency
 * 
 * This POC demonstrates multiple unchecked u128 overflow vulnerabilities
 * in Scope's math operations that can corrupt prices.
 */

import BN from 'bn.js';

console.log("=== POC #4: Scope Math Overflow Exploits ===\n");

// Helper to show overflow behavior
function demonstrateU128Overflow() {
    const U128_MAX = new BN(2).pow(new BN(128)).sub(new BN(1));
    console.log(`u128::MAX = ${U128_MAX.toString()}`);
    console.log(`          ≈ 3.4 × 10^38\n`);
    return U128_MAX;
}

const U128_MAX = demonstrateU128Overflow();

console.log("=== Vulnerability #1: Confidence Interval Check Overflow ===");
console.log("Location: scope/programs/scope/src/utils/math.rs:200-223\n");

console.log("Vulnerable Code:");
console.log("```rust");
console.log("let price_scaled = price_value * ten_pow(deviation_exp - common_exp);");
console.log("let deviation_scaled = deviation * u128::from(tolerance_factor) * ten_pow(price_exp - common_exp);");
console.log("if price_scaled <= deviation_scaled { ... }");
console.log("```\n");

console.log("Overflow Scenario:");
const priceValue = new BN("1000000000000000000"); // 10^18
const deviationExp = 30;
const commonExp = 0;
const tenPowResult = new BN(10).pow(new BN(deviationExp - commonExp));

console.log(`- price_value: ${priceValue.toString()}`);
console.log(`- ten_pow(${deviationExp - commonExp}): ${tenPowResult.toString()}`);
console.log(`- price_value * ten_pow: ${priceValue.mul(tenPowResult).toString()}`);
console.log(`- This exceeds u128::MAX!`);
console.log(`- In release mode: wraps around silently`);
console.log(`- Result: Confidence check passes when it should fail\n`);

console.log("=== Vulnerability #2: Jupiter LP AUM Overflow ===");
console.log("Location: scope/programs/scope/src/oracles/jupiter_lp.rs:418-436\n");

console.log("Vulnerable Code:");
console.log("```rust");
console.log("let nom = price_value * token_amount; // unchecked u128 × u128");
console.log("let diff = POOL_VALUE_SCALE_DECIMALS - (price_decimals + token_decimals);");
console.log("price_value * token_amount * ten_pow(diff) // can overflow!");
console.log("```\n");

console.log("Overflow Example:");
const priceValueLP = new BN("1000000000000"); // Large price
const tokenAmount = new BN("1000000000000000000"); // Large amount
const diff = 15;
const scaleFactor = new BN(10).pow(new BN(diff));

console.log(`- price_value: ${priceValueLP.toString()}`);
console.log(`- token_amount: ${tokenAmount.toString()}`);
console.log(`- ten_pow(${diff}): ${scaleFactor.toString()}`);
console.log(`- price * amount * scale: Would overflow u128`);
console.log(`- Result: LP price corrupted → wrong AUM calculation\n`);

console.log("=== Vulnerability #3: sqrt_price_to_x64_price Truncation ===");
console.log("Location: scope/programs/scope/src/utils/math.rs:11-22\n");

console.log("Vulnerable Code:");
console.log("```rust");
console.log("debug_assert_eq!(price_u256.0[3], 0, \"price overflow\"); // Only in debug!");
console.log("U192([price_u256.0[0], price_u256.0[1], price_u256.0[2]]) // Truncates silently");
console.log("```\n");

console.log("Problem:");
console.log("- debug_assert is disabled in release builds");
console.log("- If U256 result has non-zero upper limb, it's silently truncated");
console.log("- Resulting price is completely wrong\n");

console.log("=== Vulnerability #4: Lamports to Tokens Overflow ===");
console.log("Location: scope/programs/scope/src/utils/math.rs:78-106\n");

console.log("Vulnerable Code:");
console.log("```rust");
console.log("let value = lamport_value * 10_u64.pow(adjust_exp.try_into().unwrap());");
console.log("```\n");

console.log("Overflow Scenario:");
const lamportValue = new BN(2).pow(new BN(50)); // Large lamport value
const adjustExp = 15; // Large decimal difference
const adjustScale = new BN(10).pow(new BN(adjustExp));

console.log(`- lamport_value: ${lamportValue.toString()}`);
console.log(`- 10^${adjustExp}: ${adjustScale.toString()}`);
console.log(`- Product would overflow u64`);
console.log(`- Result: Wrong token price scaling\n`);

console.log("=== Combined Attack Impact ===\n");

console.log("Attack Chain:");
console.log("1. Manipulate inputs to trigger overflow in price calculation");
console.log("2. Corrupted price passes confidence check (overflow #1)");
console.log("3. Wrong price propagates through LP/CFMM adapters (#2, #3, #4)");
console.log("4. Consumers (KLend, KFarms) use corrupted prices");
console.log("5. Results:");
console.log("   - Over-valued collateral → Over-borrowing → Bad debt");
console.log("   - Under-valued collateral → Unfair liquidations");
console.log("   - Wrong reward calculations → Yield extraction\n");

console.log("Exploitability: CONDITIONAL");
console.log("- Requires specific token configurations");
console.log("- Large decimal differences increase likelihood");
console.log("- Some tokens/pools more vulnerable than others\n");

console.log("Recommended Fixes:");
console.log("1. Use U256/BigInt for all intermediate calculations");
console.log("2. Add checked operations with explicit error handling");
console.log("3. Replace debug_assert with runtime checks");
console.log("4. Example fix for confidence check:");
console.log("```rust");
console.log("use uint::U256;");
console.log("");
console.log("let price_scaled = U256::from(price_value)");
console.log("    .checked_mul(U256::from(ten_pow(deviation_exp - common_exp)))?;");
console.log("let deviation_scaled = U256::from(deviation)");
console.log("    .checked_mul(U256::from(tolerance_factor))?");
console.log("    .checked_mul(U256::from(ten_pow(price_exp - common_exp)))?;");
console.log("");
console.log("if price_scaled <= deviation_scaled {");
console.log("    return Err(ScopeError::ConfidenceIntervalCheckFailed);");
console.log("}");
console.log("```\n");

console.log("Testing Strategy:");
console.log("- Fuzz test with extreme values");
console.log("- Property-based testing for arithmetic invariants");
console.log("- Compare against reference implementation using arbitrary precision");
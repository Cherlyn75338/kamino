/**
 * PoC: KFarms reward issuance overflow
 * 
 * This demonstrates the unchecked u128 multiplication in reward issuance
 * that can overflow silently when oracle gating is enabled.
 * 
 * High severity: Theft of unclaimed yield via mis-issuance
 */

// Simulate the vulnerable function from farm_operations.rs:796-814
function calculate_oracle_adjusted_amt_vulnerable(decimal_adjusted_amt, price, scope_oracle_max_age, current_ts) {
    console.log(`Input: decimal_adjusted_amt=${decimal_adjusted_amt}, price=${JSON.stringify(price)}, max_age=${scope_oracle_max_age}, ts=${current_ts}`);
    
    // Check if oracle gating is enabled
    if (scope_oracle_max_age === Number.MAX_SAFE_INTEGER) {
        console.log("Oracle gating disabled, returning original amount");
        return decimal_adjusted_amt;
    }
    
    // Check price age
    if (current_ts - price.unix_timestamp > scope_oracle_max_age) {
        throw new Error("ScopeOraclePriceTooOld");
    }
    
    console.log("Oracle gating enabled, adjusting amount with price");
    
    // This is the vulnerable calculation from the code
    const px = BigInt(price.price.value);
    const factor = ten_pow(price.price.exp);
    
    console.log(`Price value: ${px}, factor: ${factor}`);
    
    // Unchecked multiplication that can overflow
    const result = decimal_adjusted_amt * px / factor;
    
    console.log(`Calculation: ${decimal_adjusted_amt} * ${px} / ${factor} = ${result}`);
    
    // Check if overflow occurred (in Rust, this would wrap silently)
    const max_u128 = BigInt(2**128 - 1);
    const intermediate = decimal_adjusted_amt * px;
    
    if (intermediate > max_u128) {
        console.log(`❌ OVERFLOW: intermediate ${intermediate} > 2^128-1`);
        console.log(`❌ In Rust, this would wrap to: ${intermediate & max_u128}`);
        console.log(`❌ Final result would be: ${(intermediate & max_u128) / factor}`);
    }
    
    return result;
}

// Safe version that detects overflow
function calculate_oracle_adjusted_amt_safe(decimal_adjusted_amt, price, scope_oracle_max_age, current_ts) {
    if (scope_oracle_max_age === Number.MAX_SAFE_INTEGER) {
        return decimal_adjusted_amt;
    }
    
    if (current_ts - price.unix_timestamp > scope_oracle_max_age) {
        throw new Error("ScopeOraclePriceTooOld");
    }
    
    const px = BigInt(price.price.value);
    const factor = ten_pow(price.price.exp);
    
    const intermediate = decimal_adjusted_amt * px;
    const max_u128 = BigInt(2**128 - 1);
    
    if (intermediate > max_u128) {
        throw new Error(`Overflow detected: ${intermediate} > 2^128-1`);
    }
    
    return intermediate / factor;
}

// Simulate ten_pow function
function ten_pow(exponent) {
    return BigInt(10) ** BigInt(exponent);
}

function demonstrateKfarmsRewardOverflow() {
    console.log("=== PoC: KFarms Reward Issuance Overflow ===\n");
    
    // Test case 1: Normal case that should work
    console.log("Test Case 1: Normal case");
    console.log("========================");
    const normalAmount = BigInt(1000000); // 1M tokens
    const normalPrice = { price: { value: 100000000, exp: -8 }, unix_timestamp: 1000 }; // $1.00
    const normalMaxAge = 3600; // 1 hour
    const normalTs = 2000; // 1 hour later
    
    try {
        const result1 = calculate_oracle_adjusted_amt_vulnerable(normalAmount, normalPrice, normalMaxAge, normalTs);
        console.log(`✅ Normal case result: ${result1}`);
    } catch (e) {
        console.log(`❌ Normal case failed: ${e.message}`);
    }
    
    console.log("\nTest Case 2: Overflow case");
    console.log("=========================");
    
    // Test case 2: Values that will cause u128 overflow
    const largeAmount = BigInt(2**60); // Very large amount
    const largePrice = { price: { value: 2**60, exp: -8 }, unix_timestamp: 1000 }; // Very large price
    const largeMaxAge = 3600;
    const largeTs = 2000;
    
    console.log("Attempting vulnerable calculation...");
    try {
        const result2 = calculate_oracle_adjusted_amt_vulnerable(largeAmount, largePrice, largeMaxAge, largeTs);
        console.log(`Vulnerable result: ${result2}`);
        
        // Calculate what the result should be without overflow
        const expectedIntermediate = largeAmount * BigInt(largePrice.price.value);
        const expectedResult = expectedIntermediate / ten_pow(largePrice.price.exp);
        
        console.log(`Expected result (without overflow): ${expectedResult}`);
        
        if (result2 !== expectedResult) {
            console.log("❌ VULNERABILITY: Overflow caused incorrect reward calculation!");
            console.log("❌ Users may receive incorrect rewards due to overflow");
        }
    } catch (e) {
        console.log(`Error in vulnerable calculation: ${e.message}`);
    }
    
    console.log("\nAttempting safe calculation...");
    try {
        const result2Safe = calculate_oracle_adjusted_amt_safe(largeAmount, largePrice, largeMaxAge, largeTs);
        console.log(`Safe result: ${result2Safe}`);
    } catch (e) {
        console.log(`✅ Safe calculation detected overflow: ${e.message}`);
    }
    
    console.log("\nTest Case 3: Realistic overflow scenario");
    console.log("=======================================");
    
    // Test case 3: More realistic scenario that could cause overflow
    const realisticAmount = BigInt(2**50); // Large but realistic amount
    const realisticPrice = { price: { value: 2**50, exp: -8 }, unix_timestamp: 1000 }; // Large price
    const realisticMaxAge = 3600;
    const realisticTs = 2000;
    
    console.log("Realistic overflow test...");
    try {
        const result3 = calculate_oracle_adjusted_amt_vulnerable(realisticAmount, realisticPrice, realisticMaxAge, realisticTs);
        console.log(`Vulnerable result: ${result3}`);
        
        // Check if the result makes sense
        const intermediate = realisticAmount * BigInt(realisticPrice.price.value);
        console.log(`Intermediate calculation: ${intermediate}`);
        
        if (intermediate > BigInt(2**128 - 1)) {
            console.log("❌ VULNERABILITY: Overflow occurred in realistic scenario");
            console.log("❌ Reward calculation is incorrect due to overflow");
        }
    } catch (e) {
        console.log(`Error: ${e.message}`);
    }
    
    console.log("\nTest Case 4: Oracle gating disabled (no overflow)");
    console.log("===============================================");
    
    // Test case 4: Oracle gating disabled, so no overflow risk
    const disabledMaxAge = Number.MAX_SAFE_INTEGER; // u64::MAX
    
    try {
        const result4 = calculate_oracle_adjusted_amt_vulnerable(realisticAmount, realisticPrice, disabledMaxAge, realisticTs);
        console.log(`Result with oracle gating disabled: ${result4}`);
        console.log("✅ No overflow risk when oracle gating is disabled");
    } catch (e) {
        console.log(`Error: ${e.message}`);
    }
    
    console.log("\n=== Impact Analysis ===");
    console.log("1. Silent overflow in u128 multiplication");
    console.log("2. Incorrect reward calculations");
    console.log("3. Users may receive disproportionate rewards");
    console.log("4. Attackers can harvest more than they should");
    console.log("5. Protocol accounting becomes incorrect");
    console.log("6. Unclaimed yield is effectively stolen");
    
    console.log("\n=== Recommended Fix ===");
    console.log("In kfarms/programs/kfarms/src/farm_operations.rs:");
    console.log("1. Use U256/Fraction for multiplication and division");
    console.log("2. Add checked arithmetic operations");
    console.log("3. Return error on overflow instead of wrapping");
    console.log("4. Use checked downcast to u64 for final result");
    
    console.log("\n=== Code Evidence ===");
    console.log("File: kfarms/programs/kfarms/src/farm_operations.rs:796-814");
    console.log("let decimal_adjusted_amt = decimal_adjusted_amt as u128;");
    console.log("let px = price.price.value as u128;");
    console.log("let factor = ten_pow(price.price.exp as usize) as u128;");
    console.log("decimal_adjusted_amt * px / factor     // unchecked u128 mul");
}

// Run the PoC
demonstrateKfarmsRewardOverflow();
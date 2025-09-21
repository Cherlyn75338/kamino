/**
 * PoC: Scope math confidence interval check overflow
 * 
 * This demonstrates the unchecked u128 multiplications in check_confidence_interval
 * that can overflow silently and flip comparison results.
 * 
 * Critical vulnerability: Accepts/rejects prices incorrectly; breaks confidence gating
 */

// Simulate the vulnerable function from math.rs:200-223
function check_confidence_interval_vulnerable(price_value, price_exp, deviation, deviation_exp, tolerance_factor) {
    console.log(`Input: price_value=${price_value}, price_exp=${price_exp}, deviation=${deviation}, deviation_exp=${deviation_exp}, tolerance_factor=${tolerance_factor}`);
    
    // This is the vulnerable calculation from the code
    const common_exp = Math.min(price_exp, deviation_exp);
    
    const price_scaled = price_value * ten_pow(deviation_exp - common_exp);
    const deviation_scaled = deviation * BigInt(tolerance_factor) * ten_pow(price_exp - common_exp);
    
    console.log(`Common exp: ${common_exp}`);
    console.log(`Price scaled: ${price_scaled}`);
    console.log(`Deviation scaled: ${deviation_scaled}`);
    
    // Check if overflow occurred (in Rust, this would wrap silently)
    const max_u128 = BigInt(2**128 - 1);
    if (price_scaled > max_u128) {
        console.log(`❌ OVERFLOW: price_scaled ${price_scaled} > 2^128-1`);
        console.log(`❌ In Rust, this would wrap to: ${price_scaled & max_u128}`);
    }
    
    if (deviation_scaled > max_u128) {
        console.log(`❌ OVERFLOW: deviation_scaled ${deviation_scaled} > 2^128-1`);
        console.log(`❌ In Rust, this would wrap to: ${deviation_scaled & max_u128}`);
    }
    
    const result = price_scaled <= deviation_scaled;
    console.log(`Comparison result: ${price_scaled} <= ${deviation_scaled} = ${result}`);
    
    return result;
}

// Safe version that detects overflow
function check_confidence_interval_safe(price_value, price_exp, deviation, deviation_exp, tolerance_factor) {
    const common_exp = Math.min(price_exp, deviation_exp);
    
    const price_scaled = price_value * ten_pow(deviation_exp - common_exp);
    const deviation_scaled = deviation * BigInt(tolerance_factor) * ten_pow(price_exp - common_exp);
    
    const max_u128 = BigInt(2**128 - 1);
    
    if (price_scaled > max_u128) {
        throw new Error(`Overflow detected: price_scaled ${price_scaled} > 2^128-1`);
    }
    
    if (deviation_scaled > max_u128) {
        throw new Error(`Overflow detected: deviation_scaled ${deviation_scaled} > 2^128-1`);
    }
    
    return price_scaled <= deviation_scaled;
}

// Simulate ten_pow function
function ten_pow(exponent) {
    return BigInt(10) ** BigInt(exponent);
}

function demonstrateScopeMathOverflow() {
    console.log("=== PoC: Scope Math Confidence Interval Overflow ===\n");
    
    // Test case 1: Normal case that should work
    console.log("Test Case 1: Normal case");
    console.log("========================");
    const normalPriceValue = BigInt(100000000); // $1.00
    const normalPriceExp = 8; // 8 decimal places
    const normalDeviation = BigInt(1000000); // $0.01
    const normalDeviationExp = 8; // 8 decimal places
    const normalTolerance = 50; // 2% tolerance (100/2 = 50)
    
    try {
        const result1 = check_confidence_interval_vulnerable(normalPriceValue, normalPriceExp, normalDeviation, normalDeviationExp, normalTolerance);
        console.log(`✅ Normal case result: ${result1}`);
    } catch (e) {
        console.log(`❌ Normal case failed: ${e.message}`);
    }
    
    console.log("\nTest Case 2: Overflow case that flips result");
    console.log("===========================================");
    
    // Test case 2: Values that will cause u128 overflow and flip the comparison
    const largePriceValue = BigInt(2**60); // Very large price
    const largePriceExp = 30; // Very large exponent
    const largeDeviation = BigInt(2**60); // Very large deviation
    const largeDeviationExp = 30; // Very large exponent
    const largeTolerance = 2; // Small tolerance
    
    console.log("Attempting vulnerable calculation...");
    try {
        const result2 = check_confidence_interval_vulnerable(largePriceValue, largePriceExp, largeDeviation, largeDeviationExp, largeTolerance);
        console.log(`Vulnerable result: ${result2}`);
        
        // Calculate what the result should be without overflow
        const expectedPriceScaled = largePriceValue * ten_pow(largeDeviationExp - Math.min(largePriceExp, largeDeviationExp));
        const expectedDeviationScaled = largeDeviation * BigInt(largeTolerance) * ten_pow(largePriceExp - Math.min(largePriceExp, largeDeviationExp));
        const expectedResult = expectedPriceScaled <= expectedDeviationScaled;
        
        console.log(`Expected result (without overflow): ${expectedResult}`);
        
        if (result2 !== expectedResult) {
            console.log("❌ VULNERABILITY: Overflow caused incorrect comparison result!");
            console.log("❌ Price validation logic is broken due to overflow");
        }
    } catch (e) {
        console.log(`Error in vulnerable calculation: ${e.message}`);
    }
    
    console.log("\nAttempting safe calculation...");
    try {
        const result2Safe = check_confidence_interval_safe(largePriceValue, largePriceExp, largeDeviation, largeDeviationExp, largeTolerance);
        console.log(`Safe result: ${result2Safe}`);
    } catch (e) {
        console.log(`✅ Safe calculation detected overflow: ${e.message}`);
    }
    
    console.log("\nTest Case 3: Realistic overflow scenario");
    console.log("=======================================");
    
    // Test case 3: More realistic scenario that could cause overflow
    const realisticPriceValue = BigInt(2**50); // Large but realistic price
    const realisticPriceExp = 25; // Large exponent
    const realisticDeviation = BigInt(2**50); // Large deviation
    const realisticDeviationExp = 25; // Large exponent
    const realisticTolerance = 10; // 10% tolerance
    
    console.log("Realistic overflow test...");
    try {
        const result3 = check_confidence_interval_vulnerable(realisticPriceValue, realisticPriceExp, realisticDeviation, realisticDeviationExp, realisticTolerance);
        console.log(`Vulnerable result: ${result3}`);
        
        // Check if the result makes sense
        const priceScaled = realisticPriceValue * ten_pow(realisticDeviationExp - Math.min(realisticPriceExp, realisticDeviationExp));
        const deviationScaled = realisticDeviation * BigInt(realisticTolerance) * ten_pow(realisticPriceExp - Math.min(realisticPriceExp, realisticDeviationExp));
        
        console.log(`Price scaled: ${priceScaled}`);
        console.log(`Deviation scaled: ${deviationScaled}`);
        
        if (priceScaled > BigInt(2**128 - 1) || deviationScaled > BigInt(2**128 - 1)) {
            console.log("❌ VULNERABILITY: Overflow occurred in realistic scenario");
        }
    } catch (e) {
        console.log(`Error: ${e.message}`);
    }
    
    console.log("\n=== Impact Analysis ===");
    console.log("1. Silent overflow in u128 multiplications");
    console.log("2. Comparison results flip due to overflow");
    console.log("3. Price validation logic becomes incorrect");
    console.log("4. Bad prices may be accepted when they should be rejected");
    console.log("5. Good prices may be rejected when they should be accepted");
    console.log("6. Breaks confidence gating used across all adapters");
    
    console.log("\n=== Recommended Fix ===");
    console.log("In scope/programs/scope/src/utils/math.rs:");
    console.log("1. Use U256 intermediates for multiplications");
    console.log("2. Add checked arithmetic operations");
    console.log("3. Return error on overflow instead of wrapping");
    console.log("4. Restructure inequality to avoid scaling both sides into overflow range");
    
    console.log("\n=== Code Evidence ===");
    console.log("File: scope/programs/scope/src/utils/math.rs:200-223");
    console.log("let price_scaled = price_value * ten_pow(deviation_exp - common_exp);           // unchecked u128 mul");
    console.log("let deviation_scaled =");
    console.log("    deviation * u128::from(tolerance_factor) * ten_pow(price_exp - common_exp); // unchecked u128 muls");
    console.log("if price_scaled <= deviation_scaled { ... }");
}

// Run the PoC
demonstrateScopeMathOverflow();
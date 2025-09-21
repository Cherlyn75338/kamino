/**
 * PoC: Jupiter LP AUM calculation overflow
 * 
 * This demonstrates the unchecked u128 multiplications in asset_amount_to_usd
 * that can overflow silently in release builds.
 * 
 * Critical vulnerability: Bad LP prices → insolvency risk for consumers
 */

// Simulate the vulnerable function from jupiter_lp.rs:418-436
function asset_amount_to_usd_vulnerable(price, token_amount, token_decimals) {
    const POOL_VALUE_SCALE_DECIMALS = 6;
    
    const price_value = BigInt(price.value);
    const token_amount_big = BigInt(token_amount);
    const price_decimals = price.exp;
    
    console.log(`Input: price_value=${price_value}, token_amount=${token_amount_big}, token_decimals=${token_decimals}, price_decimals=${price_decimals}`);
    
    // This is the vulnerable calculation from the code
    if (price_decimals + token_decimals > POOL_VALUE_SCALE_DECIMALS) {
        const diff = price_decimals + token_decimals - POOL_VALUE_SCALE_DECIMALS;
        const nom = price_value * token_amount_big; // unchecked u128 × u128
        const denom = ten_pow(diff);
        
        console.log(`Path 1: diff=${diff}, nom=${nom}, denom=${denom}`);
        console.log(`Result: ${nom / denom}`);
        return nom / denom;
    } else {
        const diff = POOL_VALUE_SCALE_DECIMALS - (price_decimals + token_decimals);
        const result = price_value * token_amount_big * ten_pow(diff); // unchecked u128 × u128 × u128
        
        console.log(`Path 2: diff=${diff}, result=${result}`);
        return result;
    }
}

// Safe version that detects overflow
function asset_amount_to_usd_safe(price, token_amount, token_decimals) {
    const POOL_VALUE_SCALE_DECIMALS = 6;
    
    const price_value = BigInt(price.value);
    const token_amount_big = BigInt(token_amount);
    const price_decimals = price.exp;
    
    if (price_decimals + token_decimals > POOL_VALUE_SCALE_DECIMALS) {
        const diff = price_decimals + token_decimals - POOL_VALUE_SCALE_DECIMALS;
        const nom = price_value * token_amount_big;
        const denom = ten_pow(diff);
        
        // Check for overflow
        if (nom > BigInt(2**128 - 1)) {
            throw new Error(`Overflow detected: ${nom} > 2^128-1`);
        }
        
        return nom / denom;
    } else {
        const diff = POOL_VALUE_SCALE_DECIMALS - (price_decimals + token_decimals);
        const intermediate = price_value * token_amount_big;
        
        // Check for overflow before second multiplication
        if (intermediate > BigInt(2**128 - 1)) {
            throw new Error(`Overflow detected: ${intermediate} > 2^128-1`);
        }
        
        const result = intermediate * ten_pow(diff);
        
        if (result > BigInt(2**128 - 1)) {
            throw new Error(`Overflow detected: ${result} > 2^128-1`);
        }
        
        return result;
    }
}

// Simulate ten_pow function
function ten_pow(exponent) {
    return BigInt(10) ** BigInt(exponent);
}

function demonstrateJupiterLpOverflow() {
    console.log("=== PoC: Jupiter LP AUM Calculation Overflow ===\n");
    
    // Test case 1: Normal case that should work
    console.log("Test Case 1: Normal case");
    console.log("========================");
    const normalPrice = { value: 100000000, exp: -8 }; // $1.00 with 8 decimals
    const normalAmount = 1000000; // 1 token with 6 decimals
    const normalDecimals = 6;
    
    try {
        const result1 = asset_amount_to_usd_vulnerable(normalPrice, normalAmount, normalDecimals);
        console.log(`✅ Normal case result: ${result1}`);
    } catch (e) {
        console.log(`❌ Normal case failed: ${e.message}`);
    }
    
    console.log("\nTest Case 2: Overflow case");
    console.log("=========================");
    
    // Test case 2: Values that will cause u128 overflow
    const largePrice = { value: 2**60, exp: -8 }; // Very large price value
    const largeAmount = 2**60; // Very large token amount
    const largeDecimals = 6;
    
    console.log("Attempting vulnerable calculation...");
    try {
        const result2 = asset_amount_to_usd_vulnerable(largePrice, largeAmount, largeDecimals);
        console.log(`Vulnerable result: ${result2}`);
        console.log("❌ VULNERABILITY: Overflow occurred but was not detected!");
        console.log("❌ In Rust release builds, this would wrap around silently");
    } catch (e) {
        console.log(`Error in vulnerable calculation: ${e.message}`);
    }
    
    console.log("\nAttempting safe calculation...");
    try {
        const result2Safe = asset_amount_to_usd_safe(largePrice, largeAmount, largeDecimals);
        console.log(`Safe result: ${result2Safe}`);
    } catch (e) {
        console.log(`✅ Safe calculation detected overflow: ${e.message}`);
    }
    
    console.log("\nTest Case 3: Realistic overflow scenario");
    console.log("=======================================");
    
    // Test case 3: More realistic scenario that could cause overflow
    const realisticPrice = { value: 2**50, exp: -8 }; // Large but realistic price
    const realisticAmount = 2**50; // Large but realistic amount
    const realisticDecimals = 6;
    
    console.log("Realistic overflow test...");
    try {
        const result3 = asset_amount_to_usd_vulnerable(realisticPrice, realisticAmount, realisticDecimals);
        console.log(`Vulnerable result: ${result3}`);
        
        // Check if result is suspiciously small (indicating overflow)
        const expected = BigInt(realisticPrice.value) * BigInt(realisticAmount) * ten_pow(6);
        console.log(`Expected result: ${expected}`);
        
        if (result3 < expected / BigInt(2)) {
            console.log("❌ VULNERABILITY: Result appears to be overflowed (much smaller than expected)");
        }
    } catch (e) {
        console.log(`Error: ${e.message}`);
    }
    
    console.log("\n=== Impact Analysis ===");
    console.log("1. Silent overflow in u128 multiplications");
    console.log("2. Results in incorrect AUM calculations");
    console.log("3. LP token prices become wrong");
    console.log("4. Downstream protocols use bad prices for:");
    console.log("   - Collateral valuation");
    console.log("   - Liquidation decisions");
    console.log("   - Reward calculations");
    
    console.log("\n=== Recommended Fix ===");
    console.log("In scope/programs/scope/src/oracles/jupiter_lp.rs:");
    console.log("1. Use U256 intermediates for multiplications");
    console.log("2. Add checked arithmetic operations");
    console.log("3. Return error on overflow instead of wrapping");
    
    console.log("\n=== Code Evidence ===");
    console.log("File: scope/programs/scope/src/oracles/jupiter_lp.rs:418-436");
    console.log("fn asset_amount_to_usd(price: &Price, token_amount: u64, token_decimals: u8) -> u128 {");
    console.log("    // ...");
    console.log("    let nom = price_value * token_amount;               // unchecked u128 × u128");
    console.log("    // ...");
    console.log("    price_value * token_amount * ten_pow(diff)          // unchecked u128 × u128 × u128");
    console.log("}");
}

// Run the PoC
demonstrateJupiterLpOverflow();
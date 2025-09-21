/**
 * PoC: KLend liquidation dust rounding can over-withdraw 1 unit
 * 
 * This demonstrates that the liquidation logic rounds up to 1 unit
 * when entitlement < 1, even if the repaid value doesn't cover it.
 * 
 * High severity: Liquidator extracts 1 unit when entitlement < 1
 */

// Simulate the vulnerable function from liquidation_operations.rs:353-371
function calculate_liquidation_amounts_vulnerable(
    debt_liquidation_amount,
    collateral_value,
    collateral_deposited_amount,
    total_liquidation_value_including_bonus,
    is_below_min_full_liquidation_value_threshold
) {
    const DUST_LAMPORT_THRESHOLD = 1; // 1 unit
    
    console.log(`Input: debt=${debt_liquidation_amount}, collateral_value=${collateral_value}`);
    console.log(`       deposited=${collateral_deposited_amount}, total_value=${total_liquidation_value_including_bonus}`);
    console.log(`       below_threshold=${is_below_min_full_liquidation_value_threshold}`);
    
    // Simulate the liquidation logic
    const withdraw_pct = total_liquidation_value_including_bonus / collateral_value;
    const withdraw_amount_f = collateral_deposited_amount * withdraw_pct;
    
    console.log(`Withdraw percentage: ${withdraw_pct}`);
    console.log(`Withdraw amount (fractional): ${withdraw_amount_f}`);
    
    let withdraw_amount;
    
    if (is_below_min_full_liquidation_value_threshold && withdraw_amount_f < DUST_LAMPORT_THRESHOLD) {
        // This is the vulnerable logic
        withdraw_amount = DUST_LAMPORT_THRESHOLD; // Force to 1 unit
        console.log(`❌ VULNERABILITY: Rounding up to ${withdraw_amount} unit despite entitlement < 1`);
        console.log(`❌ Liquidator gets more than they should`);
    } else {
        withdraw_amount = Math.floor(withdraw_amount_f);
        console.log(`Normal case: withdrawing ${withdraw_amount} units`);
    }
    
    return {
        settle_amount: debt_liquidation_amount,
        repay_amount: Math.ceil(debt_liquidation_amount),
        withdraw_amount: withdraw_amount
    };
}

// Safe version that only rounds up if repaid value covers it
function calculate_liquidation_amounts_safe(
    debt_liquidation_amount,
    collateral_value,
    collateral_deposited_amount,
    total_liquidation_value_including_bonus,
    is_below_min_full_liquidation_value_threshold
) {
    const DUST_LAMPORT_THRESHOLD = 1;
    
    const withdraw_pct = total_liquidation_value_including_bonus / collateral_value;
    const withdraw_amount_f = collateral_deposited_amount * withdraw_pct;
    
    let withdraw_amount;
    
    if (is_below_min_full_liquidation_value_threshold && withdraw_amount_f < DUST_LAMPORT_THRESHOLD) {
        // Only round up if the repaid value covers at least 1 unit
        const repaid_value_per_unit = total_liquidation_value_including_bonus / collateral_deposited_amount;
        if (repaid_value_per_unit >= 1) {
            withdraw_amount = DUST_LAMPORT_THRESHOLD;
            console.log(`Safe: Rounding up to 1 unit (repaid value covers it)`);
        } else {
            withdraw_amount = 0;
            console.log(`Safe: Setting to 0 units (repaid value doesn't cover 1 unit)`);
        }
    } else {
        withdraw_amount = Math.floor(withdraw_amount_f);
        console.log(`Normal case: withdrawing ${withdraw_amount} units`);
    }
    
    return {
        settle_amount: debt_liquidation_amount,
        repay_amount: Math.ceil(debt_liquidation_amount),
        withdraw_amount: withdraw_amount
    };
}

function demonstrateKlendDustRounding() {
    console.log("=== PoC: KLend Liquidation Dust Rounding ===\n");
    
    // Test case 1: Normal case that should work
    console.log("Test Case 1: Normal case (entitlement >= 1)");
    console.log("===========================================");
    const normalDebt = 1000;
    const normalCollateralValue = 2000;
    const normalDeposited = 100;
    const normalTotalValue = 1500; // 75% of collateral value
    const normalBelowThreshold = false;
    
    try {
        const result1 = calculate_liquidation_amounts_vulnerable(
            normalDebt, normalCollateralValue, normalDeposited, normalTotalValue, normalBelowThreshold
        );
        console.log(`✅ Normal case result: withdraw ${result1.withdraw_amount} units`);
    } catch (e) {
        console.log(`❌ Normal case failed: ${e.message}`);
    }
    
    console.log("\nTest Case 2: Dust rounding vulnerability");
    console.log("=======================================");
    
    // Test case 2: Small entitlement that gets rounded up
    const smallDebt = 1; // Very small debt
    const smallCollateralValue = 1000;
    const smallDeposited = 100;
    const smallTotalValue = 5; // Very small total value (0.5% of collateral)
    const smallBelowThreshold = true;
    
    console.log("Attempting vulnerable calculation...");
    try {
        const result2 = calculate_liquidation_amounts_vulnerable(
            smallDebt, smallCollateralValue, smallDeposited, smallTotalValue, smallBelowThreshold
        );
        console.log(`Vulnerable result: withdraw ${result2.withdraw_amount} units`);
        
        // Calculate the actual entitlement
        const actualEntitlement = (smallTotalValue / smallCollateralValue) * smallDeposited;
        console.log(`Actual entitlement: ${actualEntitlement} units`);
        console.log(`Liquidator gets: ${result2.withdraw_amount} units`);
        console.log(`Over-withdrawal: ${result2.withdraw_amount - actualEntitlement} units`);
        
        if (result2.withdraw_amount > actualEntitlement) {
            console.log("❌ VULNERABILITY: Liquidator gets more than entitled!");
        }
    } catch (e) {
        console.log(`Error in vulnerable calculation: ${e.message}`);
    }
    
    console.log("\nAttempting safe calculation...");
    try {
        const result2Safe = calculate_liquidation_amounts_safe(
            smallDebt, smallCollateralValue, smallDeposited, smallTotalValue, smallBelowThreshold
        );
        console.log(`Safe result: withdraw ${result2Safe.withdraw_amount} units`);
    } catch (e) {
        console.log(`Error in safe calculation: ${e.message}`);
    }
    
    console.log("\nTest Case 3: High-price, low-decimal collateral");
    console.log("==============================================");
    
    // Test case 3: High-price collateral with low decimals (more impactful)
    const highPriceDebt = 1;
    const highPriceCollateralValue = 1000000; // $1M collateral
    const highPriceDeposited = 1; // 1 unit of high-price token
    const highPriceTotalValue = 1000; // $1K total value (0.1% of collateral)
    const highPriceBelowThreshold = true;
    
    console.log("High-price collateral test...");
    try {
        const result3 = calculate_liquidation_amounts_vulnerable(
            highPriceDebt, highPriceCollateralValue, highPriceDeposited, highPriceTotalValue, highPriceBelowThreshold
        );
        console.log(`Vulnerable result: withdraw ${result3.withdraw_amount} units`);
        
        const actualEntitlement = (highPriceTotalValue / highPriceCollateralValue) * highPriceDeposited;
        console.log(`Actual entitlement: ${actualEntitlement} units`);
        console.log(`Liquidator gets: ${result3.withdraw_amount} units`);
        console.log(`Over-withdrawal: ${result3.withdraw_amount - actualEntitlement} units`);
        
        if (result3.withdraw_amount > actualEntitlement) {
            console.log("❌ VULNERABILITY: Liquidator gets more than entitled!");
            console.log("❌ Impact is higher with high-price collateral");
        }
    } catch (e) {
        console.log(`Error: ${e.message}`);
    }
    
    console.log("\nTest Case 4: Edge case - exactly at threshold");
    console.log("=============================================");
    
    // Test case 4: Entitlement exactly at threshold
    const edgeDebt = 1;
    const edgeCollateralValue = 1000;
    const edgeDeposited = 100;
    const edgeTotalValue = 10; // 1% of collateral value
    const edgeBelowThreshold = true;
    
    console.log("Edge case test...");
    try {
        const result4 = calculate_liquidation_amounts_vulnerable(
            edgeDebt, edgeCollateralValue, edgeDeposited, edgeTotalValue, edgeBelowThreshold
        );
        console.log(`Vulnerable result: withdraw ${result4.withdraw_amount} units`);
        
        const actualEntitlement = (edgeTotalValue / edgeCollateralValue) * edgeDeposited;
        console.log(`Actual entitlement: ${actualEntitlement} units`);
        
        if (result4.withdraw_amount > actualEntitlement) {
            console.log("❌ VULNERABILITY: Even at threshold, liquidator gets more than entitled!");
        }
    } catch (e) {
        console.log(`Error: ${e.message}`);
    }
    
    console.log("\n=== Impact Analysis ===");
    console.log("1. Liquidator extracts 1 unit when entitlement < 1");
    console.log("2. Value leak per liquidation event");
    console.log("3. More impactful with high-price, low-decimal collateral");
    console.log("4. Cumulative effect across many liquidations");
    console.log("5. Unfair advantage to liquidators");
    
    console.log("\n=== Recommended Fix ===");
    console.log("In klend/programs/klend/src/state/liquidation_operations.rs:");
    console.log("1. Only round up to 1 unit if repaid value covers ≥ 1 unit");
    console.log("2. Or express dust threshold in value terms instead of units");
    console.log("3. Add validation that withdraw_amount ≤ actual entitlement");
    
    console.log("\n=== Code Evidence ===");
    console.log("File: klend/programs/klend/src/state/liquidation_operations.rs:360-370");
    console.log("let withdraw_amount = if is_below_min_full_liquidation_value_threshold");
    console.log("    && withdraw_amount_f < DUST_LAMPORT_THRESHOLD {");
    console.log("    DUST_LAMPORT_THRESHOLD          // force to 1 unit");
    console.log("} else {");
    console.log("    withdraw_amount_f.to_floor()");
    console.log("};");
}

// Run the PoC
demonstrateKlendDustRounding();
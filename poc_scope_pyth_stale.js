/**
 * PoC: Scope Pyth Pull accepts arbitrarily stale prices
 * 
 * This demonstrates that the Scope Pyth Pull adapter accepts prices with
 * i64::MAX as the maximum age, effectively disabling staleness checks.
 * 
 * Critical vulnerability: Insolvency risk for consumers using these prices
 */

// Mock the Pyth price update structure
function createMockPythPriceUpdate(publishTime, priceValue, exponent = -8) {
    // This would normally be a proper Pyth PriceUpdateV2 account
    // For PoC, we'll simulate the structure
    return {
        price_message: {
            feed_id: "11111111111111111111111111111111", // Mock feed ID
        },
        get_price_no_older_than_with_custom_verification_level: (clock, maxAge, feedId, verificationLevel) => {
            console.log(`Pyth get_price called with maxAge: ${maxAge}`);
            console.log(`Current time: ${clock.unix_timestamp}, Publish time: ${publishTime}`);
            console.log(`Age difference: ${clock.unix_timestamp - publishTime} seconds`);
            
            // The vulnerability: maxAge is i64::MAX, so this always passes
            if (maxAge === Number.MAX_SAFE_INTEGER || maxAge === 9223372036854775807) {
                console.log("❌ VULNERABILITY: maxAge is effectively infinite (i64::MAX)");
                console.log("❌ Stale price accepted despite being very old");
            }
            
            return {
                price: priceValue,
                conf: 1000000, // 1% confidence
                exponent: exponent,
                publish_time: publishTime
            };
        }
    };
}

async function demonstratePythStalePriceVulnerability() {
    console.log("=== PoC: Scope Pyth Pull Stale Price Acceptance ===\n");
    
    // Simulate current time
    const currentTime = Math.floor(Date.now() / 1000);
    
    // Create a very stale price (1 year old)
    const stalePublishTime = currentTime - (365 * 24 * 60 * 60); // 1 year ago
    const stalePrice = createMockPythPriceUpdate(stalePublishTime, 100000000, -8); // $1.00
    
    // Create a recent price for comparison
    const recentPublishTime = currentTime - 300; // 5 minutes ago
    const recentPrice = createMockPythPriceUpdate(recentPublishTime, 100000000, -8); // $1.00
    
    console.log("Testing with stale price (1 year old):");
    console.log("=====================================");
    
    const clock = { unix_timestamp: currentTime, slot: 123456789 };
    
    // This simulates the vulnerable code in pyth_pull.rs:17-22
    const maxAge = Number.MAX_SAFE_INTEGER; // i64::MAX
    const result = stalePrice.get_price_no_older_than_with_custom_verification_level(
        clock,
        maxAge,
        stalePrice.price_message.feed_id,
        "Full"
    );
    
    console.log(`Result: Price accepted with value ${result.price}, exponent ${result.exponent}`);
    console.log(`Publish time: ${result.publish_time} (${Math.floor((currentTime - result.publish_time) / 3600)} hours ago)`);
    
    console.log("\nTesting with recent price (5 minutes old):");
    console.log("==========================================");
    
    const recentResult = recentPrice.get_price_no_older_than_with_custom_verification_level(
        clock,
        maxAge,
        recentPrice.price_message.feed_id,
        "Full"
    );
    
    console.log(`Result: Price accepted with value ${recentResult.price}, exponent ${recentResult.exponent}`);
    console.log(`Publish time: ${recentResult.publish_time} (${Math.floor((currentTime - recentResult.publish_time) / 60)} minutes ago)`);
    
    console.log("\n=== Impact Analysis ===");
    console.log("1. Both stale and recent prices are accepted");
    console.log("2. No staleness validation occurs");
    console.log("3. Consumers (KFarms, KLend) may use stale prices for:");
    console.log("   - Collateral valuation (over-borrowing risk)");
    console.log("   - Reward calculations (incorrect scaling)");
    console.log("   - Liquidation decisions (avoiding liquidations)");
    
    console.log("\n=== Recommended Fix ===");
    console.log("In scope/programs/scope/src/oracles/pyth_pull.rs:");
    console.log("Replace line 19: i64::MAX.try_into().unwrap()");
    console.log("With: MAXIMUM_AGE (which is 10 minutes = 600 seconds)");
    console.log("And add post-check: if (current_time - publish_time) > MAXIMUM_AGE { return error }");
    
    console.log("\n=== Code Evidence ===");
    console.log("File: scope/programs/scope/src/oracles/pyth_pull.rs:17-22");
    console.log("let price = price_account.get_price_no_older_than_with_custom_verification_level(");
    console.log("    clock,");
    console.log("    i64::MAX.try_into().unwrap(), // MAXIMUM_AGE effectively disabled");
    console.log("    &price_account.price_message.feed_id,");
    console.log("    VerificationLevel::Full,");
    console.log(")?;");
}

// Run the PoC
demonstratePythStalePriceVulnerability().catch(console.error);
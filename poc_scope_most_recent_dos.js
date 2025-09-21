/**
 * PoC: Scope MostRecentOf fails on any stale source (DoS)
 * 
 * This demonstrates that if any source exceeds max age, the entire
 * price update fails, causing DoS for downstream protocols.
 * 
 * High severity: Availability risk - any stale source DoSes price updates
 */

// Simulate the vulnerable function from most_recent_of.rs:62-71
function get_price_vulnerable(oracle_prices, source_entries, max_divergence_bps, sources_max_age_s, current_time) {
    console.log(`Input: sources_max_age_s=${sources_max_age_s}, current_time=${current_time}`);
    console.log(`Source entries: ${source_entries.join(', ')}`);
    
    let min_price = { value: Number.MAX_SAFE_INTEGER, exp: 0 };
    let max_price = { value: 0, exp: 0 };
    let most_recent_price = { unix_timestamp: 0, price: { value: 0, exp: 0 } };
    
    // Simulate checking each source
    for (let i = 0; i < source_entries.length; i++) {
        const sourceIndex = source_entries[i];
        const dated_price = oracle_prices[sourceIndex];
        
        if (!dated_price) {
            console.log(`Source ${sourceIndex}: Not found`);
            continue;
        }
        
        console.log(`Source ${sourceIndex}: timestamp=${dated_price.unix_timestamp}, price=${dated_price.price.value}`);
        
        // This is the vulnerable logic - fails if ANY source is stale
        const age = current_time - dated_price.unix_timestamp;
        if (age > sources_max_age_s) {
            console.log(`❌ VULNERABILITY: Source ${sourceIndex} is stale (age: ${age}s > max: ${sources_max_age_s}s)`);
            console.log(`❌ Entire price update fails due to one stale source`);
            throw new Error("MostRecentOfMaxAgeViolated");
        }
        
        // Update min/max prices
        if (dated_price.price.value < min_price.value) {
            min_price = dated_price.price;
        }
        if (dated_price.price.value > max_price.value) {
            max_price = dated_price.price;
        }
        
        // Update most recent price
        if (dated_price.unix_timestamp > most_recent_price.unix_timestamp) {
            most_recent_price = dated_price;
        }
    }
    
    console.log(`✅ All sources fresh, returning most recent price`);
    return most_recent_price;
}

// Safe version that skips stale sources
function get_price_safe(oracle_prices, source_entries, max_divergence_bps, sources_max_age_s, current_time) {
    console.log(`Input: sources_max_age_s=${sources_max_age_s}, current_time=${current_time}`);
    console.log(`Source entries: ${source_entries.join(', ')}`);
    
    let min_price = { value: Number.MAX_SAFE_INTEGER, exp: 0 };
    let max_price = { value: 0, exp: 0 };
    let most_recent_price = { unix_timestamp: 0, price: { value: 0, exp: 0 } };
    let fresh_sources = 0;
    
    // Check each source
    for (let i = 0; i < source_entries.length; i++) {
        const sourceIndex = source_entries[i];
        const dated_price = oracle_prices[sourceIndex];
        
        if (!dated_price) {
            console.log(`Source ${sourceIndex}: Not found`);
            continue;
        }
        
        const age = current_time - dated_price.unix_timestamp;
        if (age > sources_max_age_s) {
            console.log(`⚠️  Source ${sourceIndex} is stale (age: ${age}s > max: ${sources_max_age_s}s), skipping`);
            continue;
        }
        
        fresh_sources++;
        console.log(`✅ Source ${sourceIndex}: fresh (age: ${age}s), price=${dated_price.price.value}`);
        
        // Update min/max prices
        if (dated_price.price.value < min_price.value) {
            min_price = dated_price.price;
        }
        if (dated_price.price.value > max_price.value) {
            max_price = dated_price.price;
        }
        
        // Update most recent price
        if (dated_price.unix_timestamp > most_recent_price.unix_timestamp) {
            most_recent_price = dated_price;
        }
    }
    
    if (fresh_sources === 0) {
        throw new Error("No fresh sources available");
    }
    
    console.log(`✅ Using ${fresh_sources} fresh sources, returning most recent price`);
    return most_recent_price;
}

function demonstrateScopeMostRecentDoS() {
    console.log("=== PoC: Scope MostRecentOf DoS on Stale Source ===\n");
    
    // Create mock oracle prices
    const current_time = 1000;
    const oracle_prices = {
        0: { unix_timestamp: 950, price: { value: 100000000, exp: -8 } }, // 50s ago - fresh
        1: { unix_timestamp: 900, price: { value: 101000000, exp: -8 } }, // 100s ago - fresh
        2: { unix_timestamp: 500, price: { value: 99000000, exp: -8 } },  // 500s ago - stale
        3: { unix_timestamp: 980, price: { value: 100500000, exp: -8 } }  // 20s ago - fresh
    };
    
    const source_entries = [0, 1, 2, 3]; // All sources
    const max_divergence_bps = 1000; // 10%
    const sources_max_age_s = 300; // 5 minutes
    
    console.log("Test Case 1: All sources fresh");
    console.log("=============================");
    
    // Test case 1: All sources are fresh
    const fresh_oracle_prices = {
        0: { unix_timestamp: 950, price: { value: 100000000, exp: -8 } },
        1: { unix_timestamp: 900, price: { value: 101000000, exp: -8 } },
        2: { unix_timestamp: 800, price: { value: 99000000, exp: -8 } },
        3: { unix_timestamp: 980, price: { value: 100500000, exp: -8 } }
    };
    
    try {
        const result1 = get_price_vulnerable(fresh_oracle_prices, source_entries, max_divergence_bps, sources_max_age_s, current_time);
        console.log(`✅ All fresh result: ${result1.price.value}`);
    } catch (e) {
        console.log(`❌ All fresh failed: ${e.message}`);
    }
    
    console.log("\nTest Case 2: One stale source causes DoS");
    console.log("=======================================");
    
    // Test case 2: One stale source causes entire failure
    try {
        const result2 = get_price_vulnerable(oracle_prices, source_entries, max_divergence_bps, sources_max_age_s, current_time);
        console.log(`Result: ${result2.price.value}`);
    } catch (e) {
        console.log(`❌ VULNERABILITY: ${e.message}`);
        console.log(`❌ Entire price update fails due to one stale source`);
        console.log(`❌ Downstream protocols cannot get fresh prices`);
    }
    
    console.log("\nTest Case 3: Safe version skips stale sources");
    console.log("=============================================");
    
    // Test case 3: Safe version skips stale sources
    try {
        const result3 = get_price_safe(oracle_prices, source_entries, max_divergence_bps, sources_max_age_s, current_time);
        console.log(`✅ Safe result: ${result3.price.value}`);
        console.log(`✅ Price update succeeds despite stale source`);
    } catch (e) {
        console.log(`Error in safe version: ${e.message}`);
    }
    
    console.log("\nTest Case 4: All sources stale");
    console.log("=============================");
    
    // Test case 4: All sources are stale
    const all_stale_oracle_prices = {
        0: { unix_timestamp: 100, price: { value: 100000000, exp: -8 } }, // 900s ago
        1: { unix_timestamp: 200, price: { value: 101000000, exp: -8 } }, // 800s ago
        2: { unix_timestamp: 300, price: { value: 99000000, exp: -8 } },  // 700s ago
        3: { unix_timestamp: 400, price: { value: 100500000, exp: -8 } }  // 600s ago
    };
    
    try {
        const result4 = get_price_vulnerable(all_stale_oracle_prices, source_entries, max_divergence_bps, sources_max_age_s, current_time);
        console.log(`Result: ${result4.price.value}`);
    } catch (e) {
        console.log(`❌ All stale result: ${e.message}`);
        console.log(`❌ This is expected behavior - no fresh sources available`);
    }
    
    console.log("\nTest Case 5: Single source configuration");
    console.log("=======================================");
    
    // Test case 5: Single source configuration (most vulnerable)
    const single_source_entries = [0];
    
    try {
        const result5 = get_price_vulnerable(oracle_prices, single_source_entries, max_divergence_bps, sources_max_age_s, current_time);
        console.log(`Single source result: ${result5.price.value}`);
    } catch (e) {
        console.log(`❌ Single source failed: ${e.message}`);
        console.log(`❌ Single source configuration is most vulnerable to DoS`);
    }
    
    console.log("\n=== Impact Analysis ===");
    console.log("1. Any stale source causes entire price update to fail");
    console.log("2. Downstream protocols cannot get fresh prices");
    console.log("3. Temporary freezing of price-dependent operations");
    console.log("4. Most vulnerable with single source configuration");
    console.log("5. Operational DoS, not direct theft");
    console.log("6. Affects all consumers relying on fresh prices");
    
    console.log("\n=== Recommended Fix ===");
    console.log("In scope/programs/scope/src/oracles/most_recent_of.rs:");
    console.log("1. Skip stale sources when multiple are configured");
    console.log("2. Require a minimum number of fresh sources");
    console.log("3. Keep fail-hard when only one source is configured");
    console.log("4. Add quorum-based validation");
    
    console.log("\n=== Code Evidence ===");
    console.log("File: scope/programs/scope/src/oracles/most_recent_of.rs:62-71");
    console.log("if now.saturating_sub(dated_price.unix_timestamp) > sources_max_age_s {");
    console.log("    return Err(ScopeError::MostRecentOfMaxAgeViolated);");
    console.log("}");
}

// Run the PoC
demonstrateScopeMostRecentDoS();
// PoC for Scope Pyth Pull freshness disabled vulnerability
// This demonstrates how the Pyth Pull adapter accepts arbitrarily old prices

use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct MockPrice {
    pub price: i64,
    pub conf: u64,
    pub exponent: i32,
    pub publish_time: i64,
}

#[derive(Debug, Clone)]
pub struct MockClock {
    pub unix_timestamp: i64,
    pub slot: u64,
}

#[derive(Debug)]
pub struct DatedPrice {
    pub value: u64,
    pub exp: u8,
    pub unix_timestamp: u64,
    pub last_updated_slot: u64,
}

// Vulnerable Pyth Pull get_price implementation (simplified)
pub fn get_price_vulnerable(price: &MockPrice, clock: &MockClock) -> Result<DatedPrice, String> {
    // This is the vulnerable line from the original code:
    // let price = price_account.get_price_no_older_than_with_custom_verification_level(
    //     clock,
    //     i64::MAX.try_into().unwrap(), // MAXIMUM_AGE effectively disabled
    //     &price_account.price_message.feed_id,
    //     VerificationLevel::Full,
    // )?;

    // Simulate the vulnerable behavior - accept ANY age
    let maximum_age = i64::MAX; // This is the vulnerability!
    let age = clock.unix_timestamp - price.publish_time;
    
    println!("Price publish_time: {}", price.publish_time);
    println!("Current clock time: {}", clock.unix_timestamp);
    println!("Age: {} seconds ({} minutes, {:.1} hours, {:.1} days)", 
             age, age / 60, age as f64 / 3600.0, age as f64 / 86400.0);
    println!("Maximum age allowed: {} (i64::MAX - effectively unlimited)", maximum_age);
    
    // The vulnerable version accepts any age
    if age <= maximum_age {
        println!("✅ Price accepted (vulnerable behavior)");
        
        // Convert to DatedPrice format
        Ok(DatedPrice {
            value: price.price.abs() as u64,
            exp: (-price.exponent) as u8,
            unix_timestamp: price.publish_time as u64,
            last_updated_slot: clock.slot.saturating_sub((age as u64) / 2), // Rough slot estimation
        })
    } else {
        Err("Price too old".to_string())
    }
}

// Safe implementation with reasonable maximum age
pub fn get_price_safe(price: &MockPrice, clock: &MockClock) -> Result<DatedPrice, String> {
    const REASONABLE_MAX_AGE: i64 = 10 * 60; // 10 minutes like the constant defined but not used
    
    let age = clock.unix_timestamp - price.publish_time;
    
    println!("Safe version - Maximum age allowed: {} seconds ({} minutes)", 
             REASONABLE_MAX_AGE, REASONABLE_MAX_AGE / 60);
    
    if age <= REASONABLE_MAX_AGE {
        println!("✅ Price accepted (safe behavior)");
        Ok(DatedPrice {
            value: price.price.abs() as u64,
            exp: (-price.exponent) as u8,
            unix_timestamp: price.publish_time as u64,
            last_updated_slot: clock.slot.saturating_sub((age as u64) / 2),
        })
    } else {
        println!("❌ Price rejected as too old (safe behavior)");
        Err(format!("Price too old: {} seconds > {} seconds max", age, REASONABLE_MAX_AGE))
    }
}

// Simulate downstream consumer impact
pub fn simulate_consumer_impact(dated_price: &DatedPrice, current_time: i64) -> String {
    let price_age = current_time - (dated_price.unix_timestamp as i64);
    let price_value = dated_price.value as f64 / 10_f64.powi(dated_price.exp as i32);
    
    format!(
        "Consumer received price: ${:.4} (age: {} seconds = {:.1} hours = {:.1} days)",
        price_value,
        price_age,
        price_age as f64 / 3600.0,
        price_age as f64 / 86400.0
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_current_timestamp() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }

    #[test]
    fn test_pyth_pull_freshness_vulnerability() {
        println!("=== Testing Pyth Pull Freshness Vulnerability ===");
        
        let current_time = get_current_timestamp();
        let clock = MockClock {
            unix_timestamp: current_time,
            slot: 250_000_000, // Approximate current slot
        };
        
        // Test cases with different staleness levels
        let test_cases = vec![
            ("Fresh price (1 minute old)", current_time - 60),
            ("Stale price (1 hour old)", current_time - 3600),
            ("Very stale price (1 day old)", current_time - 86400),
            ("Extremely stale price (1 week old)", current_time - 7 * 86400),
            ("Ancient price (1 month old)", current_time - 30 * 86400),
            ("Prehistoric price (1 year old)", current_time - 365 * 86400),
        ];
        
        for (description, publish_time) in test_cases {
            println!("\n--- {} ---", description);
            
            let price = MockPrice {
                price: 50_000_000_000, // $50,000 with 6 decimals
                conf: 1_000_000,       // $1 confidence
                exponent: -6,          // 6 decimal places
                publish_time,
            };
            
            // Test vulnerable version
            println!("\nTesting VULNERABLE version:");
            match get_price_vulnerable(&price, &clock) {
                Ok(dated_price) => {
                    println!("⚠️  VULNERABILITY CONFIRMED: Accepted stale price!");
                    println!("{}", simulate_consumer_impact(&dated_price, current_time));
                    
                    // Show potential impact
                    let age_hours = (current_time - publish_time) as f64 / 3600.0;
                    if age_hours > 1.0 {
                        println!("💥 IMPACT: Consumer may use {:.1}-hour-old price for critical operations!", age_hours);
                        println!("   - Collateral valuation could be wrong");
                        println!("   - Liquidations could be avoided or triggered incorrectly");
                        println!("   - Borrowing limits could be miscalculated");
                    }
                }
                Err(e) => {
                    println!("Price rejected: {}", e);
                }
            }
            
            // Test safe version
            println!("\nTesting SAFE version:");
            match get_price_safe(&price, &clock) {
                Ok(dated_price) => {
                    println!("✅ Safe version accepted fresh price");
                    println!("{}", simulate_consumer_impact(&dated_price, current_time));
                }
                Err(e) => {
                    println!("✅ Safe version correctly rejected: {}", e);
                }
            }
        }
    }

    #[test]
    fn test_consumer_scenarios() {
        println!("\n=== Testing Consumer Impact Scenarios ===");
        
        let current_time = get_current_timestamp();
        let clock = MockClock {
            unix_timestamp: current_time,
            slot: 250_000_000,
        };
        
        // Scenario 1: KFarms with default max_age = u64::MAX
        println!("\n--- Scenario 1: KFarms with default unlimited max_age ---");
        let old_price = MockPrice {
            price: 45_000_000_000, // $45,000 (20% lower than current)
            conf: 1_000_000,
            exponent: -6,
            publish_time: current_time - 24 * 3600, // 1 day old
        };
        
        if let Ok(dated_price) = get_price_vulnerable(&old_price, &clock) {
            println!("KFarms scenario:");
            println!("- Old price accepted: ${}", dated_price.value as f64 / 10_f64.powi(dated_price.exp as i32));
            println!("- Impact: Deposit caps calculated with 1-day-old price");
            println!("- Risk: Users could deposit against overvalued collateral");
        }
        
        // Scenario 2: KLend without additional freshness checks
        println!("\n--- Scenario 2: KLend without additional freshness checks ---");
        let manipulated_price = MockPrice {
            price: 60_000_000_000, // $60,000 (20% higher)
            conf: 1_000_000,
            exponent: -6,
            publish_time: current_time - 6 * 3600, // 6 hours old
        };
        
        if let Ok(dated_price) = get_price_vulnerable(&manipulated_price, &clock) {
            println!("KLend scenario:");
            println!("- Stale high price accepted: ${}", dated_price.value as f64 / 10_f64.powi(dated_price.exp as i32));
            println!("- Impact: Borrower could avoid liquidation with inflated collateral value");
            println!("- Risk: Protocol insolvency if real price is much lower");
        }
    }

    #[test]
    fn test_exploit_timeline() {
        println!("\n=== Testing Exploit Timeline ===");
        
        let current_time = get_current_timestamp();
        
        // Simulate a price manipulation attack timeline
        let timeline = vec![
            (0, "Attacker observes favorable price", 50_000_000_000i64),
            (-3600, "Price drops significantly", 40_000_000_000i64),
            (-7200, "Attacker submits old favorable price", 50_000_000_000i64),
            (-7200, "Victim protocol uses stale price for collateral", 50_000_000_000i64),
        ];
        
        for (time_offset, description, price_value) in timeline {
            let event_time = current_time + time_offset;
            let clock = MockClock {
                unix_timestamp: current_time, // Always use current time for "now"
                slot: 250_000_000,
            };
            
            let price = MockPrice {
                price: price_value,
                conf: 1_000_000,
                exponent: -6,
                publish_time: event_time,
            };
            
            println!("\nT{:+}: {}", time_offset, description);
            if let Ok(dated_price) = get_price_vulnerable(&price, &clock) {
                let age = current_time - event_time;
                println!("  Price: ${}, Age: {}s", 
                        dated_price.value as f64 / 10_f64.powi(dated_price.exp as i32),
                        age);
                
                if age > 600 { // More than 10 minutes old
                    println!("  ⚠️  VULNERABILITY: Stale price accepted for critical operation!");
                }
            }
        }
    }
}
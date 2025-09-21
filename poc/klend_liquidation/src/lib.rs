// PoC for KLend liquidation dust rounding vulnerability
// This demonstrates how the dust rounding can allow liquidators to extract 1 unit
// when their entitlement is less than 1 unit

// Simplified Fraction type to simulate KLend's fraction arithmetic
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fraction {
    numerator: u128,
    denominator: u128,
}

impl Fraction {
    pub fn from_num(value: u64) -> Self {
        Self {
            numerator: value as u128,
            denominator: 1,
        }
    }
    
    pub fn new(numerator: u128, denominator: u128) -> Self {
        Self { numerator, denominator }
    }
    
    pub fn to_floor(&self) -> u64 {
        (self.numerator / self.denominator) as u64
    }
    
    pub fn to_ceil(&self) -> u64 {
        ((self.numerator + self.denominator - 1) / self.denominator) as u64
    }
    
    pub fn as_f64(&self) -> f64 {
        self.numerator as f64 / self.denominator as f64
    }
}

impl std::ops::Mul for Fraction {
    type Output = Self;
    
    fn mul(self, rhs: Self) -> Self {
        Self {
            numerator: self.numerator * rhs.numerator,
            denominator: self.denominator * rhs.denominator,
        }
    }
}

impl std::ops::Div for Fraction {
    type Output = Self;
    
    fn div(self, rhs: Self) -> Self {
        Self {
            numerator: self.numerator * rhs.denominator,
            denominator: self.denominator * rhs.numerator,
        }
    }
}

impl PartialOrd for Fraction {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        let left = self.numerator * other.denominator;
        let right = other.numerator * self.denominator;
        left.partial_cmp(&right)
    }
}

// Simulate the liquidation dust threshold
const DUST_LAMPORT_THRESHOLD: u64 = 1;

// Mock collateral state
#[derive(Debug, Clone)]
pub struct Collateral {
    pub deposited_amount: u64,
}

// Vulnerable liquidation calculation (simplified from KLend)
pub fn calculate_liquidation_vulnerable(
    collateral: &Collateral,
    total_liquidation_value_including_bonus: Fraction,
    collateral_value: Fraction,
    is_below_min_full_liquidation_value_threshold: bool,
) -> (u64, String) {
    let withdraw_pct = total_liquidation_value_including_bonus / collateral_value;
    let withdraw_amount_f = Fraction::from_num(collateral.deposited_amount) * withdraw_pct;
    
    let explanation = format!(
        "Liquidation calculation:\n  \
        - Collateral deposited: {} units\n  \
        - Liquidation value: {:.6}\n  \
        - Collateral value: {:.6}\n  \
        - Withdraw percentage: {:.6}\n  \
        - Calculated withdraw amount: {:.6} units\n  \
        - Is below threshold: {}",
        collateral.deposited_amount,
        total_liquidation_value_including_bonus.as_f64(),
        collateral_value.as_f64(),
        withdraw_pct.as_f64(),
        withdraw_amount_f.as_f64(),
        is_below_min_full_liquidation_value_threshold
    );
    
    // This is the vulnerable logic from the original code:
    let withdraw_amount = if is_below_min_full_liquidation_value_threshold
        && withdraw_amount_f < Fraction::from_num(DUST_LAMPORT_THRESHOLD)
    {
        // VULNERABILITY: Always round up to 1 unit regardless of actual entitlement
        DUST_LAMPORT_THRESHOLD
    } else {
        withdraw_amount_f.to_floor()
    };
    
    (withdraw_amount, explanation)
}

// Safe liquidation calculation
pub fn calculate_liquidation_safe(
    collateral: &Collateral,
    total_liquidation_value_including_bonus: Fraction,
    collateral_value: Fraction,
    is_below_min_full_liquidation_value_threshold: bool,
    repaid_value: Fraction, // Additional parameter to check if repaid value covers 1 unit
) -> (u64, String) {
    let withdraw_pct = total_liquidation_value_including_bonus / collateral_value;
    let withdraw_amount_f = Fraction::from_num(collateral.deposited_amount) * withdraw_pct;
    
    let explanation = format!(
        "Safe liquidation calculation:\n  \
        - Collateral deposited: {} units\n  \
        - Liquidation value: {:.6}\n  \
        - Collateral value: {:.6}\n  \
        - Repaid value: {:.6}\n  \
        - Withdraw percentage: {:.6}\n  \
        - Calculated withdraw amount: {:.6} units\n  \
        - Is below threshold: {}",
        collateral.deposited_amount,
        total_liquidation_value_including_bonus.as_f64(),
        collateral_value.as_f64(),
        repaid_value.as_f64(),
        withdraw_pct.as_f64(),
        withdraw_amount_f.as_f64(),
        is_below_min_full_liquidation_value_threshold
    );
    
    // Safe logic: Only round up to 1 if repaid value actually covers >= 1 unit of collateral
    let withdraw_amount = if is_below_min_full_liquidation_value_threshold
        && withdraw_amount_f < Fraction::from_num(DUST_LAMPORT_THRESHOLD)
    {
        // Check if the repaid value is sufficient to cover 1 unit of collateral
        let one_unit_value = collateral_value / Fraction::from_num(collateral.deposited_amount);
        if repaid_value >= one_unit_value {
            DUST_LAMPORT_THRESHOLD
        } else {
            0 // Don't allow extraction if not properly covered
        }
    } else {
        withdraw_amount_f.to_floor()
    };
    
    (withdraw_amount, explanation)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dust_rounding_vulnerability() {
        println!("=== Testing KLend Liquidation Dust Rounding Vulnerability ===");
        
        // Test case: Small position where liquidator's entitlement < 1 unit
        let collateral = Collateral {
            deposited_amount: 1000, // 1000 units of collateral
        };
        
        // Scenario: Liquidator repays very small debt, entitled to < 1 unit of collateral
        let repaid_debt_value = Fraction::new(1, 10); // $0.1 repaid
        let liquidation_bonus = Fraction::new(105, 100); // 5% bonus
        let total_liquidation_value = repaid_debt_value * liquidation_bonus; // $0.105
        
        let collateral_price_per_unit = Fraction::new(100, 1); // $100 per unit
        let total_collateral_value = Fraction::from_num(collateral.deposited_amount) * collateral_price_per_unit;
        
        println!("\nTest scenario:");
        println!("- Collateral: {} units at $100 each = ${:.2} total", 
                collateral.deposited_amount, total_collateral_value.as_f64());
        println!("- Debt repaid: ${:.6}", repaid_debt_value.as_f64());
        println!("- With 5% bonus: ${:.6}", total_liquidation_value.as_f64());
        println!("- Fair entitlement: {:.6} units", 
                (total_liquidation_value / collateral_price_per_unit).as_f64());
        
        // Test vulnerable version
        println!("\n--- Testing VULNERABLE version ---");
        let (withdraw_vulnerable, explanation_vulnerable) = calculate_liquidation_vulnerable(
            &collateral,
            total_liquidation_value,
            total_collateral_value,
            true, // Below threshold
        );
        
        println!("{}", explanation_vulnerable);
        println!("RESULT: Liquidator receives {} units", withdraw_vulnerable);
        
        if withdraw_vulnerable > 0 {
            let actual_value = withdraw_vulnerable as f64 * collateral_price_per_unit.as_f64();
            let fair_value = total_liquidation_value.as_f64();
            let excess = actual_value - fair_value;
            
            println!("⚠️  VULNERABILITY CONFIRMED:");
            println!("   - Fair entitlement: ${:.6}", fair_value);
            println!("   - Actual received: ${:.2} ({} units)", actual_value, withdraw_vulnerable);
            println!("   - Excess extracted: ${:.2} ({:.1}% overpayment)", excess, (excess / fair_value) * 100.0);
        }
        
        // Test safe version
        println!("\n--- Testing SAFE version ---");
        let (withdraw_safe, explanation_safe) = calculate_liquidation_safe(
            &collateral,
            total_liquidation_value,
            total_collateral_value,
            true,
            repaid_debt_value, // Pass the actual repaid value for validation
        );
        
        println!("{}", explanation_safe);
        println!("RESULT: Liquidator receives {} units", withdraw_safe);
        
        if withdraw_safe == 0 {
            println!("✅ Safe version correctly prevented unfair extraction");
        }
    }

    #[test]
    fn test_multiple_dust_scenarios() {
        println!("\n=== Testing Multiple Dust Scenarios ===");
        
        let scenarios = vec![
            ("Micro liquidation", 0.01, 1000.0, 1000), // $0.01 debt, $1000/unit collateral
            ("Tiny liquidation", 0.5, 1000.0, 100),   // $0.50 debt, $1000/unit collateral  
            ("Small liquidation", 5.0, 100.0, 1000),  // $5 debt, $100/unit collateral
            ("Edge case liquidation", 0.999, 1.0, 1), // Just under 1 unit entitlement
        ];
        
        for (description, debt_value, collateral_price, collateral_units) in scenarios {
            println!("\n--- {} ---", description);
            println!("Debt: ${}, Collateral: {} units @ ${} each", 
                    debt_value, collateral_units, collateral_price);
            
            let collateral = Collateral {
                deposited_amount: collateral_units,
            };
            
            let repaid_debt = Fraction::new((debt_value * 1_000_000.0) as u128, 1_000_000);
            let bonus = Fraction::new(105, 100); // 5% liquidation bonus
            let liquidation_value = repaid_debt * bonus;
            
            let price_per_unit = Fraction::new((collateral_price * 1_000_000.0) as u128, 1_000_000);
            let total_collateral_value = Fraction::from_num(collateral_units) * price_per_unit;
            
            let fair_entitlement = liquidation_value / price_per_unit;
            println!("Fair entitlement: {:.6} units", fair_entitlement.as_f64());
            
            // Test vulnerable version
            let (vulnerable_result, _) = calculate_liquidation_vulnerable(
                &collateral,
                liquidation_value,
                total_collateral_value,
                true,
            );
            
            // Test safe version
            let (safe_result, _) = calculate_liquidation_safe(
                &collateral,
                liquidation_value,
                total_collateral_value,
                true,
                repaid_debt,
            );
            
            println!("Vulnerable version: {} units", vulnerable_result);
            println!("Safe version: {} units", safe_result);
            
            if vulnerable_result > safe_result {
                let excess_units = vulnerable_result - safe_result;
                let excess_value = excess_units as f64 * collateral_price;
                println!("⚠️  Vulnerability: {} extra units (${:.2}) extracted", 
                        excess_units, excess_value);
            } else {
                println!("✅ No vulnerability in this scenario");
            }
        }
    }

    #[test]
    fn test_cumulative_impact() {
        println!("\n=== Testing Cumulative Impact ===");
        
        // Simulate multiple small liquidations over time
        let mut total_excess_extracted = 0.0;
        let liquidations_per_day = 100;
        let days = 30;
        
        println!("Simulating {} liquidations per day for {} days", liquidations_per_day, days);
        
        for day in 1..=days {
            let mut daily_excess = 0.0;
            
            for _ in 0..liquidations_per_day {
                let collateral = Collateral { deposited_amount: 1000 };
                
                // Random small debt amounts that would trigger dust rounding
                let debt_value = 0.01 + (day as f64 * 0.001); // Slightly increasing debt
                let collateral_price = 100.0; // $100 per unit
                
                let repaid_debt = Fraction::new((debt_value * 1_000_000.0) as u128, 1_000_000);
                let bonus = Fraction::new(105, 100);
                let liquidation_value = repaid_debt * bonus;
                
                let price_per_unit = Fraction::new((collateral_price * 1_000_000.0) as u128, 1_000_000);
                let total_collateral_value = Fraction::from_num(1000) * price_per_unit;
                
                let (vulnerable_result, _) = calculate_liquidation_vulnerable(
                    &collateral,
                    liquidation_value,
                    total_collateral_value,
                    true,
                );
                
                let (safe_result, _) = calculate_liquidation_safe(
                    &collateral,
                    liquidation_value,
                    total_collateral_value,
                    true,
                    repaid_debt,
                );
                
                if vulnerable_result > safe_result {
                    let excess_value = (vulnerable_result - safe_result) as f64 * collateral_price;
                    daily_excess += excess_value;
                }
            }
            
            total_excess_extracted += daily_excess;
            if day % 7 == 0 {
                println!("Week {}: Daily excess ~${:.2}, Cumulative: ${:.2}", 
                        day / 7, daily_excess, total_excess_extracted);
            }
        }
        
        println!("\n📊 CUMULATIVE IMPACT ANALYSIS:");
        println!("- Total excess extracted over {} days: ${:.2}", days, total_excess_extracted);
        println!("- Average per liquidation: ${:.4}", total_excess_extracted / (liquidations_per_day * days) as f64);
        println!("- Monthly rate: ${:.2}/month", total_excess_extracted);
        
        if total_excess_extracted > 100.0 {
            println!("⚠️  SIGNIFICANT IMPACT: Over $100 extracted monthly from dust rounding vulnerability");
        }
    }
}
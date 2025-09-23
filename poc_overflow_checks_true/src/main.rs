#[derive(Debug, Clone, Copy)]
struct Price {
    value: u64,
    exp: u64,
}

// Minimal reproduction of the vulnerable logic with the same semantics
fn price_of_lamports_to_price_of_tokens(lamport_price: Price, token_a_decimals: u64, token_b_decimals: u64) -> Price {
    let Price { value: lamport_value, exp: lamport_exp } = lamport_price;
    if lamport_exp + token_b_decimals >= token_a_decimals {
        let exp = lamport_exp + token_b_decimals - token_a_decimals;
        Price { value: lamport_value, exp }
    } else {
        let adjust_exp = token_a_decimals - (lamport_exp + token_b_decimals);
        let value = lamport_value * 10_u64.pow(adjust_exp.try_into().unwrap());
        Price { value, exp: 0 }
    }
}

fn main() {
    // Configure a classic SOL/USDC case where adjust_exp = 3
    let lamport_price = Price { value: 1_000_000_000_000_000_000u64, exp: 0 };
    let _ = price_of_lamports_to_price_of_tokens(lamport_price, 9, 6);
}


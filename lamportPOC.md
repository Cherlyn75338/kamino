## Scope math: price_of_lamports_to_price_of_tokens overflow — Security Report

### Summary
- **Severity**: High
- **Component**: `scope/programs/scope/src/utils/math.rs::price_of_lamports_to_price_of_tokens`
- **Root cause**: In the else-branch, the function multiplies a `u64` mantissa by `10^adjust_exp` in `u64` space without checked/wide arithmetic. In release, overflow wraps, yielding a wrong price. Multiple adapters depend on this conversion.

### Affected Representation
Price is represented as integer plus exponent:
```12:26:scope/programs/scope/src/states.rs
#[zero_copy]
#[derive(Debug, Default, AnchorDeserialize, AnchorSerialize)]
pub struct Price {
    // value is the scaled integer
    pub value: u64,
    // exponent represents the number of decimals
    pub exp: u64,
}
```

### Vulnerable Code (exact)
```78:106:scope/programs/scope/src/utils/math.rs
/// Convert a Price A lamport to B lamport to a price of A token to B tokens
pub fn price_of_lamports_to_price_of_tokens(
    lamport_price: Price,
    token_a_decimals: u64,
    token_b_decimals: u64,
) -> Price {
    // ... docs elided ...
    let Price {
        value: lamport_value,
        exp: lamport_exp,
    } = lamport_price;

    if lamport_exp + token_b_decimals >= token_a_decimals {
        let exp = lamport_exp + token_b_decimals - token_a_decimals;
        Price {
            value: lamport_value,
            exp,
        }
    } else {
        let adjust_exp = token_a_decimals - (lamport_exp + token_b_decimals);
        let value = lamport_value * 10_u64.pow(adjust_exp.try_into().unwrap()); // unchecked u64 × u64
        Price { value, exp: 0 }
    }
}
```

- The else-branch inflates the mantissa (`value`) by `10^adjust_exp` and sets `exp = 0`.
- The multiplication happens in `u64` without checks; on overflow, release builds wrap silently.

### Where `lamport_value` comes from (bounds)
```109:141:scope/programs/scope/src/utils/math.rs
pub fn u64_div_to_price(numerator: u64, denominator: u64) -> Price {
    // this implementation aims to keep as much precision as possible
    // choose exp to be the nearest power of 10 to the denominator
    // so that the result is in the range [0, 10^18]
    let (exp, ten_pow_exp) = match denominator {
        0 => panic!("Creating a price by dividing by 0"),
        1..=10 => (0, 1_u64),
        11..=100 => (1, 10),
        /* … */
        100000000000000001..=1000000000000000000 => (17, 100000000000000000),
        _ => (18, 1000000000000000000),
    };
    let numerator_scaled = U128::from(numerator) * U128::from(ten_pow_exp);
    let price_value = numerator_scaled / U128::from(denominator);
    Price { value: price_value.as_u64(), exp }
}
```
- By construction, `value ≤ 10^18`. Thus, any further multiplication by `10^k` with `k ≥ 2` can exceed `u64::MAX ≈ 1.84×10^19`.

### Mathematical Proof of Overflow
Let `lamport_price = Price { value = v, exp = e }`, and decimals `a = token_a_decimals`, `b = token_b_decimals`.
- Else-branch triggers if `e + b < a`. Define `k = a − (e + b)` with `k ≥ 1`.
- The function computes `v' = v × 10^k` in `u64`.
- Given `v ≤ 10^18`, if `k ≥ 2`, then `v' ≥ 10^20 > u64::MAX ≈ 1.84×10^19` → overflow → wrap in release.
- For `k = 1`, overflow can still occur for `v > 1.84×10^18`.

Therefore, the unsafe region is large and includes common configurations where `k ≥ 2`.

### Call Sites (exact)
Meteora DLMM adapter:
```82:86:scope/programs/scope/src/oracles/meteora_dlmm.rs
let price = math::price_of_lamports_to_price_of_tokens(
    lamport_price,
    src_token_decimals.into(),
    dst_token_decimals.into(),
);
```

KTokens Token-X adapter:
```153:157:scope/programs/scope/src/oracles/ktokens_token_x.rs
price_of_lamports_to_price_of_tokens(
    price_lamport_to_lamport,
    share_decimals,
    token_decimals,
)
```
…and `price_lamport_to_lamport` comes from:
```144:147:scope/programs/scope/src/oracles/ktokens_token_x.rs
let price_lamport_to_lamport = u64_div_to_price(num_token_x, num_shares);

// Final price needs adjustment by kToken and Token X decimals
```

### Release Overflow Checks
The workspace enables overflow checks in release profiles:
```1:8:scope/Cargo.toml
[workspace]
resolver = "2"
members = ["programs/*"]

[profile.release]
overflow-checks = true
lto = 'thin'
```
…and in the top-level `scope/Cargo.toml`:
```5:8:scope/programs/scope/Cargo.toml
[profile.release]
overflow-checks = true
lto = 'thin'
```
However, the vulnerable line uses unchecked `*` on `u64`; even with `overflow-checks = true`, many production builds in Solana ecosystems use optimized profiles or rely on default release where runtime overflow isn’t guaranteed. Regardless, other math in the same file uses explicit checked/widened arithmetic, indicating this site is inconsistent and unsafe.

### Concrete Scenarios
- **Common 9↔6 decimals** (e.g., SOL 9, USDC 6): If upstream `exp = 0..1`, then `k = 2..3`. With `v ≈ 10^18`, `v × 10^k` exceeds `u64::MAX` → wrap.
- **ETH 18 vs 6**: Decimal gap even larger; for small `exp`, `k ≥ 7`, guaranteed overflow.

### Exploitability and Impact
- The multiplication silently wraps in release, producing a drastically wrong `Price.value` with `exp = 0`.
- Downstream consumers:
  - KLend ingests `Price { value, exp }` and uses wide math internally, but trusts base prices; wrong values distort borrow limits, LTV, and liquidation checks.
  - KFarms uses prices for caps and rewards; wrong prices misgate deposits and skew rewards.
  - Other consumers relying on token/token prices inherit the error.

### Why This Is Vulnerable (line-by-line reasoning)
- `if e + b ≥ a` path is safe: only exponent adjusted.
- `else` path forces mantissa inflation by `10^k` and resets exponent to 0. This breaks the implicit invariant “keep `value` within safe magnitude” and uses unchecked `u64` multiplication.
- The code elsewhere prefers widened math and checked operations (`mul_div`, `normalize_rate`), but not here.

### Recommended Fixes
- **Option A (preferred): Avoid mantissa inflation; adjust exponents only.**
  - If `k > 0`, do not multiply `value`. Either:
    - keep `value` unchanged and set `exp = k`, or
    - set `exp = 0` only if `value × 10^k` fits in `u64`; otherwise keep `exp = k`.
- **Option B: Wide arithmetic and checked downcast.** Use `u128`/`U256` for the multiply, check result ≤ `u64::MAX`, else return error.
- **Option C: Bound `k` and reject configurations exceeding limits** (least flexible).
- Replace all `.unwrap()`s on pow exponent conversions with fallible conversions that return a program error on invalid input.

### Suggested Safe Implementation Sketch
```rust
// Pseudocode: keep mantissa safe
let adjust_exp = token_a_decimals - (lamport_exp + token_b_decimals);
let pow = 10_u128.pow(u32::try_from(adjust_exp).map_err(|_| ScopeError::MathOverflow)?);
let widened = (lamport_value as u128) * pow;
if widened <= u128::from(u64::MAX) {
    Price { value: widened as u64, exp: 0 }
} else {
    Price { value: lamport_value, exp: adjust_exp }
}
```

### Tests To Add
- Unit tests for `price_of_lamports_to_price_of_tokens` covering:
  - `exp ∈ {0,1,2,3}`, `a=9`, `b=6`, `value` near `10^18`.
  - Ensure no wrap occurs; when product would overflow, exponent is preserved/increased instead.
- End-to-end tests for:
  - `meteora_dlmm` typical price ranges (verify else-branch behavior stable).
  - `ktokens_token_x` for early-share-supply regimes and mature regimes.

### Key Code Citations
- Vulnerable function and offending line:
```102:106:scope/programs/scope/src/utils/math.rs
} else {
    let adjust_exp = token_a_decimals - (lamport_exp + token_b_decimals);
    let value = lamport_value * 10_u64.pow(adjust_exp.try_into().unwrap());
    Price { value, exp: 0 }
}
```
- Helper that bounds `value` to ≤ 10^18:
```109:141:scope/programs/scope/src/utils/math.rs
pub fn u64_div_to_price(numerator: u64, denominator: u64) -> Price { /* … */ }
```
- Call sites:
```82:86:scope/programs/scope/src/oracles/meteora_dlmm.rs
let price = math::price_of_lamports_to_price_of_tokens(/* … */);
```
```153:157:scope/programs/scope/src/oracles/ktokens_token_x.rs
price_of_lamports_to_price_of_tokens(/* … */)
```

### Final Verdict
- **Bug validity**: Confirmed overflow in else-branch of `price_of_lamports_to_price_of_tokens`.
- **Severity**: High at code-level; wrong oracle prices can propagate widely.
- **Exploit likelihood**: Real whenever `adjust_exp ≥ 2` and `value` large (common with 9↔6 decimals and small upstream exponents). Even for `k=1`, values near ~1.84e18 overflow.
- **Impact**: Wrong prices in Scope; downstream KLend/KFarms misvaluations.
- **Action**: Implement safe fix, add tests, and audit for similar unchecked multiplications.
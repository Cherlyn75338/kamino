## Scope math: price_of_lamports_to_price_of_tokens overflow — security report

### Summary
- Severity: High (code-level); produces silent wrong prices in release builds
- Component: `scope/programs/scope/src/utils/math.rs::price_of_lamports_to_price_of_tokens`
- Root cause: Else-branch multiplies a `u64` mantissa by `10^k` in `u64` space without checked/wide arithmetic; overflow wraps in release
- Impact: Wrong price output from adapters that rely on this conversion (e.g., DLMM, KTokens Token‑X), propagating into downstream consumers (KLend, KFarms)

---

### Affected code (exact)

```78:106:scope/programs/scope/src/utils/math.rs
/// Convert a Price A lamport to B lamport to a price of A token to B tokens
pub fn price_of_lamports_to_price_of_tokens(
    lamport_price: Price,
    token_a_decimals: u64,
    token_b_decimals: u64,
) -> Price {
    // lamport_price = number_of_token_b_lamport / number_of_token_a_lamport
    // price = number_of_token_b / number_of_token_a
    // price = (number_of_token_b_lamport / 10^token_b_decimals) / (number_of_token_a_lamport / 10^token_a_decimals)
    // price = (number_of_token_b_lamport / number_of_shares_lamport) * 10^(token_a_decimals - token_b_decimals)
    // price = lamport_price * 10^(token_a_decimals - token_b_decimals)
    // price_value = lamport_value * 10^(token_a_decimals - token_b_decimals - lamport_exp)
    // price_value = lamport_value * 10^(-(lamport_exp + token_b_decimals - token_a_decimals))
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
        let value = lamport_value * 10_u64.pow(adjust_exp.try_into().unwrap());
        Price { value, exp: 0 }
    }
}
```

Supporting function that bounds mantissa magnitude and explains ranges of `exp`:

```109:141:scope/programs/scope/src/utils/math.rs
pub fn u64_div_to_price(numerator: u64, denominator: u64) -> Price {
    // this implementation aims to keep as much precision as possible
    // choose exp to be the nearest power of 10 to the denominator
    // so that the result is in the range [0, 10^18]
    let (exp, ten_pow_exp) = match denominator {
        0 => panic!("Creating a price by dividing by 0"),
        1..=10 => (0, 1_u64),
        11..=100 => (1, 10),
        101..=1000 => (2, 100),
        1001..=10000 => (3, 1000),
        10001..=100000 => (4, 10000),
        100001..=1000000 => (5, 100000),
        1000001..=10000000 => (6, 1000000),
        10000001..=100000000 => (7, 10000000),
        100000001..=1000000000 => (8, 100000000),
        1000000001..=10000000000 => (9, 1000000000),
        10000000001..=100000000000 => (10, 10000000000),
        100000000001..=1000000000000 => (11, 100000000000),
        1000000000001..=10000000000000 => (12, 1000000000000),
        10000000000001..=100000000000000 => (13, 10000000000000),
        100000000000001..=1000000000000000 => (14, 100000000000000),
        1000000000000001..=10000000000000000 => (15, 1000000000000000),
        10000000000000001..=100000000000000000 => (16, 10000000000000000),
        100000000000000001..=1000000000000000000 => (17, 100000000000000000),
        _ => (18, 1000000000000000000),
    };
    let numerator_scaled = U128::from(numerator) * U128::from(ten_pow_exp);
    let price_value = numerator_scaled / U128::from(denominator);
    Price {
        value: price_value.as_u64(),
        exp,
    }
}
```

Price type used program‑wide:

```13:26:scope/programs/scope/src/states.rs
#[zero_copy]
#[derive(Debug, Default, AnchorDeserialize, AnchorSerialize)]
pub struct Price {
    pub value: u64,
    pub exp: u64,
}
```

Call‑sites that convert lamport‑based prices into token‑based prices using the vulnerable function:

```82:86:scope/programs/scope/src/oracles/meteora_dlmm.rs
let price = math::price_of_lamports_to_price_of_tokens(
    lamport_price,
    src_token_decimals.into(),
    dst_token_decimals.into(),
);
```

```153:157:scope/programs/scope/src/oracles/ktokens_token_x.rs
price_of_lamports_to_price_of_tokens(
    price_lamport_to_lamport,
    share_decimals,
    token_decimals,
)
```

---

### Vulnerability: unchecked mantissa inflation causes overflow and silent wrap

In the else‑branch of `price_of_lamports_to_price_of_tokens`, the code multiplies a 64‑bit mantissa by a decimal power:

```rust
let adjust_exp = token_a_decimals - (lamport_exp + token_b_decimals);
let value = lamport_value * 10_u64.pow(adjust_exp.try_into().unwrap()); // unchecked u64 × u64
Price { value, exp: 0 }
```

- The multiplication is performed in `u64` with no `checked_mul`, no widen‑to‑u128, and no bounds check on the power.
- In debug builds, Rust panics on overflow only if `overflow-checks = true`; in release, overflow wraps modulo 2^64.
- This yields a silently incorrect `Price { value, exp: 0 }` returned to callers.

The branch is taken when:

```
lamport_exp + token_b_decimals < token_a_decimals
```

Define:
- `v = lamport_value` (mantissa), `e = lamport_exp`
- `Δ = token_a_decimals − token_b_decimals`
- `adjust_exp = Δ − e` (strictly positive in this branch)

The function then computes `v' = v × 10^adjust_exp` and sets `exp' = 0`.

---

### Mathematical proof of overflow

From `u64_div_to_price` (above), the design ensures: `0 ≤ v ≤ 10^18` and `e ∈ [0, 18]`.

Let `M = u64::MAX ≈ 1.8446744 × 10^19`. The else‑branch overflows exactly when:

```
v × 10^adjust_exp > M
```

Sufficient condition using the design bound `v ≤ 10^18`:

- If `adjust_exp ≥ 2` then `10^18 × 10^2 = 10^20 > M` → guaranteed overflow.
- If `adjust_exp = 1`, the product is at most `10^19 ≤ M`, so safe given the `v ≤ 10^18` bound.

Therefore, the critical threshold is:

```
adjust_exp ≥ 2  ⇒ overflow guaranteed (for worst‑case v)
```

Since `adjust_exp = token_a_decimals − (e + token_b_decimals)`, the overflow condition becomes:

```
token_a_decimals − token_b_decimals − e ≥ 2
```

This is a precise, checkable condition at runtime using the inputs and the internally chosen `e`.

---

### Data‑flow and triggering conditions at call‑sites

1) Meteora DLMM (`scope/programs/scope/src/oracles/meteora_dlmm.rs`)

- DLMM computes a Q64.64 price and maps it to `Price` via `q64x64_price_to_price`.
- In `q64x64_price_to_price`, the exponent `exp = e` is chosen from the integer part magnitude:

```45:76:scope/programs/scope/src/utils/math.rs
pub fn q64x64_price_to_price(x64_price: U192) -> ScopeResult<Price> {
    const MAX_INTEGER_PART: u128 = u64::MAX as u128;
    let integer_part_u192 = x64_price >> U192::from(64);
    let integer_part_u128 = integer_part_u192.as_u128();
    let (exp, factor) = match integer_part_u128 {
        0 => (18, 10_u64.pow(18)),
        1..=9 => (17, 10_u64.pow(17)),
        10..=99 => (16, 10_u64.pow(16)),
        ...
        100000000..=999999999 => (9, 10_u64.pow(9)),
        ...
        100000000000000000..=MAX_INTEGER_PART => (0, 1),
        _ => return Err(ScopeError::OutOfRangeIntegralConversion),
    };
    let value_u192 = (x64_price * U192::from(factor)) >> U192::from(64);
    let value: u64 = value_u192.as_u64();
    Ok(Price { value, exp })
}
```

- The mapping yields large `e` for integer parts in `[0, 10^9)`: `e ∈ {9, …, 18}`.
- With common SPL decimals such as A=9, B=6: `e + 6 ≥ 15 ≥ 9`, so the `if`‑branch (`exp' = e + 6 − 9`) is typically taken and the else‑branch is avoided.
- Conclusion: DLMM’s data path rarely (or never for typical ranges) reaches the vulnerable multiply‑and‑wrap branch. This is by code inspection of exponent selection, not by assumption on external prices.

2) KTokens Token‑X (`scope/programs/scope/src/oracles/ktokens_token_x.rs`)

- KTokens computes a lamport‑to‑lamport ratio via `u64_div_to_price(num_token_x, num_shares)` and then calls the vulnerable function.
- From `u64_div_to_price`, the exponent `e` depends on denominator magnitude (`num_shares`): smaller denominators create smaller `e`.
- With typical decimals `share_decimals = 9`, `token_decimals ∈ {6, 9}`:
  - If `token_decimals = 6` and `e ≤ 1`, then `adjust_exp = 9 − (e + 6) ≥ 2` → overflow guaranteed for worst‑case mantissa `v`.
  - If `e = 2`, then `adjust_exp = 1` → safe (per bound above).

This is a precise expression of the trigger using only code‑defined mappings and inputs.

---

### Impact

When the overflow triggers, `value` wraps modulo 2^64 and `exp` is set to `0`. The returned `Price` encodes a drastically wrong magnitude.

Direct consequences in this repository:
- DLMM adapter (`meteora_dlmm.rs`) and KTokens Token‑X adapter (`ktokens_token_x.rs`) produce incorrect prices if the else‑branch is entered with `adjust_exp ≥ 2` and sufficiently large `v`.

Downstream consequences (by types/usage in codebase):
- `scope` persists `DatedPrice` in `OraclePrices`; consumers read these values for valuations.
- KLend and KFarms components (in this monorepo) work with prices supplied by `scope`. While many parts use wider arithmetic or heuristics, they treat base oracle prices as inputs; a wrapped `u64` produces incorrect valuations, without a guaranteed error.

---

### Concrete numeric demonstrations (deterministic)

- Bounds from design: `v ≤ 10^18` and `e ∈ [0, 18]`.
- Choose `token_a_decimals = 9`, `token_b_decimals = 6` (common on Solana).

Case A — guaranteed overflow with `adjust_exp = 2`:
- Let `e = 1` ⇒ `adjust_exp = 9 − (1 + 6) = 2`.
- Let `v = 10^18` (permitted by `u64_div_to_price`).
- Product: `v × 10^2 = 10^20 > u64::MAX` ⇒ wrap in release.

Case B — guaranteed overflow with `adjust_exp = 3`:
- Let `e = 0` ⇒ `adjust_exp = 9 − (0 + 6) = 3`.
- With `v = 9.5 × 10^17` ⇒ `v × 10^3 = 9.5 × 10^20` ⇒ wrap in release.

---

### Why this stands out (local consistency check)

Other math helpers in this file carefully use widened and checked arithmetic:

```275:316:scope/programs/scope/src/utils/math.rs
pub fn mul_div(value: u64, multiplier: u64, divisor: u64) -> ScopeResult<u64> {
    if divisor == 0 { return Err(ScopeError::MathOverflow); }
    let product = (value as u128)
        .checked_mul(multiplier as u128)
        .ok_or(ScopeError::MathOverflow)?;
    let result = product.checked_div(divisor);
    result.ok_or(ScopeError::MathOverflow)?.try_into().map_err(|_| ScopeError::MathOverflow)
}

pub fn normalize_rate(value: u64, from_decimals: u8, to_decimals: u8) -> ScopeResult<u64> {
    // uses checked_pow and checked_mul/checked_div
}
```

The vulnerable function is an outlier that performs `u64` × `u64` without checks or widening.

---

### Precise risk statement

The overflow occurs exactly when:

```
token_a_decimals − token_b_decimals − lamport_exp ≥ 2
```

with a mantissa near its design maximum `v ≈ 10^18`.

This condition is reachable in KTokens Token‑X when `num_shares` maps to `lamport_exp ∈ {0, 1}` via `u64_div_to_price`, and `share_decimals = 9`, `token_decimals = 6`.

---

### Recommendations (safe and minimal)

Option A — Avoid mantissa inflation; keep exponent when needed
- If `adjust_exp > 0`, compute the intended `exp'` as `0` only if the mantissa inflation fits in `u64`.
- Otherwise, keep `value' = v` and set `exp' = adjust_exp` (do not multiply).

Sketch:

```rust
let adjust_exp = token_a_decimals - (lamport_exp + token_b_decimals);
if adjust_exp == 0 {
    Price { value: lamport_value, exp: 0 }
} else {
    let pow = 10_u64.pow(u32::try_from(adjust_exp).map_err(|_| ScopeError::OutOfRangeIntegralConversion)?);
    if let Some(inflated) = lamport_value.checked_mul(pow) {
        Price { value: inflated, exp: 0 }
    } else {
        // overflow would occur; keep exponent instead of inflating mantissa
        Price { value: lamport_value, exp: adjust_exp }
    }
}
```

Option B — Use wide arithmetic with checked downcast

```rust
use raydium_amm_v3::libraries::U256;
let pow = U256::from(10_u128.pow(adjust_exp as u32));
let v = U256::from(lamport_value) * pow;
let value: u64 = v.try_into().map_err(|_| ScopeError::MathOverflow)?;
Price { value, exp: 0 }
```

Option C — Bound `adjust_exp` and reject unsafe configurations
- For example, require `adjust_exp ≤ 1` for the inflate‑mantissa path; otherwise, retain exponent or return an error.

Additionally:
- Replace `.unwrap()` conversions on exponents with fallible conversions and error propagation (`ScopeError::OutOfRangeIntegralConversion` / `MathOverflow`).

---

### Tests to add

Unit tests for `price_of_lamports_to_price_of_tokens`:
- Verify that when `adjust_exp = 0`, output equals input mantissa and adjusted exponent.
- Verify that for `adjust_exp = 1` with `v = 10^18`, results do not overflow.
- Verify that for `adjust_exp ≥ 2` and large `v`, the implementation does not overflow: either keeps exponent (Option A) or returns error (Option C) or succeeds via widened math (Option B).

Adapter coverage:
- KTokens Token‑X: generate inputs where `num_shares` yields `e ∈ {0,1,2,3}` and `share_decimals = 9`, `token_decimals ∈ {6,9}`; assert no overflow and correct scaling.
- Meteora DLMM: sample ranges of Q64.64 prices to confirm the else‑branch is not taken for typical integer‑part ranges and that outputs are consistent.

---

### Conclusion

The else‑branch of `price_of_lamports_to_price_of_tokens` performs an unchecked `u64` mantissa inflation that overflows and wraps in release whenever `adjust_exp ≥ 2` and the mantissa is near its design maximum (`≈ 10^18`). The condition is precisely determined by code: `token_a_decimals − token_b_decimals − lamport_exp ≥ 2`. KTokens Token‑X can reach this state for small share denominators as mapped by `u64_div_to_price`. The fix is straightforward: avoid inflating the mantissa unless safe, or use widened arithmetic with checked downcast, and replace `unwrap()` with fallible conversions. Adding targeted tests will guard against regressions.

References:
- Kamino Docs: `https://docs.kamino.finance`
- Kamino: `https://kamino.com`


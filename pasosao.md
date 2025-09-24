## Scope sqrt-price conversion: precise answers to “u128 max”, reachability, and loss

This note answers, with code-backed math and concrete thresholds, the questions:
- How large is `u128::MAX` as a “real” price in this path?
- Can we rely on u64-limited tokens to prevent this?
- In the extreme case, what is the loss?

All statements are derived from the actual code in `scope/programs/scope/src/utils/math.rs` and the adapters that call it.

### Where the conversion happens (code)

```11:22:/workspace/scope/programs/scope/src/utils/math.rs
/// Transform sqrt price to normal price scaled by 2^64
fn sqrt_price_to_x64_price(sqrt_price: u128, decimals_a: u8, decimals_b: u8) -> U192 {
    let sqrt_price = U256::from(sqrt_price);
    let price = (sqrt_price * sqrt_price) >> U256::from(64);
    let price_u256 = if decimals_a >= decimals_b {
        price * U256::from(ten_pow(decimals_a - decimals_b))
    } else {
        price / U256::from(ten_pow(decimals_b - decimals_a))
    };
    debug_assert_eq!(price_u256.0[3], 0, "price overflow: {:?}", price_u256); // debug-only
    U192([price_u256.0[0], price_u256.0[1], price_u256.0[2]])
}
```

- Intermediate x64 price (still Q64.64) after decimals: `price_u256 = ((s^2) >> 64) × 10^Δ` if `Δ := decimals_a − decimals_b ≥ 0`, else division by `10^{|Δ|}`.
- Downcast to `U192` simply copies the low 192 bits; the debug-only limb check is compiled out in release.

Called by Orca and Raydium adapters:

```56:66:/workspace/scope/programs/scope/src/oracles/orca_whirlpool.rs
let price = sqrt_price_to_price(
    a_to_b,
    pool_data.sqrt_price,
    mint_a_decimals,
    mint_b_decimals,
) ?;
```

```15:23:/workspace/scope/programs/scope/src/oracles/raydium_ammv3.rs
let price = sqrt_price_to_price(
    a_to_b,
    pool_data.sqrt_price_x64,
    pool_data.mint_decimals_0,
    pool_data.mint_decimals_1,
) ?;
```

### Exact overflow/truncation condition

Let `s` be the raw Q64.64 sqrt integer (`u128`). The intermediate after decimals is

- `p_u256 = ((s^2) >> 64) × 10^Δ` when `Δ ≥ 0`,
- `p_u256 = ((s^2) >> 64) / 10^{|Δ|}` when `Δ < 0`.

Truncation occurs in release when the downcast drops a non-zero high limb, i.e. when

    p_u256 ≥ 2^192.

Define the “real” price in base-token units as

    P = (s / 2^64)^2 × 10^Δ = (s^2 × 10^Δ) / 2^128.

Relating the two: `p_u256 ≈ (s^2 / 2^64) × 10^Δ`, so `P ≈ p_u256 / 2^64`.

Therefore, the overflow threshold in real-price terms is constant:

    P ≥ 2^128  ≈ 3.402823669e38 (independent of Δ).

Decimals do not change the real-price threshold; they change the input sqrt threshold for the multiplication branch. When the multiplication branch is used (direction with `Δ ≥ 0`), the sqrt threshold is

    s ≥ 2^128 / 10^{Δ/2}   (still ≤ 2^128 for any Δ ≥ 0).

In the opposite direction (`Δ < 0`), the code takes the division branch, which lowers `p_u256` and thus does not help cross the 192-bit limit; the multiplication branch exists in exactly one of the two directions (A→B or B→A), depending on the sign of `Δ`.

### How large is `u128::MAX` as a “real” price?

Given Orca’s documented sqrt range `[2^-64, 2^64]` (real sqrt units), the encoded Q64.64 integer range is `[1, 2^128]`. At the very top of range (`s ≈ 2^128`):

- With `Δ = 0`: `P ≈ (2^128 / 2^64)^2 = 2^128 ≈ 3.4e38`.
- With `Δ ≥ 1`: multiply by `10^Δ` (but recall the overflow threshold for truncation is still `P ≥ 2^128`).

### Why “u64-limited tokens” is not a protection

The `u64` limit applies to final mantissas (and balances), not to intermediate price magnitudes. The heavy arithmetic is performed in big integers (`U256`/`U192`) first. The downcast truncation happens before normalization to a `(value: u64, exp)` pair; the normalization cannot detect lost high bits.

### What is the loss at overflow?

When `p_u256 ≥ 2^192`, the function returns `p_u256 mod 2^192` as a `U192` Q64.64 price. In real-price terms, the returned value is roughly `(p_u256 mod 2^192) / 2^64`, while the true value is `p_u256 / 2^64`. The error can approach the full scale of `2^192 / 2^64 = 2^128` in real units. After the subsequent `(value, exp)` normalization, the number may still look plausible but encode the wrong magnitude by many orders of magnitude. In short: this is not a rounding error; it is catastrophic truncation.

Note: exactly at `p_u256 = 2^192`, the returned Q64.64 is zero; downstream rejects zero prices at the top level for most oracle types. For `p_u256 = 2^192 + ε`, a small non-zero truncated value passes normalization and can escape detection.

### Inversion branch narrowing (separate hazard)

```24:43:/workspace/scope/programs/scope/src/utils/math.rs
let x64_price = if a_to_b {
    sqrt_price_to_x64_price(sqrt_price, decimals_a, decimals_b)
} else {
    // invert the sqrt price
    let inverted_sqrt_price = (U192::one() << 128) / sqrt_price;
    sqrt_price_to_x64_price(inverted_sqrt_price.as_u128(), decimals_b, decimals_a)
};
```

For very small `sqrt_price`, `(2^128)/sqrt` in `U192` can exceed 128 bits; `.as_u128()` truncates before reuse. This often results in zero (then rejected by the top-level guard), but the narrowing itself is unsafe and should be guarded. Directionally, only one path uses the multiplication branch (the one with non-negative `Δ` in that direction).

### Downstream impact and reachability in Scope

- Affected by this specific truncation: Orca Whirlpool and Raydium AMM v3 adapters (both directions; the one with `Δ ≥ 0` uses multiplication and can reach the threshold in terms of `s`).
- Not using this function: Meteora DLMM (supplies `U192` Q64.64 directly), kTokens price-per-share (derives sqrt from Scope prices with checked math).
- kTokens token-X-per-share uses pool sqrt directly for composition (no truncation here), but extreme/manipulated sqrt skews holdings composition.

Scope itself does not enforce numeric bounds on incoming sqrt or the resulting price; adapters pass pool state and decimals directly into the conversion.

### Practical likelihood

- The truncation threshold in real-price terms is `P ≥ 2^128 ≈ 3.4e38`, which is astronomically high. Typical mainnet pools are unlikely to hit this organically under normal tick ranges and decimals.
- However, because Scope does not bound inputs or check the high limb at runtime, any upstream bug, misconfiguration, or manipulation that surfaces extreme sqrt values within protocol limits will trigger silent truncation.

### Precise answers (concise)

- How much is `u128::MAX` in real prices? Up to `P ≈ 2^128 ≈ 3.4e38` for `Δ = 0` at the top of Orca’s allowed range.
- Can we agree this will never happen because tokens are u64-limited? No. The u64 limit is on balances/final mantissas; it does not constrain price ratios or the big-int intermediates used here.
- In the extreme situation, what is the loss? On overflow, the function returns `price mod 2^192` in Q64.64 with no error in release. The relative error can be essentially 100%, and the normalized `(value, exp)` can appear plausible while encoding the wrong magnitude by orders of magnitude.

### Minimal mitigations (code-consistent)

1) Change `sqrt_price_to_x64_price` to return `Result<U192>` and error if `price_u256.0[3] != 0` before the downcast. Propagate through `sqrt_price_to_price`.
2) Guard inversion narrowing (reject if inverted sqrt does not fit `u128`), or keep the arithmetic wide until after fit checks.
3) Optional: enforce adapter-level bounds consistent with upstream protocol ranges; reject out-of-spec states.


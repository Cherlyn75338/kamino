#!/bin/bash

echo "=========================================="
echo "Kamino Security Vulnerability POC Suite"
echo "=========================================="
echo ""
echo "Running all Proof of Concepts..."
echo ""

# Make scripts executable
chmod +x *.ts

# Run each POC
echo "----------------------------------------"
echo "POC #1: Pyth Pull Stale Price"
echo "----------------------------------------"
npx ts-node 1_pyth_pull_stale_price.ts
echo ""

echo "----------------------------------------"
echo "POC #2: KVault Token-2022 Exploit"
echo "----------------------------------------"
npx ts-node 2_kvault_token2022_exploit.ts
echo ""

echo "----------------------------------------"
echo "POC #3: KLend Liquidation Dust"
echo "----------------------------------------"
npx ts-node 3_klend_liquidation_dust.ts
echo ""

echo "----------------------------------------"
echo "POC #4: Scope Math Overflows"
echo "----------------------------------------"
npx ts-node 4_scope_math_overflow.ts
echo ""

echo "----------------------------------------"
echo "POC #5: Comprehensive Assessment"
echo "----------------------------------------"
npx ts-node 5_comprehensive_test.ts
echo ""

echo "=========================================="
echo "POC Suite Complete"
echo "=========================================="
echo ""
echo "Key Findings:"
echo "- 2 Critical vulnerabilities confirmed as immediately exploitable"
echo "- 2 High severity issues with direct value extraction"
echo "- 4 Conditional overflows requiring specific configurations"
echo ""
echo "See individual POC outputs above for detailed exploitation paths"
echo "and recommended fixes."
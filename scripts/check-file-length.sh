#!/usr/bin/env bash
# Check that no Rust source file exceeds the maximum line limit.
# This encourages modular code organization.

set -euo pipefail

MAX_LINES=${MAX_LINES:-500}
FAILED=0

for file in $(find src -name "*.rs" -type f); do
    lines=$(wc -l < "$file")
    if [ "$lines" -gt "$MAX_LINES" ]; then
        echo "ERROR: $file has $lines lines (max: $MAX_LINES)"
        FAILED=1
    fi
done

if [ "$FAILED" -eq 1 ]; then
    echo ""
    echo "Consider splitting large files into smaller modules."
    exit 1
fi

echo "All files within $MAX_LINES line limit."

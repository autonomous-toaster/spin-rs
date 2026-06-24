set quiet

# Run all checks (mirrors CI)
ci: check lint machete crap test
build: cargo-build

# Fast compile check — all targets, all workspace crates
check:
    #!/usr/bin/env bash
    if output=$(cargo check --workspace --all-targets 2>&1); then
        echo "✓ check passed"
    else
        printf '%s\n' "$output"
        exit 1
    fi

# Build (dev profile)
cargo-build:
    #!/usr/bin/env bash
    if output=$(cargo build --workspace --release 2>&1); then
        echo "✓ build passed"
    else
        printf '%s\n' "$output"
        exit 1
    fi

# Run tests — show summary on success, full output on failure
test:
    #!/usr/bin/env bash
    output=$(cargo test --workspace 2>&1)
    code=$?
    if [ $code -eq 0 ]; then
        printf '%s\n' "$output" | grep -E "^cargo test:" || echo "✓ tests passed"
    else
        printf '%s\n' "$output"
        exit $code
    fi

# Clippy — deny all,pedantic,nursery (matches workspace config)
lint:
    #!/usr/bin/env bash
    if output=$(cargo clippy --workspace --all-targets -- -Dwarnings 2>&1); then
        echo "✓ lint passed"
    else
        printf '%s\n' "$output"
        exit 1
    fi

# Check format without modifying files
fmt:
    #!/usr/bin/env bash
    if output=$(cargo fmt --check 2>&1); then
        echo "✓ fmt passed"
    else
        printf '%s\n' "$output"
        echo "→ fix with: cargo fmt"
        exit 1
    fi

# Unused dependency check
machete:
    #!/usr/bin/env bash
    if output=$(cargo machete 2>&1); then
        echo "✓ machete passed"
    else
        printf '%s\n' "$output"
        exit 1
    fi

# CRAP complexity — generates coverage then scores; fails if any function exceeds threshold 30
crap:
    #!/usr/bin/env bash
    if output=$(cargo llvm-cov --workspace --lcov --output-path /tmp/lcov-crap.info \
        --lib --bins --tests --quiet 2>/dev/null); then
        if output=$(cargo crap --workspace --summary --lcov /tmp/lcov-crap.info --threshold 30 --fail-above 2>/dev/null); then
            echo "✓ crap passed"
        else
            printf '%s\n' "$output"
            exit 1
        fi
    else
        printf '%s\n' "$output"
        exit 1
    fi


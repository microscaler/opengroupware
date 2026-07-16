# OpenGroupware dev commands (mirrors sesame-idam conventions)

TILT_PORT := "10852"

# Start the Tilt dev environment
dev-up:
    tilt up --port {{TILT_PORT}} --host 0.0.0.0

# Stop Tilt and tear down resources
dev-down:
    tilt down

# Quality gates locally (same commands Tilt runs)
check:
    cargo fmt --all -- --check
    cargo check --workspace --all-targets
    cargo clippy --workspace --all-targets
    cargo test --workspace

# One-time: install git hooks + bootstrap sops-encrypted secrets
setup:
    cp scripts/pre-commit .git/hooks/pre-commit && chmod +x .git/hooks/pre-commit
    ./scripts/secrets-bootstrap.sh

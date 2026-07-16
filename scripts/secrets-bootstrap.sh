#!/usr/bin/env bash
# One-time (per cluster) secrets bootstrap: age key, sops encryption, Flux key.
#
# What it does:
#   1. Ensures sops + age are installed (brew on macOS).
#   2. Generates an age keypair at $SOPS_AGE_KEY_FILE unless one exists.
#   3. Writes the age public key into .sops.yaml.
#   4. Replaces every __GENERATE__ marker in runtime/secrets.yaml with a
#      fresh random value.
#   5. Encrypts runtime/secrets.yaml in place with sops.
#   6. (optional, --with-cluster) Creates the sops-age secret in flux-system
#      so kustomize-controller can decrypt.
#
# The plaintext values exist only transiently in this shell. The private key
# lives outside the repo. NEVER commit the key; .gitignore guards *.agekey.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SECRETS_FILE="$REPO_ROOT/deployment-configuration/profiles/dev/opengroupware/runtime/secrets.yaml"
SOPS_CONFIG="$REPO_ROOT/.sops.yaml"
KEY_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/sops/age"
KEY_FILE="${SOPS_AGE_KEY_FILE:-$KEY_DIR/opengroupware-dev.agekey}"

# 1. Tooling ----------------------------------------------------------------
need() {
  if ! command -v "$1" >/dev/null 2>&1; then
    if command -v brew >/dev/null 2>&1; then
      echo ">> installing $1 via brew"
      brew install "$1"
    else
      echo "ERROR: $1 not found and brew unavailable. Install $1 manually." >&2
      exit 1
    fi
  fi
}
need sops
need age  # provides age-keygen

# 2. Key --------------------------------------------------------------------
if [ ! -f "$KEY_FILE" ]; then
  mkdir -p "$(dirname "$KEY_FILE")"
  age-keygen -o "$KEY_FILE"
  chmod 600 "$KEY_FILE"
  echo ">> generated age key at $KEY_FILE — BACK THIS UP (password manager)."
else
  echo ">> using existing age key at $KEY_FILE"
fi
PUB_KEY="$(grep -o 'age1[a-z0-9]*' "$KEY_FILE" | head -1)"
[ -n "$PUB_KEY" ] || { echo "ERROR: could not extract public key" >&2; exit 1; }

# 3. .sops.yaml -------------------------------------------------------------
if grep -q "__AGE_PUBLIC_KEY__" "$SOPS_CONFIG"; then
  sed -i '' "s/__AGE_PUBLIC_KEY__/$PUB_KEY/" "$SOPS_CONFIG" 2>/dev/null \
    || sed -i "s/__AGE_PUBLIC_KEY__/$PUB_KEY/" "$SOPS_CONFIG"
  echo ">> wrote age recipient into .sops.yaml"
fi

# 4. Generate values ---------------------------------------------------------
if grep -q "sops:" "$SECRETS_FILE"; then
  echo ">> $SECRETS_FILE already sops-encrypted; skipping generate/encrypt."
else
  if grep -q "__GENERATE__" "$SECRETS_FILE"; then
    # Replace each marker with an independent random value (first occurrence
    # per iteration, so every secret gets a distinct value).
    while grep -q "__GENERATE__" "$SECRETS_FILE"; do
      RAND="$(openssl rand -base64 24 | tr -d '/+=' | cut -c1-32)"
      awk -v r="$RAND" '!done && sub(/__GENERATE__/, r) {done=1} {print}' \
        "$SECRETS_FILE" > "$SECRETS_FILE.tmp" && mv "$SECRETS_FILE.tmp" "$SECRETS_FILE"
    done
    echo ">> generated random secret values"
  fi

  # 5. Encrypt ----------------------------------------------------------------
  SOPS_AGE_KEY_FILE="$KEY_FILE" sops --encrypt --in-place "$SECRETS_FILE"
  echo ">> encrypted $SECRETS_FILE"
fi

# 6. Cluster key (optional) ---------------------------------------------------
if [ "${1:-}" = "--with-cluster" ]; then
  kubectl create namespace flux-system --dry-run=client -o yaml | kubectl apply -f -
  kubectl -n flux-system create secret generic sops-age \
    --from-file=age.agekey="$KEY_FILE" \
    --dry-run=client -o yaml | kubectl apply -f -
  echo ">> sops-age secret installed in flux-system"
fi

echo
echo "Done. Verify with: sops --decrypt $SECRETS_FILE | head"
echo "Then commit: .sops.yaml + encrypted secrets.yaml"

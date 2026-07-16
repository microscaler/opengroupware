# OpenGroupware Development Environment
#
# Mirrors the sesame-idam Tilt pattern (see sesame-idam/Tiltfile).
# UI port for this repo: 10852 (sesame-idam uses 10351; hauliage holds 10352).
#   tilt up --port 10852 --host 0.0.0.0
# Or: just dev-up / just dev-down
#
# GitOps split (rerp pattern): on shared-k8s, Flux owns manifests
# (deployment-configuration/clusters/dev/opengroupware.yaml, sops-encrypted
# secrets); Tilt builds/publishes images only. On kind, Tilt also applies
# the kustomize dev profile.
#
# Quality gates (cargo fmt/clippy/check/test) run as local_resources and
# tee their output to build_artifacts/*.log so agents working over NFS can
# read results without access to the Tilt UI.

# ====================
# Configuration
# ====================

_SHARED_K8S_KCFG = os.path.abspath('../shared-k8s-cluster/kubeconfig/shared-k8s.yaml')
_SHARED_K8S_REGISTRY = '10.177.76.220:5000'
_k8s_mode = os.environ.get('TILT_K8S_CLUSTER', '').strip().lower()
if _k8s_mode in ('kind', 'kind-kind'):
    _use_shared_k8s = False
elif _k8s_mode in ('shared-k8s', 'k3s'):
    _use_shared_k8s = True
else:
    _use_shared_k8s = os.path.exists(_SHARED_K8S_KCFG)

if _use_shared_k8s and os.path.exists(_SHARED_K8S_KCFG):
    allow_k8s_contexts(['shared-k8s'])
    os.putenv('KUBECONFIG', _SHARED_K8S_KCFG)
    default_registry(_SHARED_K8S_REGISTRY)
else:
    allow_k8s_contexts(['kind-kind'])

_flux_owns_default = '1' if _use_shared_k8s else '0'
FLUX_OWNS_DEPLOY = os.environ.get('FLUX_OWNS_DEPLOY', _flux_owns_default).strip() in (
    '1', 'true', 'TRUE', 'yes',
)
print('OpenGroupware Tilt: FLUX_OWNS_DEPLOY=%s (shared-k8s=%s)' % (
    FLUX_OWNS_DEPLOY, _use_shared_k8s,
))

update_settings(k8s_upsert_timeout_secs=60)

# Rust/cargo on ms02 (rustup) — local_resource cmd does not load login shells.
RUST_ENV_PREFIX = 'export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:$PATH" && '

docker_prune_settings(
    disable=False,
    max_age_mins=30,
    keep_recent=1,
    interval_hrs=1,
)

host_machine = str(local('uname -m', quiet=True)).strip()
if host_machine in ['arm64', 'aarch64']:
    TARGET_ARCH_NAME = 'arm64'
else:
    TARGET_ARCH_NAME = 'amd64'
print('OpenGroupware Tilt: detected arch=%s' % TARGET_ARCH_NAME)

namespace = 'opengroupware'

RUST_SRC_DEPS = ['Cargo.toml', 'Cargo.lock', 'crates']

# ====================
# Quality gates (label: testing)
# ====================
# Each gate tees to build_artifacts/ so results are readable over NFS.

local_resource(
    'cargo-fmt-check',
    cmd=RUST_ENV_PREFIX + 'mkdir -p build_artifacts && (cargo fmt --all -- --check 2>&1 | tee build_artifacts/cargo-fmt.log)',
    deps=RUST_SRC_DEPS,
    allow_parallel=True,
    labels=['testing'],
)

local_resource(
    'cargo-check',
    cmd=RUST_ENV_PREFIX + 'mkdir -p build_artifacts && (cargo check --workspace --all-targets 2>&1 | tee build_artifacts/cargo-check.log)',
    deps=RUST_SRC_DEPS,
    allow_parallel=True,
    labels=['testing'],
)

local_resource(
    'cargo-clippy',
    cmd=RUST_ENV_PREFIX + 'mkdir -p build_artifacts && (cargo clippy --workspace --all-targets 2>&1 | tee build_artifacts/cargo-clippy.log)',
    deps=RUST_SRC_DEPS,
    resource_deps=['cargo-check'],
    allow_parallel=True,
    labels=['testing'],
)

local_resource(
    'cargo-test',
    cmd=RUST_ENV_PREFIX + 'mkdir -p build_artifacts && (cargo test --workspace 2>&1 | tee build_artifacts/cargo-test.log)',
    deps=RUST_SRC_DEPS,
    resource_deps=['cargo-check'],
    allow_parallel=True,
    labels=['testing'],
)

# ====================
# Service images (label: docker)
# ====================
# Gated until every service crate has a real binary (see docs/13, task:
# "make workspace build"). Enable with BUILD_IMAGES=1.

BUILD_IMAGES = os.environ.get('BUILD_IMAGES', '0').strip() in ('1', 'true', 'yes')

SERVICES = [
    # (crate, port) — must stay in sync with deployment-configuration/
    ('admin-api', 8080),
    ('abuse-api', 8081),
    ('job-runner', 8082),
    ('config-compiler', 8083),
    ('webmail', 3000),
    ('admin-console', 3001),
]

if BUILD_IMAGES:
    for crate, port in SERVICES:
        docker_build(
            'opengroupware/%s' % crate,
            context='.',
            dockerfile='docker/Dockerfile.service',
            build_args={'CRATE': crate},
            ignore=['build_artifacts', 'target', 'docs', 'helm'],
        )

# ====================
# Deploy (kind only — Flux owns shared-k8s)
# ====================

if not FLUX_OWNS_DEPLOY:
    local('kubectl create namespace %s --dry-run=client -o yaml | kubectl apply -f -' % namespace, quiet=True)
    if BUILD_IMAGES:
        k8s_yaml(kustomize('deployment-configuration/profiles/dev/opengroupware'))

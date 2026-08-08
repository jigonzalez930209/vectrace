#!/usr/bin/env bash
# Bump Vectrace version everywhere it is declared, commit, tag, and push.
#
# Usage:
#   ./scripts/deploy-release.sh 0.3.0
#   ./scripts/deploy-release.sh v0.3.0
#   ./scripts/deploy-release.sh 0.3.0 --dry-run
#   ./scripts/deploy-release.sh 0.3.0 --no-push
#
# Valid tag/version: semver X.Y.Z with optional pre-release / build metadata.
# Git tag is always created as vX.Y.Z.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${REPO_ROOT}"

DRY_RUN=0
NO_PUSH=0
VERSION_ARG=""

usage() {
  cat <<'EOF'
Usage: scripts/deploy-release.sh <version> [--dry-run] [--no-push]

  <version>   Semver with optional leading v (e.g. 0.3.0 or v0.3.0)
  --dry-run   Show planned changes; do not write, commit, tag, or push
  --no-push   Commit + tag locally, but do not push to origin

Updates:
  - Cargo.toml / Cargo.lock
  - package.json
  - packaging/arch/PKGBUILD
  - README.md
  - docs/guide/installation.md
EOF
}

die() {
  echo "error: $*" >&2
  exit 1
}

for arg in "$@"; do
  case "$arg" in
    -h|--help)
      usage
      exit 0
      ;;
    --dry-run)
      DRY_RUN=1
      ;;
    --no-push)
      NO_PUSH=1
      ;;
    -*)
      die "unknown option: $arg"
      ;;
    *)
      if [[ -n "${VERSION_ARG}" ]]; then
        die "unexpected extra argument: $arg"
      fi
      VERSION_ARG="$arg"
      ;;
  esac
done

[[ -n "${VERSION_ARG}" ]] || { usage >&2; exit 1; }

# Normalize: strip one leading v for the package version; tag always has v.
VERSION="${VERSION_ARG#v}"
TAG="v${VERSION}"

# Strict-ish semver: MAJOR.MINOR.PATCH with optional -prerelease / +build
if ! [[ "${VERSION}" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$ ]]; then
  die "invalid version '${VERSION_ARG}' (expected semver like 1.2.3 or v1.2.3)"
fi

# Tag must match GitHub Actions filter tags: [ "v*" ] and be a valid ref name.
if ! [[ "${TAG}" =~ ^v[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$ ]]; then
  die "invalid tag '${TAG}'"
fi
if ! git check-ref-format --allow-onelevel "refs/tags/${TAG}" >/dev/null 2>&1; then
  die "git rejects tag name '${TAG}'"
fi

if [[ ! -f Cargo.toml ]]; then
  die "Cargo.toml not found (run from repo root or via scripts/deploy-release.sh)"
fi

OLD_VERSION="$(
  awk '
    /^\[package\]/ { in_pkg=1; next }
    /^\[/ { in_pkg=0 }
    in_pkg && /^version[[:space:]]*=/ {
      if (match($0, /"[^"]+"/)) {
        print substr($0, RSTART+1, RLENGTH-2)
        exit
      }
    }
  ' Cargo.toml
)"
[[ -n "${OLD_VERSION}" ]] || die "could not read current version from Cargo.toml"

if [[ "${OLD_VERSION}" == "${VERSION}" ]]; then
  die "version is already ${VERSION}"
fi

if git rev-parse "${TAG}" >/dev/null 2>&1; then
  die "tag ${TAG} already exists locally"
fi

if git ls-remote --tags origin "refs/tags/${TAG}" 2>/dev/null | grep -q .; then
  die "tag ${TAG} already exists on origin"
fi

echo "Release plan"
echo "  current : ${OLD_VERSION}"
echo "  new     : ${VERSION}"
echo "  tag     : ${TAG}"
echo "  dry-run : ${DRY_RUN}"
echo "  push    : $(( NO_PUSH == 0 && DRY_RUN == 0 ))"

set_cargo_toml_version() {
  local file="Cargo.toml"
  echo "  update ${file}  (package version → ${VERSION})"
  if [[ "${DRY_RUN}" -eq 0 ]]; then
    local tmp
    tmp="$(mktemp)"
    awk -v ver="${VERSION}" '
      /^\[package\]/ { in_pkg=1; print; next }
      /^\[/ { in_pkg=0 }
      in_pkg && /^version[[:space:]]*=/ {
        print "version = \"" ver "\""
        next
      }
      { print }
    ' "${file}" >"${tmp}"
    mv "${tmp}" "${file}"
  fi
}

set_package_json_version() {
  local file="package.json"
  echo "  update ${file}  (version → ${VERSION})"
  if [[ "${DRY_RUN}" -eq 0 ]]; then
    local tmp
    tmp="$(mktemp)"
    awk -v ver="${VERSION}" '
      BEGIN { done=0 }
      !done && /"version"[[:space:]]*:/ {
        sub(/"version"[[:space:]]*:[[:space:]]*"[^"]*"/, "\"version\": \"" ver "\"")
        done=1
      }
      { print }
    ' "${file}" >"${tmp}"
    mv "${tmp}" "${file}"
  fi
}

set_pkgbuild_version() {
  local file="packaging/arch/PKGBUILD"
  echo "  update ${file}  (pkgver → ${VERSION})"
  if [[ "${DRY_RUN}" -eq 0 ]]; then
    local tmp
    tmp="$(mktemp)"
    awk -v ver="${VERSION}" '
      /^pkgver=/ { print "pkgver=" ver; next }
      { print }
    ' "${file}" >"${tmp}"
    mv "${tmp}" "${file}"
  fi
}

set_cargo_lock_version() {
  local file="Cargo.lock"
  [[ -f "${file}" ]] || return 0
  echo "  update ${file}  (vectrace package → ${VERSION})"
  if [[ "${DRY_RUN}" -eq 0 ]]; then
    local tmp
    tmp="$(mktemp)"
    awk -v ver="${VERSION}" '
      /^name = "vectrace"$/ { in_vr=1; print; next }
      in_vr && /^version = / {
        print "version = \"" ver "\""
        in_vr=0
        next
      }
      /^name = / { in_vr=0 }
      { print }
    ' "${file}" >"${tmp}"
    mv "${tmp}" "${file}"
  fi
}

# Docs / README install examples may lag behind Cargo.toml — sync by pattern.
sync_install_docs() {
  local file="$1"
  [[ -f "${file}" ]] || return 0
  echo "  sync   ${file}  (install artifact versions → ${VERSION})"
  if [[ "${DRY_RUN}" -eq 0 ]]; then
    VERSION="${VERSION}" FILE="${file}" python3 - <<'PY'
import os, re
from pathlib import Path
path = Path(os.environ["FILE"])
ver = os.environ["VERSION"]
text = path.read_text()
# AppImage / tarball style: Vectrace-v0.1.0-... or vectrace-v0.1.0-...
text2 = re.sub(
    r"(Vectrace|vectrace)-v\d+\.\d+\.\d+(?:[.-][0-9A-Za-z.-]+)?",
    rf"\1-v{ver}",
    text,
)
# deb: vectrace_0.1.0_amd64.deb
text2 = re.sub(
    r"vectrace_\d+\.\d+\.\d+(?:[.-][0-9A-Za-z.-]+)?_amd64\.deb",
    f"vectrace_{ver}_amd64.deb",
    text2,
)
# rpm: vectrace-0.1.0-1.x86_64.rpm
text2 = re.sub(
    r"vectrace-\d+\.\d+\.\d+(?:[.-][0-9A-Za-z.-]+)?-1\.x86_64\.rpm",
    f"vectrace-{ver}-1.x86_64.rpm",
    text2,
)
if text2 != text:
    path.write_text(text2)
PY
  fi
}

echo "Updating version references..."
set_cargo_toml_version
set_cargo_lock_version
set_package_json_version
set_pkgbuild_version
sync_install_docs "README.md"
sync_install_docs "docs/guide/installation.md"

if [[ "${DRY_RUN}" -eq 1 ]]; then
  echo "Dry run complete — no commit/tag/push."
  exit 0
fi

# Ensure lockfile stays consistent with Cargo.toml when cargo is available.
if command -v cargo >/dev/null 2>&1; then
  cargo metadata --format-version 1 --no-deps >/dev/null
fi

git add \
  Cargo.toml \
  Cargo.lock \
  package.json \
  packaging/arch/PKGBUILD \
  README.md \
  docs/guide/installation.md

if git diff --cached --quiet; then
  die "no staged changes after version bump"
fi

git commit -m "$(cat <<EOF
chore: release ${TAG}

Bump package version from ${OLD_VERSION} to ${VERSION}.
EOF
)"

git tag -a "${TAG}" -m "Vectrace ${TAG}"

echo "Created commit and tag ${TAG}"

if [[ "${NO_PUSH}" -eq 1 ]]; then
  echo "Skipping push (--no-push)."
  exit 0
fi

git push origin HEAD
git push origin "${TAG}"

echo "Pushed HEAD and ${TAG} to origin."
echo "CI should build release artifacts for tag ${TAG}."

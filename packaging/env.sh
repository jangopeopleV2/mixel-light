# shellcheck shell=bash
# Shared setup for the packaging scripts. Source it: `. "$(dirname "$0")/../env.sh"`.
#
# Exports:
#   ROOT                    workspace root
#   VERSION                 [workspace.package] version from Cargo.toml (override: LIGHTCRAFT_VERSION)
#   DIST                    output directory for release artifacts (default: $ROOT/dist/release)
#   LIGHTCRAFT_BUILD_SHA    git commit baked into the binaries (see crates/engine/src/build_info.rs)
#   LIGHTCRAFT_BUILD_DATE   UTC build date, YYYY-MM-DD
#   CARGO_TARGET_DIR        cargo's target dir (default: $ROOT/target)

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export ROOT

# The version lives in exactly one place: `[workspace.package] version` in the root Cargo.toml.
# (`cargo xtask version` prints the same thing; awk avoids compiling xtask here.)
workspace_version() {
  awk '
    /^\[/ { in_pkg = ($0 == "[workspace.package]") ; next }
    in_pkg && $1 == "version" { gsub(/[" ]/, "", $3); print $3; exit }
  ' "$ROOT/Cargo.toml"
}

VERSION="${LIGHTCRAFT_VERSION:-$(workspace_version)}"
if [ -z "$VERSION" ]; then
  echo "error: could not read [workspace.package] version from $ROOT/Cargo.toml" >&2
  exit 1
fi
export VERSION

DIST="${DIST:-$ROOT/dist/release}"
mkdir -p "$DIST"
export DIST

if [ -z "${LIGHTCRAFT_BUILD_SHA:-}" ]; then
  LIGHTCRAFT_BUILD_SHA="$(git -C "$ROOT" rev-parse HEAD 2>/dev/null || true)"
fi
export LIGHTCRAFT_BUILD_SHA
export LIGHTCRAFT_BUILD_DATE="${LIGHTCRAFT_BUILD_DATE:-$(date -u +%Y-%m-%d)}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"

# Emit a GitHub Actions warning (plain stderr outside Actions).
warn() {
  if [ -n "${GITHUB_ACTIONS:-}" ]; then echo "::warning::$*"; else echo "warning: $*" >&2; fi
}

# Copy licence and readme files, including the selected CJK font licences.
copy_docs() {
  local dest="$1" f
  # Full frozen-lock notices; keep application-relative links within Licenses.
  if [ ! -f "$ROOT/NOTICE" ]; then
    echo "error: missing legal notice: $ROOT/NOTICE" >&2
    return 1
  fi
  mkdir -p "$dest/Licenses/." || return 1
  install -m 0644 "$ROOT/NOTICE" "$dest/Licenses/NOTICE" || return 1
  if [ ! -f "$ROOT/packaging/licenses/epaint_default_fonts-0.36.2/Hack-Regular.txt" ]; then
    echo "error: missing legal notice: $ROOT/packaging/licenses/epaint_default_fonts-0.36.2/Hack-Regular.txt" >&2
    return 1
  fi
  mkdir -p "$dest/Licenses/epaint_default_fonts-0.36.2" || return 1
  install -m 0644 "$ROOT/packaging/licenses/epaint_default_fonts-0.36.2/Hack-Regular.txt" "$dest/Licenses/epaint_default_fonts-0.36.2/Hack-Regular.txt" || return 1
  if [ ! -f "$ROOT/packaging/licenses/epaint_default_fonts-0.36.2/OFL.txt" ]; then
    echo "error: missing legal notice: $ROOT/packaging/licenses/epaint_default_fonts-0.36.2/OFL.txt" >&2
    return 1
  fi
  mkdir -p "$dest/Licenses/epaint_default_fonts-0.36.2" || return 1
  install -m 0644 "$ROOT/packaging/licenses/epaint_default_fonts-0.36.2/OFL.txt" "$dest/Licenses/epaint_default_fonts-0.36.2/OFL.txt" || return 1
  if [ ! -f "$ROOT/packaging/licenses/epaint_default_fonts-0.36.2/UFL.txt" ]; then
    echo "error: missing legal notice: $ROOT/packaging/licenses/epaint_default_fonts-0.36.2/UFL.txt" >&2
    return 1
  fi
  mkdir -p "$dest/Licenses/epaint_default_fonts-0.36.2" || return 1
  install -m 0644 "$ROOT/packaging/licenses/epaint_default_fonts-0.36.2/UFL.txt" "$dest/Licenses/epaint_default_fonts-0.36.2/UFL.txt" || return 1
  if [ ! -f "$ROOT/packaging/licenses/epaint_default_fonts-0.36.2/emoji-icon-font-mit-license.txt" ]; then
    echo "error: missing legal notice: $ROOT/packaging/licenses/epaint_default_fonts-0.36.2/emoji-icon-font-mit-license.txt" >&2
    return 1
  fi
  mkdir -p "$dest/Licenses/epaint_default_fonts-0.36.2" || return 1
  install -m 0644 "$ROOT/packaging/licenses/epaint_default_fonts-0.36.2/emoji-icon-font-mit-license.txt" "$dest/Licenses/epaint_default_fonts-0.36.2/emoji-icon-font-mit-license.txt" || return 1
  for f in README.md LICENSE LICENSE-MIT LICENSE-APACHE COPYRIGHT NOTICE; do
    if [ -f "$ROOT/$f" ]; then cp "$ROOT/$f" "$dest/"; fi
  done
  copy_font_licences "$dest"
}

# External CRAFT_FONTS_DIR overrides the pinned default UI subset. Both choices
# ship their complete OFL licences beside the binaries as OFL-<family-dir>.txt.
copy_font_licences() {
  local dest="$1" lic family fonts_dir="${CRAFT_FONTS_DIR:-$ROOT/mixel/vendor/craft-fonts}"
  for lic in "$fonts_dir"/fonts/*/OFL.txt; do
    [ -f "$lic" ] || continue
    family="$(basename "$(dirname "$lic")")"
    cp "$lic" "$dest/OFL-$family.txt"
  done
}

# Portable SHA-256 of a file (prints just the hash).
sha256() {
  if command -v sha256sum >/dev/null; then sha256sum "$1" | cut -d' ' -f1; else shasum -a 256 "$1" | cut -d' ' -f1; fi
}

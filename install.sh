#!/bin/sh
# Install the asana CLI and its agent skill from a GitHub release.
#
#   curl -fsSL https://raw.githubusercontent.com/nazriel/asana-cli/main/install.sh | sh
#   curl -fsSL https://raw.githubusercontent.com/nazriel/asana-cli/main/install.sh | sh -s -- --version v0.6.0
#
# Options (or the matching environment variables):
#   --version TAG      release to install (ASANA_VERSION, default: latest)
#   --bin-dir DIR      where the binary goes (ASANA_BIN_DIR, default: ~/.local/bin)
#   --skills-dir DIR   where the skill goes (ASANA_SKILLS_DIR, default: ~/.agents/skills)
#   --no-bin           install only the skill
#   --no-skill         install only the binary
#
# The skill is linked into ~/.claude/skills when ~/.claude exists. An existing
# skill that is a symlink (for example to a development checkout) is left alone.
set -eu

repo="nazriel/asana-cli"
version="${ASANA_VERSION:-latest}"
bin_dir="${ASANA_BIN_DIR:-$HOME/.local/bin}"
skills_dir="${ASANA_SKILLS_DIR:-$HOME/.agents/skills}"
install_bin=1
install_skill=1

say() { printf 'asana-install: %s\n' "$*" >&2; }
die() { say "error: $*"; exit 1; }
usage() {
  cat <<'USAGE'
Usage: install.sh [--version TAG] [--bin-dir DIR] [--skills-dir DIR] [--no-bin] [--no-skill]
USAGE
}

while [ $# -gt 0 ]; do
  case "$1" in
    --version) [ $# -ge 2 ] || die "--version needs a value"; version="$2"; shift 2 ;;
    --bin-dir) [ $# -ge 2 ] || die "--bin-dir needs a value"; bin_dir="$2"; shift 2 ;;
    --skills-dir) [ $# -ge 2 ] || die "--skills-dir needs a value"; skills_dir="$2"; shift 2 ;;
    --no-bin) install_bin=0; shift ;;
    --no-skill) install_skill=0; shift ;;
    -h | --help) usage; exit 0 ;;
    *) die "unknown option: $1" ;;
  esac
done

need() { command -v "$1" >/dev/null 2>&1 || die "$1 is required"; }
need curl
need tar
need uname

case "$(uname -s)" in
  Darwin)
    os="apple-darwin"
    arch="$(uname -m)"
    # A shell running under Rosetta reports x86_64 on Apple Silicon.
    if [ "$arch" = "x86_64" ] && [ "$(sysctl -n sysctl.proc_translated 2>/dev/null || echo 0)" = "1" ]; then
      arch="arm64"
    fi
    ;;
  Linux)
    os="unknown-linux-musl"
    arch="$(uname -m)"
    ;;
  *) die "unsupported operating system: $(uname -s)" ;;
esac
case "$arch" in
  x86_64 | amd64) arch="x86_64" ;;
  arm64 | aarch64) arch="aarch64" ;;
  *) die "unsupported architecture: $arch" ;;
esac
target="${arch}-${os}"

if [ "$version" = "latest" ]; then
  url="$(curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/${repo}/releases/latest")" ||
    die "could not resolve the latest release"
  version="${url##*/}"
  case "$version" in v*) ;; *) die "no published release found" ;; esac
fi

archive="asana-${version}-${target}"
base="https://github.com/${repo}/releases/download/${version}"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT INT TERM

say "downloading ${archive}.tar.gz"
curl -fsSL -o "${tmp}/${archive}.tar.gz" "${base}/${archive}.tar.gz" ||
  die "download failed: ${base}/${archive}.tar.gz"
curl -fsSL -o "${tmp}/SHA256SUMS" "${base}/SHA256SUMS" || die "download failed: ${base}/SHA256SUMS"

expected="$(awk -v f="${archive}.tar.gz" '$2 == f || $2 == "*" f { print $1 }' "${tmp}/SHA256SUMS")"
[ -n "$expected" ] || die "${archive}.tar.gz is not listed in SHA256SUMS"
if command -v sha256sum >/dev/null 2>&1; then
  actual="$(sha256sum "${tmp}/${archive}.tar.gz" | awk '{ print $1 }')"
else
  actual="$(shasum -a 256 "${tmp}/${archive}.tar.gz" | awk '{ print $1 }')"
fi
[ "$expected" = "$actual" ] || die "checksum mismatch for ${archive}.tar.gz"

tar -xzf "${tmp}/${archive}.tar.gz" -C "$tmp"
src="${tmp}/${archive}"

if [ "$install_bin" = 1 ]; then
  mkdir -p "$bin_dir"
  cp "${src}/asana" "${bin_dir}/asana.tmp"
  chmod 755 "${bin_dir}/asana.tmp"
  mv -f "${bin_dir}/asana.tmp" "${bin_dir}/asana"
  say "installed ${bin_dir}/asana (${version})"
  case ":${PATH}:" in
    *":${bin_dir}:"*) ;;
    *) say "note: ${bin_dir} is not on PATH; add it in your shell profile" ;;
  esac
fi

if [ "$install_skill" = 1 ]; then
  skill="${skills_dir}/asana-cli"
  if [ -L "$skill" ]; then
    say "skipped skill: ${skill} is a symlink to $(readlink "$skill")"
  else
    mkdir -p "$skills_dir"
    rm -rf "${skill}.tmp"
    cp -R "${src}/skills/asana-cli" "${skill}.tmp"
    rm -rf "$skill"
    mv "${skill}.tmp" "$skill"
    say "installed skill ${skill}"
  fi

  claude_skill="$HOME/.claude/skills/asana-cli"
  if [ -d "$HOME/.claude" ] && [ "$claude_skill" != "$skill" ]; then
    if [ -L "$claude_skill" ]; then
      :
    elif [ -e "$claude_skill" ]; then
      say "skipped Claude Code link: ${claude_skill} already exists"
    else
      mkdir -p "$HOME/.claude/skills"
      ln -s "$skill" "$claude_skill"
      say "linked ${claude_skill} -> ${skill}"
    fi
  fi
fi

if [ "$install_bin" = 1 ]; then
  say "next: run \`asana auth login\`, then \`asana setup\` for shell completion"
fi

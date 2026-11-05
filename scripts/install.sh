#!/bin/sh
# Installs a kt-affected release binary.
#
#   curl -fsSL https://raw.githubusercontent.com/chrisjenx/kt-affected/main/scripts/install.sh | sh
#
# Environment: KT_AFFECTED_VERSION (a tag like v0.4.0; default: the latest release),
# KT_AFFECTED_INSTALL_DIR (default: ~/.local/bin). Verifies the archive against the release's SHA256SUMS.
set -eu

REPO=chrisjenx/kt-affected
DIR=${KT_AFFECTED_INSTALL_DIR:-$HOME/.local/bin}
VERSION=${KT_AFFECTED_VERSION:-latest}

case "$(uname -s)" in
  Linux) os=unknown-linux-gnu ;;
  Darwin) os=apple-darwin ;;
  *) echo "kt-affected: unsupported OS $(uname -s); download a release manually from https://github.com/$REPO/releases" >&2; exit 1 ;;
esac
case "$(uname -m)" in
  x86_64 | amd64) arch=x86_64 ;;
  arm64 | aarch64) arch=aarch64 ;;
  *) echo "kt-affected: unsupported architecture $(uname -m)" >&2; exit 1 ;;
esac
asset=kt-affected-$arch-$os.tar.gz
if [ "$VERSION" = latest ]; then
  base=https://github.com/$REPO/releases/latest/download
else
  base=https://github.com/$REPO/releases/download/$VERSION
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
curl -fsSL "$base/$asset" -o "$tmp/$asset"
curl -fsSL "$base/SHA256SUMS" -o "$tmp/SHA256SUMS"
expected=$(grep " $asset\$" "$tmp/SHA256SUMS" | cut -d' ' -f1)
if command -v sha256sum > /dev/null; then actual=$(sha256sum "$tmp/$asset" | cut -d' ' -f1); else actual=$(shasum -a 256 "$tmp/$asset" | cut -d' ' -f1); fi
[ -n "$expected" ] && [ "$expected" = "$actual" ] || { echo "kt-affected: checksum mismatch for $asset" >&2; exit 1; }
tar -xzf "$tmp/$asset" -C "$tmp"
mkdir -p "$DIR"
install -m 0755 "$tmp/kt-affected-$arch-$os/kt-affected" "$DIR/kt-affected"
echo "installed $("$DIR/kt-affected" --version) to $DIR/kt-affected"
case ":$PATH:" in *":$DIR:"*) ;; *) echo "add $DIR to your PATH" ;; esac

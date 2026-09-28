#!/bin/bash
# Native, opt-in CI proof for the pinned experimental DNS source pair.
# Fetching exact upstream commits is the caller's job. This script installs,
# enables, enrolls and connects nothing.
set -euo pipefail

[[ $# == 3 ]] || exit 2
source_root=$(git rev-parse --show-toplevel)
mihomo_git=$1
sing_tun_git=$2
output=$3
[[ $mihomo_git == /* && $sing_tun_git == /* && $output == /* ]] || exit 2
[[ ! -e $output && ! -L $output ]] || exit 2
case $(uname -m) in
  x86_64|aarch64) architecture=$(uname -m) ;;
  *) exit 2 ;;
esac

[[ $(git -C "$mihomo_git" rev-parse --verify 'ab405bad5beeeac8b003bb01f60f134f6df54471^{commit}') == ab405bad5beeeac8b003bb01f60f134f6df54471 ]]
[[ $(git -C "$sing_tun_git" rev-parse --verify 'b50ae28a1409c7bce8e96e6c6966cf57d8ace754^{commit}') == b50ae28a1409c7bce8e96e6c6966cf57d8ace754 ]]
go_bin=$(readlink -f "$(command -v go)")
[[ -f $go_bin && ! -L $go_bin ]]

mkdir -m 700 "$output"
scratch=$(mktemp -d "$output/.hydrate.XXXXXXXX")
python3 - "$source_root" "$mihomo_git" "$sing_tun_git" "$scratch" <<'PY'
import sys
from pathlib import Path

root, mihomo, sing_tun, scratch = map(Path, sys.argv[1:])
sys.path.insert(0, str(root / "tests/dns_broker_host/package"))
import build_pair

# Reuse the offline builder's bounded, data-filtered Git archive extraction.
build_pair.export_git(mihomo, build_pair.MIHOMO, scratch / "mihomo")
build_pair.export_git(sing_tun, build_pair.SING_TUN, scratch / "sing-tun")
PY
git -C "$scratch/mihomo" apply "$source_root/tests/core_dns_adapter/mihomo-dns-broker.patch"
git -C "$scratch/sing-tun" apply "$source_root/tests/core_dns_adapter/sing-tun-descriptor.patch"
(
  cd "$scratch/mihomo"
  GOWORK=off "$go_bin" mod edit -replace=github.com/metacubex/sing-tun=../sing-tun
  GOWORK=off GOPROXY=https://proxy.golang.org GOSUMDB=sum.golang.org GOTOOLCHAIN=local "$go_bin" mod download
)
(
  cd "$source_root"
  cargo fetch --locked
)

python3 "$source_root/tests/dns_broker_host/package/build_pair.py" \
  --mihomo-git "$mihomo_git" --sing-tun-git "$sing_tun_git" \
  --go "$go_bin" --arch "$architecture" --output "$output/pair"
python3 "$source_root/tests/dns_broker_host/package/stage.py" \
  --pair "$output/pair" --revision "$(git -C "$source_root" rev-parse HEAD)" \
  --arch "$architecture" --output "$output/staged"
(
  cd "$output/staged"
  PKGDEST="$output/staged" makepkg --noconfirm
)
packages=("$output"/staged/omavless-dns-experimental-*.pkg.tar.*)
[[ ${#packages[@]} == 1 && -f ${packages[0]} ]]
package_arch=$(bsdtar -xOf "${packages[0]}" .PKGINFO | sed -n 's/^arch = //p')
[[ $package_arch == "$architecture" ]]
echo "Native experimental DNS pair built and packaged for $architecture; nothing installed."

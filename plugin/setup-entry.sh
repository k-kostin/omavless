#!/bin/bash
# SPDX-License-Identifier: MIT
set -euo pipefail
setup_entry_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
source "$setup_entry_dir/setup-channel.sh"
case "$(setup_channel)" in
  public) exec /bin/bash "$setup_entry_dir/setup-runtime.sh" "$@" ;;
  offline-candidate) exec /bin/bash "$setup_entry_dir/setup-offline-candidate.sh" "$@" ;;
  *) printf 'Setup channel is unavailable; no operation was submitted.\n' >&2; exit 2 ;;
esac

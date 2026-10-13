#!/usr/bin/env bash
# Executes the AppImage that will be published, not only Cargo's binaries.
#
# Usage: scripts/smoke-release-artifact.sh <RO-Launcher_X.Y.Z_amd64.AppImage> <evidence-file>
#
# 1. The AppImage extracts and ships executable ro-launcher, ro-inputd and ro-sessiond.
# 2. ro-launcher starts through the bundle's own AppRun environment (bundled libraries and hooks),
#    which proves the dynamic loader resolves it, and reaches main's argument validation without a
#    display or touching user data.
# 3. ro-inputd starts and speaks its JSON protocol. An invalid trigger makes it stop at argument
#    validation: without triggers it defaults to F1 and would grab the host's real keyboards.
# 4. The extracted ro-sessiond passes the orphan_reparent integration suite (subreaper, Yama=1
#    memory access, batching, shutdown grace, 100 cycles without zombies) via
#    RO_SESSIOND_TEST_BINARY. The suite fails unless the host is at ptrace_scope=1.
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "usage: $0 <AppImage> <evidence-file>" >&2
  exit 2
fi
appimage=$(realpath -e "$1")
evidence=$(realpath -m "$2")
repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

if [[ -n "${RO_YAMA_ALLOW_UNVALIDATED:-}" ]]; then
  echo "RO_YAMA_ALLOW_UNVALIDATED must not be set for the release smoke test" >&2
  exit 1
fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
(cd "$work" && "$appimage" --appimage-extract >/dev/null)
root="$work/squashfs-root"
bin="$root/usr/bin"
for name in ro-launcher ro-inputd ro-sessiond; do
  if [[ ! -x "$bin/$name" ]]; then
    echo "missing executable usr/bin/$name in $appimage" >&2
    exit 1
  fi
done

echo "== ro-launcher through AppRun"
set +e
launcher_out=$(env -u DISPLAY -u WAYLAND_DISPLAY timeout 30 "$root/AppRun" --import-runtime relative.tar.zst 2>&1)
launcher_status=$?
set -e
if [[ $launcher_status -ne 2 || $launcher_out != *"La importación requiere una ruta absoluta"* ]]; then
  echo "ro-launcher smoke failed (status $launcher_status): $launcher_out" >&2
  exit 1
fi

echo "== ro-inputd JSON protocol"
inputd_out=$(timeout 10 "$bin/ro-inputd" --json --triggers not-a-key </dev/null)
node -e '
  const message = JSON.parse(process.argv[1])
  if (message.type !== "fatal" || !message.message.includes("Ninguna tecla trigger")) {
    console.error(`unexpected ro-inputd message: ${process.argv[1]}`)
    process.exit(1)
  }
' "$inputd_out"

echo "== ro-sessiond orphan_reparent suite against the extracted sidecar"
sessiond_log="$work/orphan_reparent.log"
(
  cd "$repo"
  RO_SESSIOND_TEST_BINARY="$bin/ro-sessiond" \
    cargo test -p ro-sessiond --all-features --test orphan_reparent
) 2>&1 | tee "$sessiond_log"
if ! grep -qF "orphan_reparent integration tests passed (sidecar: $bin/ro-sessiond)" "$sessiond_log"; then
  echo "orphan_reparent did not report a pass against the extracted sidecar" >&2
  exit 1
fi

{
  echo "appimage=$(basename "$appimage")"
  echo "ptrace_scope=$(cat /proc/sys/kernel/yama/ptrace_scope)"
  (cd "$(dirname "$appimage")" && sha256sum "$(basename "$appimage")")
  (cd "$root" && sha256sum usr/bin/ro-launcher usr/bin/ro-inputd usr/bin/ro-sessiond)
  (cd "$root" && ls -la usr/bin/ro-launcher usr/bin/ro-inputd usr/bin/ro-sessiond)
  echo "ro-launcher AppRun argument validation: exit $launcher_status"
  echo "ro-inputd --json --triggers not-a-key: $inputd_out"
  echo "ro-sessiond: $(grep -F 'orphan_reparent integration tests passed' "$sessiond_log")"
} >"$evidence"
echo "smoke-release-artifact: ok"

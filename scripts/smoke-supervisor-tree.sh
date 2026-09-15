#!/usr/bin/env bash
# Asistencia smoke fase 4: árbol launcher → ro-sessiond → wineserver (prefix RO-Launcher).
set -euo pipefail

echo "=== Yama ==="
sysctl kernel.yama.ptrace_scope 2>/dev/null || true
echo

echo "=== RO-Launcher / sidecar ==="
pgrep -af 'target/debug/ro-launcher|ro-launcher_af7e4e6ea34f|ro-sessiond --prefix' 2>/dev/null || echo "(ninguno)"
echo

echo "=== wineserver (WINEPREFIX ~/.local/share/ro-launcher) ==="
mapfile -t ws_pids < <(pgrep -x wineserver 2>/dev/null || true)
if ((${#ws_pids[@]} == 0)); then
  echo "No hay wineserver en ejecución."
  exit 0
fi

launcher_pids=()
while read -r pid; do
  [[ -n "$pid" ]] && launcher_pids+=("$pid")
done < <(pgrep -x ro-launcher 2>/dev/null || true)

sessiond_pids=()
while read -r line; do
  [[ -n "$line" ]] || continue
  pid="${line%% *}"
  sessiond_pids+=("$pid")
done < <(pgrep -af '^[^ ]*ro-sessiond ' 2>/dev/null | awk '{print $1}' || true)

for pid in "${ws_pids[@]}"; do
  if [[ ! -r "/proc/$pid/environ" ]]; then
    continue
  fi
  prefix=$(
    tr '\0' '\n' <"/proc/$pid/environ" 2>/dev/null | sed -n 's/^WINEPREFIX=//p' | head -1
  )
  [[ "$prefix" == *ro-launcher* ]] || continue
  ppid=$(awk '/^PPid:/ {print $2}' "/proc/$pid/status")
  ppid_comm=$(ps -o comm= -p "$ppid" 2>/dev/null || echo "?")
  echo "wineserver pid=$pid WINEPREFIX=${prefix:-?} PPID=$ppid ($ppid_comm)"
  if printf '%s\n' "${sessiond_pids[@]}" | grep -qx "$ppid"; then
    echo "  OK: PPID es ro-sessiond (supervisado)."
  elif [[ "$ppid_comm" == "systemd" || "$ppid_comm" == "systemd-user" ]]; then
    echo "  FALLO: PPID parece systemd (fuera del árbol del supervisor)."
  elif printf '%s\n' "${launcher_pids[@]}" | grep -qx "$ppid"; then
    echo "  AVISO: PPID es ro-launcher directo (sin sidecar intermedio)."
  else
    echo "  REVISAR: traza de ancestros:"
    p="$pid"
    for _ in $(seq 1 8); do
      comm=$(ps -o comm= -p "$p" 2>/dev/null || break)
      echo "    $p $comm"
      [[ "$comm" == "ro-launcher" || "$comm" == "ro-sessiond" ]] && break
      p=$(awk '/^PPid:/ {print $2}' "/proc/$p/status" 2>/dev/null || echo 0)
      [[ "$p" == "0" || "$p" == "1" ]] && break
    done
  fi
done

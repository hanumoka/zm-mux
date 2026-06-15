#!/usr/bin/env bash
# Clone reference repositories for zm-mux research (read-only study material).
#
# reference/ is gitignored — NOT part of the zm-mux repo. The tracked inventory
# (URL / license / reuse-class / pinned SHA) lives in
# docs/research/05-reference-inventory.md.
#
# Reuse policy (clean-room): MIT/Apache repos are SAFE (learn + reuse). GPL/AGPL
# repos are STUDY-ONLY — read for understanding, never copy code/text into zm-mux.
#
# Usage:  bash scripts/clone-references.sh
set -u

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REF="$ROOT/reference"
mkdir -p "$REF"
DEPTH="--depth 1"

# name|url   (name = directory under reference/)
repos=(
  "cmux|https://github.com/manaflow-ai/cmux"                 # STUDY  (GPL-3.0)  reproduction target
  "wezterm|https://github.com/wezterm/wezterm"               # SAFE   (MIT)      term/gui/mux split, portable-pty origin
  "zellij|https://github.com/zellij-org/zellij"              # SAFE   (MIT)      Windows-native multiplexer (v0.44+)
  "alacritty|https://github.com/alacritty/alacritty"         # SAFE   (Apache/MIT) VT emulation, ConPTY
  "vte|https://github.com/alacritty/vte"                     # SAFE   (Apache/MIT) ANSI/VT parser
  "cosmic-term|https://github.com/pop-os/cosmic-term"        # STUDY  (GPL-3.0)  alacritty_terminal+glyphon+wgpu integration
  "psmux|https://github.com/psmux/psmux"                     # ?      verify     Windows-native tmux in Rust
  "wmux-amirlehmam|https://github.com/amirlehmam/wmux"       # STUDY  (AGPL?)    Windows port candidate A
  "wmux-openwong2kim|https://github.com/openwong2kim/wmux"   # ?      verify     Windows port candidate B (reported)
  "cmux-for-linux|https://github.com/cai0baa/cmux-for-linux" # STUDY  (GPL)      Tauri cross-platform port (ptrcode)
  "cmux-linux|https://github.com/bradwilson331/cmux-linux"   # STUDY  (GPL?)     GTK4 Linux port
)

echo "=== zm-mux reference clone (shallow) ==="
for entry in "${repos[@]}"; do
  name="${entry%%|*}"; rest="${entry#*|}"; url="${rest%%[[:space:]]*}"
  dest="$REF/$name"
  if [ -d "$dest/.git" ]; then echo "SKIP  $name (exists)"; continue; fi
  echo "CLONE $name  <-  $url"
  if git clone $DEPTH "$url" "$dest" >/dev/null 2>&1; then
    echo "  OK   $name @ $(git -C "$dest" rev-parse HEAD)"
  else
    echo "  FAIL $name  ($url)  -- record as non-existent / private in inventory"
  fi
done

# microsoft/terminal is huge: sparse partial clone, only ConPTY sample + docs.
mtdest="$REF/microsoft-terminal"
if [ ! -d "$mtdest/.git" ]; then
  echo "CLONE microsoft-terminal (sparse: samples + doc)  <-  https://github.com/microsoft/terminal"
  if git clone --depth 1 --filter=blob:none --sparse https://github.com/microsoft/terminal "$mtdest" >/dev/null 2>&1; then
    git -C "$mtdest" sparse-checkout set samples doc >/dev/null 2>&1
    echo "  OK   microsoft-terminal @ $(git -C "$mtdest" rev-parse HEAD) (sparse)"
  else
    echo "  FAIL microsoft-terminal"
  fi
fi

echo ""
echo "=== summary: repo / short-sha / license-file ==="
for d in "$REF"/*/; do
  [ -d "$d/.git" ] || continue
  n="$(basename "$d")"
  lic="$(ls "$d" 2>/dev/null | grep -iE '^(LICENSE|COPYING|LICENCE)' | head -1)"
  printf "  %-20s sha=%-10s license_file=%s\n" "$n" "$(git -C "$d" rev-parse --short HEAD 2>/dev/null)" "${lic:-NONE}"
done
echo "=== done ==="

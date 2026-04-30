#!/bin/bash
# PostCompact — 컴팩션 후 컨텍스트 복원

ROOT_DIR="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
RECOVERY_FILE="$ROOT_DIR/.project-memory/pre-compact-recovery.md"

if [ -f "$RECOVERY_FILE" ]; then
  echo "[PostCompact] Restoring context from recovery file..."
  echo ""
  cat "$RECOVERY_FILE"
else
  echo "[PostCompact] No recovery file found."
fi

exit 0

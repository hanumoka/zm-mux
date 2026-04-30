#!/bin/bash
# PreCompact — 컴팩션 전 컨텍스트 스냅샷 저장

ROOT_DIR="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
CONTEXT_FILE="$ROOT_DIR/.project-memory/context.md"
RECOVERY_FILE="$ROOT_DIR/.project-memory/pre-compact-recovery.md"

{
  echo "# Pre-Compact Recovery"
  echo "Saved: $(date '+%Y-%m-%d %H:%M:%S')"
  echo ""

  # context.md 전체 복사
  if [ -f "$CONTEXT_FILE" ]; then
    echo "## Context Snapshot"
    cat "$CONTEXT_FILE"
    echo ""
  fi

  # Git 상태
  if git rev-parse --git-dir > /dev/null 2>&1; then
    echo "## Git Status"
    echo '```'
    git status --short 2>/dev/null
    echo '```'
    echo ""
    echo "## Recent Commits"
    echo '```'
    git log --oneline -5 2>/dev/null
    echo '```'
  fi
} > "$RECOVERY_FILE" 2>/dev/null

echo "[PreCompact] Recovery snapshot saved to $RECOVERY_FILE"
exit 0

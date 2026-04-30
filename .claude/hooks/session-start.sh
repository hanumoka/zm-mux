#!/bin/bash
# SessionStart — 세션 시작 시 컨텍스트 주입

ROOT_DIR="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
CONTEXT_FILE="$ROOT_DIR/.project-memory/context.md"
RECOVERY_FILE="$ROOT_DIR/.project-memory/pre-compact-recovery.md"

echo "=== zm-mux Session Start ==="

# Git 상태
if git rev-parse --git-dir > /dev/null 2>&1; then
  BRANCH=$(git branch --show-current 2>/dev/null || echo "N/A")
  echo "[Branch] $BRANCH"
  echo ""
  echo "[Recent Commits]"
  git log --oneline -3 2>/dev/null || echo "  (no commits)"
  echo ""
fi

# context.md 표시
if [ -f "$CONTEXT_FILE" ]; then
  echo "[Context]"

  # Focus
  FOCUS=$(sed -n '/^## Focus/,/^##/p' "$CONTEXT_FILE" | head -8 | grep -v '^##')
  if [ -n "$FOCUS" ]; then
    echo "  Focus:"
    echo "$FOCUS" | sed 's/^/    /'
  fi

  # TODOs
  TODOS=$(sed -n '/^## TODOs/,/^##/p' "$CONTEXT_FILE" | head -20 | grep -v '^##')
  if [ -n "$TODOS" ]; then
    echo "  TODOs:"
    echo "$TODOS" | sed 's/^/    /'
  fi

  # Blockers
  BLOCKERS=$(sed -n '/^## Blockers/,/^##/p' "$CONTEXT_FILE" | head -10 | grep -v '^##')
  if [ -n "$BLOCKERS" ]; then
    echo "  Blockers:"
    echo "$BLOCKERS" | sed 's/^/    /'
  fi
else
  echo "[Context] No context.md found. Run /zm-memory-save to initialize."
fi

# Pre-compact recovery 체크
if [ -f "$RECOVERY_FILE" ]; then
  echo ""
  echo "[!] Pre-compact recovery file exists — context was preserved from previous compaction."
fi

# Known mistakes 카운트
MISTAKES_FILE="$ROOT_DIR/.claude/rules/known-mistakes.md"
if [ -f "$MISTAKES_FILE" ]; then
  COUNT=$(grep -c '^### M-' "$MISTAKES_FILE" 2>/dev/null || echo "0")
  echo ""
  echo "[Mistakes Registry] $COUNT patterns loaded"
fi

echo "==========================="

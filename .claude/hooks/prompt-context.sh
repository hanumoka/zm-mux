#!/bin/bash
# UserPromptSubmit — 키워드 기반 컨텍스트 자동 주입

ROOT_DIR="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
INPUT=$(cat)
PROMPT=$(echo "$INPUT" | PYTHONUTF8=1 python -c "
import sys, json
try:
    data = json.load(sys.stdin)
    print(data.get('prompt', ''))
except:
    print(sys.stdin.read())
" 2>/dev/null)

if [ -z "$PROMPT" ]; then
  exit 0
fi

# 아키텍처/설계 키워드
if echo "$PROMPT" | grep -qiE '(아키텍처|architecture|설계|design|구조|structure)'; then
  echo "[DESIGN_CONTEXT] docs/ 리서치 문서와 CLAUDE.md 설계 목표를 참조하세요."
  if [ -f "$ROOT_DIR/docs/02-cmux-overview.md" ]; then
    echo "  → cmux 아키텍처: docs/02-cmux-overview.md"
  fi
fi

# 버그/이슈 키워드
if echo "$PROMPT" | grep -qiE '(버그|bug|오류|error|이슈|issue|문제|problem)'; then
  if [ -f "$ROOT_DIR/.claude/rules/known-mistakes.md" ]; then
    COUNT=$(grep -c '^### M-' "$ROOT_DIR/.claude/rules/known-mistakes.md" 2>/dev/null || echo "0")
    echo "[MISTAKE_CONTEXT] known-mistakes.md에 $COUNT 개의 패턴이 등록되어 있습니다."
  fi
fi

# 작업 완료 키워드
if echo "$PROMPT" | grep -qiE '(완료|done|finish|끝|마무리|commit)'; then
  echo "[WORK_COMPLETION] /zm-memory-save 로 세션 컨텍스트를 저장하세요."
fi

# 조사/리서치 키워드
if echo "$PROMPT" | grep -qiE '(조사|research|분석|analysis|비교|compare)'; then
  echo "[RESEARCH_CONTEXT] 기존 리서치: docs/ 디렉토리 참조"
  echo "  01: Windows Claude Code 이슈 | 02: cmux 분석 | 03: tmux vs cmux"
  echo "  04: 기존 wmux 프로젝트 | 05: Windows 터미널 비교"
fi

exit 0

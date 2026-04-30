#!/bin/bash
# PostToolUse (Write|Edit) — 변경 후 품질 경고

ROOT_DIR="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
INPUT=$(cat)

FILE_PATH=$(echo "$INPUT" | PYTHONUTF8=1 python -c "
import sys, json
try:
    data = json.load(sys.stdin)
    print(data.get('file_path', '') or data.get('filePath', '') or '')
except:
    pass
" 2>/dev/null)

if [ -z "$FILE_PATH" ]; then
  exit 0
fi

WARNINGS=""

# 문서 변경 시 알림
if echo "$FILE_PATH" | grep -qE '\.(md|mdx)$'; then
  if echo "$FILE_PATH" | grep -q 'docs/'; then
    WARNINGS="${WARNINGS}[INFO] 문서 변경 감지: 관련 문서 간 일관성을 확인하세요.\n"
  fi
fi

# 시크릿 패턴 스캔 (모든 파일)
if [ -f "$FILE_PATH" ]; then
  if grep -qiE '(password|secret|api_key|private_key)\s*[:=]\s*["\x27][^"\x27]{8,}' "$FILE_PATH" 2>/dev/null; then
    WARNINGS="${WARNINGS}[WARN] 시크릿 패턴 감지: $FILE_PATH 에서 하드코딩된 시크릿이 있을 수 있습니다.\n"
  fi
  if grep -qiE '(sk-[a-zA-Z0-9]{20,}|ghp_[a-zA-Z0-9]{30,}|AKIA[0-9A-Z]{16})' "$FILE_PATH" 2>/dev/null; then
    WARNINGS="${WARNINGS}[WARN] API 키/토큰 패턴 감지: $FILE_PATH\n"
  fi
fi

# TypeScript/Rust 변경 시 빌드 확인 제안
if echo "$FILE_PATH" | grep -qE '\.(ts|tsx|rs)$'; then
  WARNINGS="${WARNINGS}[INFO] 코드 변경 감지: 빌드 검증을 권장합니다.\n"
fi

if [ -n "$WARNINGS" ]; then
  echo -e "$WARNINGS"
fi

exit 0

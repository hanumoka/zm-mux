#!/bin/bash
# PostToolUseFailure hook — Bash 실패 시 로깅 + 컨텍스트 주입

INPUT=$(cat)

TOOL_NAME=$(PYTHONUTF8=1 python -c "
import sys, json
try:
    data = json.loads(sys.stdin.read())
    print(data.get('tool_name', ''))
except:
    print('')
" <<< "$INPUT" 2>/dev/null || echo "")

# Bash 실패만 처리
if [ "$TOOL_NAME" != "Bash" ]; then
  exit 0
fi

# 명령어와 에러 추출
FAILURE_INFO=$(PYTHONUTF8=1 python -c "
import sys, json, os
try:
    data = json.loads(sys.stdin.read())
    cmd = data.get('tool_input', {}).get('command', 'unknown')
    err = data.get('tool_result', {}).get('stderr', data.get('tool_result', {}).get('error', 'unknown'))
    if isinstance(err, str) and len(err) > 200:
        err = err[:200] + '...'
    print(f'CMD: {cmd}')
    print(f'ERR: {err}')
except Exception as e:
    print(f'PARSE_ERROR: {e}')
" <<< "$INPUT" 2>/dev/null || echo "PARSE_ERROR")

# 로그 디렉토리 확인 및 기록
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LOG_DIR="$SCRIPT_DIR/../logs"
LOG_FILE="$LOG_DIR/failures.log"

if [ -d "$LOG_DIR" ]; then
  TIMESTAMP=$(date '+%Y-%m-%d %H:%M:%S')
  echo "[$TIMESTAMP] $FAILURE_INFO" >> "$LOG_FILE" 2>/dev/null
fi

# stdout으로 간단한 요약 출력 (LLM 컨텍스트 주입)
echo "[FAILURE] Bash 명령 실패 — .claude/logs/failures.log 참조"

exit 0

#!/bin/bash
# PreToolUse — known-mistakes.md [BLOCK] 패턴 동적 차단

ROOT_DIR="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
MISTAKES_FILE="$ROOT_DIR/.claude/rules/known-mistakes.md"
INPUT=$(cat)

# known-mistakes.md 없으면 통과
if [ ! -f "$MISTAKES_FILE" ]; then
  exit 0
fi

# 도구 입력에서 command/content 추출
TOOL_INPUT=$(echo "$INPUT" | PYTHONUTF8=1 python -c "
import sys, json
try:
    data = json.load(sys.stdin)
    content = data.get('command', '') or data.get('content', '') or data.get('new_string', '') or ''
    print(content)
except:
    pass
" 2>/dev/null || echo "$INPUT")

if [ -z "$TOOL_INPUT" ]; then
  exit 0
fi

# 기본 안전 검사: force push, reset --hard
if echo "$TOOL_INPUT" | grep -qiE 'git\s+push\s+(-f|--force)'; then
  echo "[BLOCK] Force push 감지. 이 작업은 금지되어 있습니다."
  exit 2
fi

if echo "$TOOL_INPUT" | grep -qiE 'git\s+reset\s+--hard'; then
  echo "[BLOCK] git reset --hard 감지. 이 작업은 금지되어 있습니다."
  exit 2
fi

# 시크릿 패턴 검사
if echo "$TOOL_INPUT" | grep -qiE '(PRIVATE.KEY|sk-[a-zA-Z0-9]{20,}|ghp_[a-zA-Z0-9]{30,}|aws_secret)'; then
  echo "[BLOCK] 시크릿/키 패턴 감지. 코드에 시크릿을 포함하지 마세요."
  exit 2
fi

# known-mistakes.md에서 [BLOCK] 패턴 동적 로드 (환경변수로 안전하게 전달)
HOOK_MISTAKES_FILE="$MISTAKES_FILE" HOOK_TOOL_INPUT="$TOOL_INPUT" PYTHONUTF8=1 python -c "
import re, sys, os

mistakes_file = os.environ.get('HOOK_MISTAKES_FILE', '')
tool_input = os.environ.get('HOOK_TOOL_INPUT', '')

if not mistakes_file or not tool_input:
    sys.exit(0)

try:
    with open(mistakes_file, 'r', encoding='utf-8') as f:
        content = f.read()

    blocks = re.findall(r'### (M-\d+).*?\[BLOCK\].*?\n(.*?)(?=\n### M-|\Z)', content, re.DOTALL)

    for mid, body in blocks:
        detection = re.search(r'\*\*탐지\*\*:\s*\`(.+?)\`', body)
        if detection:
            pattern = detection.group(1)
            try:
                if re.search(pattern, tool_input, re.IGNORECASE):
                    desc_match = re.search(r'\*\*실수\*\*:\s*(.+)', body)
                    desc = desc_match.group(1) if desc_match else 'Blocked pattern detected'
                    print(f'[BLOCK] {mid}: {desc}')
                    sys.exit(2)
            except re.error:
                if pattern.lower() in tool_input.lower():
                    print(f'[BLOCK] {mid}: Pattern matched (literal)')
                    sys.exit(2)
except Exception as e:
    pass

sys.exit(0)
" 2>/dev/null

exit $?

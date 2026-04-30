---
name: zm-troubleshoot
description: 트러블슈팅 패턴 기록 및 조회
user-invocable: true
disable-model-invocation: false
argument-hint: "[add|TS-NNN]"
---

# zm-troubleshoot

트러블슈팅 패턴을 관리합니다.

## 사용법

- `/zm-troubleshoot` — 등록된 패턴 목록
- `/zm-troubleshoot add` — 새 패턴 등록

## 패턴 형식 (TS-NNN)
```
### TS-NNN: <제목>
- **증상**: ...
- **원인**: ...
- **해결**: ...
- **날짜**: YYYY-MM-DD
```

## 저장 위치
- `.claude/memory/troubleshooting-patterns.md`

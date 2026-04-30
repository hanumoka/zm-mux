---
name: zm-memory-save
description: 세션 종료 시 컨텍스트 저장. context.md 업데이트 + 작업 완료 기록
user-invocable: true
disable-model-invocation: false
argument-hint: "[작업 요약]"
---

# zm-memory-save

세션 컨텍스트를 `.project-memory/context.md`에 저장합니다.

## 실행 단계

1. **현재 상태 수집**
   - `git diff --stat` (변경 파일 목록)
   - `git status --short` (작업 트리 상태)
   - `git log --oneline -5` (최근 커밋)

2. **작업 유형 분류**
   - bugfix: 버그 수정 → known-mistakes.md 업데이트 검토
   - feature: 기능 추가 → TODOs 업데이트
   - research: 조사 → docs/ 업데이트 확인
   - none: 변경 없음

3. **context.md 업데이트** (섹션별)
   - `## Focus`: 현재 작업 초점
   - `## TODOs`: 할 일 목록 (완료 항목 체크)
   - `## Blockers`: 차단 요소
   - `## Decisions`: 이번 세션 결정사항
   - `## Metrics`: 프로젝트 지표

4. **bugfix인 경우**
   - 패턴이 반복 가능하면 known-mistakes.md에 M-NNN 추가 제안

## 인자
- 첫 번째 인자: 작업 요약 (선택)
  - 예: `/zm-memory-save "ConPTY 프로토타입 완료"`

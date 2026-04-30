---
name: zm-changelog
description: 변경 이력 조회 및 기록
user-invocable: true
disable-model-invocation: true
argument-hint: "[full]"
---

# zm-changelog

프로젝트 변경 이력을 관리합니다.

## 사용법

- `/zm-changelog` — 최근 변경사항 요약
- `/zm-changelog full` — 전체 changelog 표시

## 동작
1. `git log`에서 최근 커밋 수집
2. 카테고리별 분류 (feat/fix/docs/refactor/chore)
3. 구조화된 changelog 출력

---
name: zm-docs
description: docs/ 리서치 문서 관리 및 새 문서 생성
user-invocable: true
disable-model-invocation: true
argument-hint: "[status|new <주제>|update <번호>]"
---

# zm-docs

리서치 문서를 관리합니다.

## 사용법

- `/zm-docs status` — 전체 문서 목록 및 상태
- `/zm-docs new <주제>` — 새 리서치 문서 생성 (번호 자동 할당)
- `/zm-docs update <번호>` — 기존 문서 업데이트

## 문서 규칙
- 파일명: `NN-kebab-case.md` (예: `06-conpty-analysis.md`)
- 언어: 한국어
- 조사일 표시 필수
- docs/README.md 인덱스 동기화

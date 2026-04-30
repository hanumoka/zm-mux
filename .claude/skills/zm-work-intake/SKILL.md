---
name: zm-work-intake
description: 새 작업 수용 시 요구사항 검증 + 영향 분석
user-invocable: true
disable-model-invocation: false
argument-hint: "[요구사항 텍스트]"
---

# zm-work-intake

새로운 작업/요구사항을 수용할 때 사전 검증을 수행합니다.

## 실행 단계

1. **요구사항 파악**
   - 사용자 입력에서 핵심 요구사항 추출
   - 기존 docs/ 문서와 대조

2. **정책 검증**
   - `.claude/memory/policy-registry.md` 참조
   - 기존 ARCH/TECH/PROD 정책과 충돌 여부 확인
   - 충돌 발견 시 사용자에게 명시적 고지

3. **영향 분석**
   - 변경 범위 (어떤 모듈/파일에 영향?)
   - CLAUDE.md 설계 목표와 정합성

4. **실행 계획 제시**
   - 단계별 구현 계획
   - 예상 리스크
   - 필요한 추가 조사

5. **context.md 업데이트**
   - TODOs에 새 작업 추가
   - Focus 업데이트 (필요 시)

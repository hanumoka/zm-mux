# Claude Code + Codex 결합 워크플로우 (Dynamic Duo)

## 개요

OpenAI가 2026년 3월 30일 공식 릴리즈한 `codex-plugin-cc` 플러그인을 통해, Claude Code 내에서 OpenAI Codex를 직접 호출하여 코드 리뷰 및 작업 위임이 가능하다. 두 AI를 결합하여 계획 수립(Claude) + 검증(Codex)의 상호 보완적 워크플로우를 구현한다.

**출처**: [YouTube — Claude Code + Codex Dynamic Duo](https://www.youtube.com/watch?v=Fu5KIG2Jm1g)

---

## 1. 각 모델의 강점

| 역할 | Claude Code | Codex |
|------|------------|-------|
| **핵심 강점** | 카피라이팅, 디자인 사고, 코드 패턴 설계 | 지시사항 정확 실행, 코드 검토 |
| **비유** | 아키텍트/디자이너 | 숙련된 외과의사 |
| **포지션** | 메인 개발 도구 | 감사(Audit) 보조 |

---

## 2. 설치 및 설정

### 요구사항
- ChatGPT 구독 (Free 포함) 또는 OpenAI API 키
- Node.js 18.18+
- Codex CLI (`npm install -g @openai/codex`)

### 설치 절차
```bash
# Claude Code 내에서 실행
/plugin marketplace add openai/codex-plugin-cc
/plugin install codex@openai-codex
/reload-plugins
/codex:setup
```

### 인증
```bash
# Codex 로그인 (Claude Code 터미널에서)
! codex login
```

### 프로젝트 설정 (선택)
`.codex/config.toml`:
```toml
model = "gpt-5.4-mini"
model_reasoning_effort = "high"
```

---

## 3. 핵심 명령어

### `/codex:review` — 표준 코드 리뷰
- 미커밋 변경사항 또는 브랜치 비교 리뷰
- 코드를 수정하지 않고 결함만 검토
- 플래그: `--base <ref>`, `--wait`, `--background`

### `/codex:adversarial-review` — 적대적 리뷰 (가장 추천)
- "악마의 대변인" 역할 — 코드/계획의 허점을 집요하게 탐색
- 특정 위험 영역 집중 가능 (인증, 데이터 손실, 롤백 등)
- 커스텀 포커스 텍스트 지원

### `/codex:rescue` — 작업 위임
- Codex 서브에이전트에 작업 위임
- 버그 조사, 수정, 작업 계속 처리
- 플래그: `--background`, `--wait`, `--resume`, `--fresh`, `--model`, `--effort`

### `/codex:status` — 작업 상태 확인
- 현재 저장소의 실행 중/최근 작업 표시

### `/codex:result` — 완료 결과 조회
- Codex 세션 ID 포함하여 재개 가능

### `/codex:cancel` — 작업 취소

### `/codex:setup` — 설치/인증 확인
- Review gate 관리: `--enable-review-gate` / `--disable-review-gate`

---

## 4. 추천 워크플로우 패턴

### Phase 1: 계획 수립 (Planning First)
```
Claude Code로 초안 계획 작성
↓
/codex:adversarial-review 로 계획 허점 검출
↓
Claude Code로 허점 보완
↓
반복 (Codex가 더 이상 지적 없을 때까지)
↓
확정된 계획으로 코드 구현
```

### Phase 2: 구현 후 검증
```
Claude Code로 코드 구현
↓
/codex:review 로 코드 품질 검토
↓
필요시 /codex:rescue 로 특정 이슈 위임
```

### Review Gate (자동 검토)
- Claude가 코드 생성할 때마다 Codex 자동 더블 체크
- **주의**: 토큰 소모가 매우 큼 → 일상적 사용보다 중요 릴리즈 시 활용 권장
- 활성화: `/codex:setup --enable-review-gate`

---

## 5. 경제적 활용 전략

| 항목 | 비용 | 용도 |
|------|------|------|
| Claude Code (메인) | ~$100/월 | 설계, 구현, 일상 개발 |
| Codex (감사용) | ~$20/월 | adversarial review, 코드 리뷰 |
| **합계** | ~$120/월 | 2명의 전문가와 일하는 효과 |

---

## 6. zm-mux 적용 방안

### 적용 가치
- **높음**: Rust + 크로스 플랫폼 개발에서 안전성이 중요 (ConPTY/POSIX PTY 이중 관리)
- **높음**: 보안 민감 영역 (터미널 에뮬레이터, 프로세스 격리)
- **보통**: 초기 프로토타입 단계에서는 리뷰 대상 코드가 적음

### 도입 시점
- **즉시**: `/codex:adversarial-review`를 계획 검토에 활용
- **코드 작성 시작 후**: `/codex:review`를 PR 전 코드 리뷰에 활용
- **안정화 후**: Review gate 활성화 검토

### 기존 zm-mux 워크플로우와 통합
```
/zm-work-intake (요구사항 검증)
↓
Claude Code 계획 수립
↓
/codex:adversarial-review (계획 허점 검출)  ← 새로 추가
↓
구현
↓
/codex:review (코드 리뷰)  ← 새로 추가
↓
/zm-memory-save (세션 저장)
```

---

## 참고 링크

- [codex-plugin-cc GitHub](https://github.com/openai/codex-plugin-cc) (16.8k stars, Apache-2.0)
- [OpenAI 커뮤니티 발표](https://community.openai.com/t/introducing-codex-plugin-for-claude-code/1378186)
- [설치 가이드 (Medium)](https://medium.com/@markchen69/when-rivals-collaborate-installing-openais-codex-plugin-in-claude-code-5d3e503ce493)
- [fcakyon/claude-codex-settings](https://github.com/fcakyon/claude-codex-settings) — 실전 Claude+Codex 설정 레퍼런스
- [영상 원본](https://www.youtube.com/watch?v=Fu5KIG2Jm1g)

---

*조사일: 2026-04-30*

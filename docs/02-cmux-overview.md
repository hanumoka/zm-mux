# cmux - AI 코딩 에이전트를 위한 macOS 네이티브 터미널

## 개요

- **이름**: cmux
- **공식 사이트**: https://cmux.com/
- **GitHub**: https://github.com/manaflow-ai/cmux
- **개발사**: Manaflow AI
- **출시일**: 2026년 2월
- **라이선스**: 오픈소스 (무료)
- **플랫폼**: macOS 전용
- **기반 기술**: Ghostty의 libghostty 렌더링 엔진
- **GitHub Stars**: 출시 1개월 만에 7.7k+

---

## 핵심 설계 철학

> "Primitive, Not Solution"

완성된 워크플로우가 아닌 저수준 빌딩 블록(read-screen, send, notifications)을 제공하여, AI 에이전트가 자체 워크플로우를 조합하도록 설계되었다.

---

## 주요 기능

### 1. AI 에이전트 팀 네이티브 지원

```bash
cmux claude-teams --dangerously-skip-permissions
```

- 팀원/서브에이전트가 **네이티브 cmux pane split으로 자동 생성**
- 세로 컬럼에 자동 정렬, 에이전트 생성/종료 시 자동 리밸런싱
- Claude Code가 cmux 내부 실행 감지 시 (`CMUX_WORKSPACE_ID` / `CMUX_SURFACE_ID` 환경변수) 자동으로 cmux 백엔드 사용

### 2. 알림 시스템

프로세스가 주의를 요할 때 다층 알림 제공:
- 패널 주변 **알림 링** (시각적 인디케이터)
- 사이드바 **미읽음 뱃지**
- **알림 팝오버**
- macOS **데스크톱 알림**
- 표준 터미널 이스케이프 시퀀스 사용 (OSC 9/99/777)
- cmux CLI 및 Claude Code hooks로 트리거 가능

### 3. Vertical Tabs (사이드바)

각 워크스페이스 탭에 표시되는 정보:
- Git 브랜치명
- 연결된 PR 상태/번호
- 작업 디렉토리
- 리스닝 포트
- 최신 알림 텍스트

### 4. 내장 브라우저

- **WebKit 기반** (Safari와 동일 엔진)
- 터미널 패널 옆에 직접 내장
- 개발자 도구 접근 가능
- 별도 Chrome 탭이 아닌 인앱 브라우저

### 5. Remote Sessions

```bash
cmux ssh user@remote
```

- 원격 머신용 워크스페이스 자동 생성
- 브라우저 패널이 원격 네트워크를 통해 라우팅
- `localhost`가 원격 서버의 localhost로 작동

### 6. Socket API

- 스크립트/에이전트가 프로그래밍적으로 패널 제어 가능
- 외부 도구와의 통합 지원

### 7. GPU 가속 렌더링

- libghostty 기반으로 GPU 가속 터미널 렌더링
- 높은 성능과 부드러운 스크롤

---

## 설치 방법

```bash
brew tap manaflow-ai/cmux
brew install --cask cmux
```

---

## Claude Code 통합

### Oh My Claude Code (OMC) 통합

```bash
cmux omc
```

- cmux-aware 환경에서 OMC 실행
- 팀 패널이 네이티브 cmux split으로 생성

### 지원 AI 도구

cmux는 다음 AI 코딩 도구와 호환:
- Claude Code
- OpenAI Codex
- OpenCode
- Gemini CLI
- Kiro
- Aider
- 기타 모든 CLI 도구

---

## 관련 이슈/개발

- [cmux #123](https://github.com/manaflow-ai/cmux/issues/123) - Claude Code 에이전트 팀 네이티브 지원
- [claude-code #36926](https://github.com/anthropics/claude-code/issues/36926) - cmux를 teammateMode 백엔드로 지원 요청

---

## 참고 링크

- [cmux 공식 사이트](https://cmux.com/)
- [cmux GitHub](https://github.com/manaflow-ai/cmux)
- [cmux Better Stack 가이드](https://betterstack.com/community/guides/ai/cmux-terminal/)
- [cmux Product Hunt](https://www.producthunt.com/products/cmux)
- [cmux Terminal Guide](https://www.terminal.guide/tools/terminal-emulator/cmux/)
- [cmux 리뷰 (vibecoding.app)](https://vibecoding.app/blog/cmux-review)

---

*조사일: 2026-04-30*

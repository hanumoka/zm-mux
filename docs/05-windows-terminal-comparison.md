# Windows 터미널 프로그램 비교 (Claude Code 관점)

## 개요

Claude Code는 모든 터미널에서 설정 없이 동작하지만, 터미널별로 UX 차이가 존재한다. 본 문서는 Windows에서 사용 가능한 터미널을 Claude Code 호환성 관점에서 비교한다.

---

## 터미널 비교표

| 터미널 | Shift+Enter | 알림 지원 | GPU 가속 | 에이전트 팀 Split-pane | 가격 |
|--------|------------|----------|----------|----------------------|------|
| **Windows Terminal** | X (Ctrl+J 대체) | X | X | X | 무료 (내장) |
| **Alacritty** | 설정 필요 | X | O | X | 무료 |
| **WezTerm** | O (네이티브) | X | O | X | 무료 |
| **Ghostty** | O (네이티브) | O (desktop notify) | O | X (공식 미지원) | 무료 |
| **wmux** | - | O | X (xterm.js) | O (MCP 기반) | 무료 |
| **VS Code Terminal** | X | X | X | X | 무료 |
| **Hyper** | - | X | X | X | 무료 |

---

## 각 터미널 상세

### Windows Terminal (기본 내장)

- **장점**: 무료, 기본 설치, 안정적, 탭 지원, 유니코드/컬러 우수
- **단점**: Shift+Enter 미지원 (Ctrl+J로 대체), 에이전트 팀 split-pane 미지원
- **Claude Code 사용 시**: 일상적 사용에 무난, `/terminal-setup` 실행 권장
- **적합 대상**: 기본적인 Claude Code 사용자

### Alacritty

- **장점**: 최상의 성능 (GPU 가속, Rust 기반), 가벼움, 미니멀
- **단점**: GUI 설정 없음 (YAML 파일 설정), Shift+Enter 별도 설정 필요
- **Claude Code 사용 시**: `/terminal-setup` 실행으로 Shift+Enter 설정
- **적합 대상**: 성능 우선 사용자

### WezTerm

- **장점**: GPU 가속, Shift+Enter 네이티브 지원, 풍부한 기능, Lua 스크립팅, 크로스플랫폼
- **단점**: Alacritty 대비 약간 무거움
- **Claude Code 사용 시**: 추가 설정 없이 바로 사용 가능
- **적합 대상**: 기능과 성능 균형을 원하는 사용자

### Ghostty

- **장점**: 최신 터미널, GPU 가속, Shift+Enter 네이티브, 알림 지원 (desktop notify)
- **단점**: Windows 지원 상태 확인 필요, 에이전트 팀 split-pane 공식 미지원
- **Claude Code 사용 시**: 알림까지 지원하는 가장 현대적 선택
- **적합 대상**: 최신 기능을 원하는 사용자

### wmux (AI 에이전트 전용)

- **장점**: AI 에이전트 워크플로우 특화, split-pane 에이전트 관리, MCP 통합, 내장 브라우저
- **단점**: Electron 기반 (메모리 오버헤드), 아직 초기 단계
- **Claude Code 사용 시**: 에이전트 팀 운영 시 가장 적합
- **적합 대상**: AI 에이전트 병렬 운영 사용자

---

## Claude Code에서 중요한 터미널 기능

### 1. Shift+Enter (줄바꿈)
멀티라인 입력에 필수. 미지원 시 `Ctrl+J` 또는 `\`로 대체.

| 지원 수준 | 터미널 |
|----------|--------|
| 네이티브 지원 | WezTerm, Ghostty |
| 설정 후 지원 | Alacritty (`/terminal-setup`) |
| 미지원 (대체 필요) | Windows Terminal, VS Code |

### 2. 알림 (Notifications)
에이전트 작업 완료 등을 알려주는 데스크톱 알림.

| 지원 수준 | 터미널 |
|----------|--------|
| 네이티브 지원 | Ghostty, wmux |
| hooks로 가능 | 기타 터미널 (Stop hook 설정) |

### 3. 유니코드/컬러
Claude Code UI 렌더링에 필요. 모든 최신 터미널에서 지원.

### 4. GPU 가속
긴 출력 처리 시 성능 차이. Alacritty, WezTerm, Ghostty가 지원.

---

## 추천 조합

| 사용 시나리오 | 추천 터미널 | 이유 |
|-------------|-----------|------|
| 기본 사용 | Windows Terminal | 무료, 안정적, 추가 설치 불필요 |
| 성능 중시 | Alacritty | 가장 빠름, 가벼움 |
| 기능 + 성능 균형 | WezTerm | Shift+Enter 네이티브, GPU |
| 최신 경험 | Ghostty | 알림까지 지원 |
| AI 에이전트 팀 운영 | wmux | split-pane 에이전트 관리 |
| 범용 + 에이전트 | WezTerm + wmux 병행 | 일상은 WezTerm, 팀은 wmux |

---

## 참고 링크

- [Claude Code 터미널 설정 가이드](https://code.claude.com/docs/en/terminal-setup)
- [Alacritty 공식](https://alacritty.org/)
- [WezTerm 공식](https://wezfurlong.org/wezterm/)
- [Ghostty 공식](https://ghostty.org/)
- [Windows Terminal 공식](https://github.com/microsoft/terminal)

---

*조사일: 2026-04-30*

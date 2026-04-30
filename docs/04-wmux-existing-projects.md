# 기존 wmux 프로젝트 분석

## 개요

cmux가 macOS 전용이기 때문에, Windows 사용자들이 유사한 경험을 얻기 위한 다수의 wmux 프로젝트가 존재한다. 본 문서는 기존 구현체들을 분석하고 비교한다.

---

## 주요 프로젝트 목록

### 1. openwong2kim/wmux (가장 활발)

- **GitHub**: https://github.com/openwong2kim/wmux
- **설명**: Windows tmux alternative for AI agents
- **기술 스택**: Electron + xterm.js
- **설치**: winget, Chocolatey, PowerShell one-liner, Setup.exe
- **라이선스**: MIT

#### 주요 기능
- **WSL 불필요** - 네이티브 Windows (ConPTY)
- **Split pane**: `Ctrl+D`로 분할, `Ctrl+N`으로 새 워크스페이스
- **세션 지속성**: 앱 재시작 후에도 세션 유지, 스크롤백 보존
- **CDP 브라우저 자동화**: localhost:9222로 Claude Code의 chrome-devtools-mcp 연동
- **MCP 서버 자동 등록**: AI 에이전트 자동 감지, 내장 브라우저/터미널 출력 제어
- **스마트 알림**: 에이전트 진행상황, 작업 완료, 중요 동작 알림

#### 설치 방법
```powershell
# winget (권장)
winget install openwong2kim.wmux

# 또는 Chocolatey
choco install wmux
```

#### 특이사항
- DEV Community에 상세 가이드 존재
- MCP Market에 등록됨
- Claude Code, Codex, Gemini CLI 동시 실행 지원

---

### 2. amirlehmam/wmux

- **GitHub**: https://github.com/amirlehmam/wmux
- **설명**: Windows terminal multiplexer for AI agents. Port of cmux.
- **기술 스택**: ConPTY + Electron
- **특징**: cmux 공식 포트를 지향

#### 주요 기능
- cmux 프로토콜 호환 (cmux용 도구가 wmux에서도 동작)
- tmux 스타일 split panes, prefix keys
- 세션 지속성

---

### 3. fernandomenuk/wmux

- **GitHub**: https://github.com/fernandomenuk/wmux
- **문서 사이트**: https://fernandomenuk.github.io/wmux/
- **설명**: tmux for Windows — split panes, tabbed workspaces, and a JSON-RPC socket API for AI agents
- **태그라인**: "cmux for Windows"

#### 주요 기능
- JSON-RPC Socket API (프로그래밍적 제어)
- Tabbed workspaces
- Split panes

---

### 4. mkurman/cmux-windows

- **GitHub**: https://github.com/mkurman/cmux-windows
- **설명**: Windows terminal with useful features. Fully vibe-coded using CC and Codex.
- **특징**: cmux의 직접적 Windows 재구현 시도

---

### 5. mohemohe/wmux (레거시)

- **GitHub**: https://github.com/mohemohe/wmux
- **설명**: windowed terminal multiplexer
- **기술 스택**: Go
- **특징**: AI 에이전트 이전 세대의 wmux, 범용 목적

---

### 6. christopherfujino/wmux (레거시)

- **GitHub**: https://github.com/christopherfujino/wmux
- **설명**: Windows Command Prompt Multiplexer
- **특징**: 초기 Windows 멀티플렉서 시도

---

## 유사 프로젝트

### winsmux

- **GitHub**: https://github.com/Sora-bluesky/winsmux
- **설명**: Native Windows terminal multiplexer with cross-pane AI agent communication
- **특징**: WSL2 불필요, 크로스 패널 AI 에이전트 통신 지원

### limux

- **GitHub**: https://github.com/am-will/limux
- **설명**: GPU-accelerated terminal multiplexer for Linux
- **특징**: cmux의 Linux 포트 (Ghostty 렌더링 엔진 기반)

---

## 프로젝트 비교표

| 프로젝트 | 기술 스택 | WSL 필요 | Socket API | MCP 통합 | 브라우저 내장 | 성숙도 |
|----------|-----------|----------|------------|----------|-------------|--------|
| openwong2kim/wmux | Electron + xterm.js | X | - | O (자동) | O (CDP) | 높음 |
| amirlehmam/wmux | ConPTY + Electron | X | - | - | - | 중간 |
| fernandomenuk/wmux | - | X | O (JSON-RPC) | - | - | 중간 |
| mkurman/cmux-windows | - | - | - | - | - | 초기 |
| winsmux | - | X | - | - | - | 초기 |

---

## cmux와의 기능 비교

| 기능 | cmux (macOS) | wmux (Windows) 최선 |
|------|-------------|-------------------|
| GPU 가속 렌더링 | O (libghostty) | X (xterm.js) |
| 네이티브 앱 | O (Swift/macOS) | 부분 (Electron) |
| 알림 시스템 | 풍부 (링, 뱃지, 데스크톱) | 기본 |
| 내장 브라우저 | O (WebKit) | O (CDP, openwong2kim) |
| Vertical Tabs | O | 구현체에 따라 다름 |
| Socket API | O | O (fernandomenuk) |
| Claude Code 팀 통합 | 네이티브 | MCP 기반 (openwong2kim) |
| 세션 지속성 | 제한적 | O (openwong2kim) |

---

## 결론 및 시사점

1. **openwong2kim/wmux**가 가장 완성도 높고 활발한 프로젝트
2. 모든 wmux는 아직 cmux 대비 기능 격차 존재 (특히 GPU 렌더링, 알림 시스템)
3. cmux 프로토콜 호환성을 갖춘 프로젝트가 장기적으로 유리
4. Electron 기반이 대다수 — 성능/메모리 오버헤드 고려 필요

---

## 참고 링크

- [wmux DEV Community 가이드](https://dev.to/wong2kim/wmux-run-claude-code-codex-and-gemini-cli-side-by-side-on-windows-pkg)
- [wmux MCP Market](https://mcpmarket.com/server/wmux)
- [wmux 개발 배경 스토리](https://dev.to/wong2kim/i-tried-to-vibe-code-on-windows-it-broke-me-so-i-built-my-own-terminal-17a)
- [wmux winstall](https://winstall.app/apps/openwong2kim.wmux)

---

*조사일: 2026-04-30*

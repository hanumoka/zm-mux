# tmux vs cmux 비교 분석

## 개요

tmux(2007)는 19년 역사의 범용 터미널 멀티플렉서이고, cmux(2026)는 AI 에이전트 시대를 위해 새로 설계된 macOS 네이티브 터미널이다. 두 도구는 대체재가 아닌 **보완재** 관계이다.

---

## 상세 비교표

| 항목 | tmux | cmux |
|------|------|------|
| **출시** | 2007년 | 2026년 2월 |
| **역사** | 19년 | 신생 |
| **플랫폼** | Linux, macOS, *BSD, WSL | macOS 전용 |
| **아키텍처** | 클라이언트-서버 (텍스트 기반) | 네이티브 macOS 앱 (GUI) |
| **렌더링** | 텍스트 기반 | GPU 가속 (libghostty) |
| **설계 목적** | 범용 터미널 멀티플렉싱 | AI 에이전트 워크플로우 |
| **세션 지속성** | 강력 (SSH 끊겨도 유지) | 제한적 (앱 재시작 시 유지) |
| **원격 서버** | 핵심 강점 (detach/attach) | `cmux ssh`로 지원 |
| **알림 시스템** | 없음 (hooks 필요) | 네이티브 (링, 뱃지, 데스크톱) |
| **내장 브라우저** | 없음 | WebKit 내장 |
| **Vertical Tabs** | 없음 | 지원 (git, PR, 포트 표시) |
| **Socket API** | 없음 | 지원 |
| **플러그인 생태계** | 풍부 (19년 축적) | 초기 단계 |
| **CI/CD 연동** | 강력 | 해당 없음 |
| **학습 곡선** | 높음 | 낮음 (GUI) |
| **설치** | 패키지 매니저 | `brew install --cask cmux` |

---

## Claude Code 에이전트 팀 지원 비교

| 기능 | tmux | cmux |
|------|------|------|
| teammateMode 공식 지원 | O (`teammateMode: "tmux"`) | 개발 중 (이슈 #36926) |
| Split-pane 에이전트 표시 | O (tmux pane) | O (네이티브 pane) |
| 에이전트 자동 정렬 | 수동 설정 | 자동 (세로 컬럼) |
| 에이전트 알림 | 없음 | 시각적 링 + 뱃지 |
| 에이전트 I/O 실시간 모니터링 | 가능 (pane 전환) | 가능 (모든 pane 동시 표시) |
| 에이전트 종료 시 pane 처리 | 수동 | 자동 리밸런싱 |

### tmux에서 Claude Code 사용 시 필수 설정

`~/.tmux.conf`:
```bash
set -g allow-passthrough on
set -s extended-keys on
set -as terminal-features 'xterm*:extkeys'
```

이 설정이 없으면:
- Shift+Enter 줄바꿈 불가
- 데스크톱 알림 패스스루 불가
- 프로그레스 바 표시 불가

---

## 사용 시나리오별 추천

| 시나리오 | 추천 | 이유 |
|----------|------|------|
| 로컬 macOS에서 AI 에이전트 다수 운영 | **cmux** | 알림, 시각적 모니터링, 자동 정렬 |
| 원격 서버 작업 | **tmux** | 세션 지속성, detach/attach |
| SSH 세션 유지 | **tmux** | 핵심 강점 |
| CI/CD 파이프라인 | **tmux** | 서버 환경 지원 |
| Claude Code 팀 시각적 관리 | **cmux** | 네이티브 팀 지원 |
| Windows 환경 | **둘 다 제한적** | wmux 또는 WSL+tmux 필요 |
| 복합 사용 (로컬+원격) | **cmux + tmux 병행** | 각각의 강점 활용 |

---

## 결론

- **tmux**: 서버, 원격, CI/CD 등 인프라 중심 작업에 여전히 필수
- **cmux**: AI 에이전트 병렬 운영 시 macOS에서 최적의 경험 제공
- **둘은 대체재가 아닌 보완재**: 로컬에서는 cmux, 원격에서는 tmux

---

## 참고 링크

- [cmux vs tmux 비교 (soloterm)](https://soloterm.com/cmux-vs-tmux)
- [tmux vs cmux 상세 비교 (ice-ice-bear)](https://ice-ice-bear.github.io/posts/2026-03-23-tmux-cmux/)
- [Claude Code + tmux 워크플로우](https://www.blle.co/blog/claude-code-tmux-beautiful-terminal)
- [Claude Code 에이전트 팀 문서](https://code.claude.com/docs/en/agent-teams)

---

*조사일: 2026-04-30*

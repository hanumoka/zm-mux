# Windows에서 Claude Code 사용 시 알려진 문제점

## 개요

Claude Code는 Windows 10 1809+ 및 Windows Server 2019+를 공식 지원하지만, macOS/Linux 대비 다수의 제한사항과 버그가 존재한다.

---

## 1. 에이전트 팀 (Agent Teams) 제한사항

에이전트 팀은 실험적(experimental) 기능으로, Windows에서 가장 큰 제약이 있는 영역이다.

### Split-pane 모드 미지원

| 기능 | macOS (tmux/cmux) | Windows |
|------|-------------------|---------|
| Split-pane 모드 (각 에이전트 별도 패널) | O | **X** |
| In-process 모드 (단일 터미널) | O | O |
| 에이전트 간 실시간 시각적 모니터링 | O | **X** |
| 패널 클릭으로 에이전트 전환 | O | **X** |
| Shift+Down 에이전트 순환 | O | O (in-process만) |
| 공유 태스크 리스트 | O | O |
| 에이전트 간 메시징 | O | O |

공식 문서 원문:
> "Split-pane mode isn't supported in VS Code's integrated terminal, Windows Terminal, or Ghostty."
> "tmux has known limitations on certain operating systems and traditionally works best on macOS."

### 기타 에이전트 팀 제한
- `/resume`, `/rewind` 시 in-process 팀원 복원 불가
- 팀원이 태스크 완료 표시를 놓치는 경우 발생
- 세션당 1개 팀만 운영 가능
- 중첩 팀 불가 (팀원이 자체 팀 생성 불가)
- 종료(shutdown) 지연 발생

---

## 2. 알려진 버그

### GitHub #54865 - `/resume` 경로 인코딩 문제

- **영향**: MSYS2/Git Bash 사용자
- **증상**: `/resume` 실행 시 이전 세션 목록이 비어있음
- **원인**: Windows 경로 형식이 비정규화 인코딩됨
  - `U:\0_Projects\...` → 인코딩 A
  - `U:/0_Projects/...` → 인코딩 B
  - `/u/0_Projects/...` → 인코딩 C
- **결과**: 동일 프로젝트가 서로 다른 디렉토리로 매핑
- **우회**: `claude --resume <session-id>` 직접 지정

### GitHub #54870 - Windows 11 Home 행(Hang) 문제

- **영향**: Windows 11 Home Edition 25H2 Build 26200.8246
- **증상**: Cold-start 시 AppHangXProcB1 발생 (7일간 17회)
- **관련 오류**:
  - `"YukonSilver not supported"` (Cowork 기능)
  - `"Claude Code native binary not found"` (바이너리 존재하나 감지 실패)
  - 403 Cloudflare Turnstile challenge loop
- **Windows 11 Pro에서는 미발생**

---

## 3. 기능별 제한사항

### 샌드박싱 (Sandboxing)
- 네이티브 Windows: **미지원**
- WSL 2: 지원
- WSL 1: 미지원

### Chrome 통합
- WSL에서 **미지원** (네이티브 Windows 또는 Edge 필요)
- Named pipe 충돌 가능 (다수 Claude Code 인스턴스 동시 사용 시)
- Chrome/Edge만 지원 (Brave, Arc 등 미지원)

### MCP 서버
- Windows Desktop 앱에서 MCP 서버 연결 불안정 보고

### PowerShell 도구
- 점진적 배포 중 (완전 지원 아님)
- `CLAUDE_CODE_USE_POWERSHELL_TOOL=1`로 수동 활성화 필요

### Shift+Enter (줄바꿈)
- Windows Terminal에서 네이티브 미지원
- `Ctrl+J` 또는 `\`로 대체 필요

---

## 4. 설치 관련 문제

### 셸 요구사항
- Bash (Git for Windows 권장) 또는 PowerShell 필요
- Git for Windows 미설치 시 PowerShell로 폴백
- 둘 다 없으면 오류 발생

### 흔한 설치 오류
| 오류 | 원인 |
|------|------|
| `'irm' is not recognized` | CMD에서 PowerShell 인스톨러 실행 |
| `'bash' is not recognized` | macOS/Linux 인스톨러를 Windows에서 실행 |
| `The process cannot access the file` | 안티바이러스가 다운로드 폴더 잠금 |
| 32-bit 감지 오류 | Windows PowerShell (x86) 사용 |

---

## 5. 참고 링크

- [Claude Code 공식 설치 문서](https://code.claude.com/docs/en/setup.md)
- [Claude Code 설치 문제 해결](https://code.claude.com/docs/en/troubleshoot-install.md)
- [에이전트 팀 문서](https://code.claude.com/docs/en/agent-teams.md)
- [GitHub Issues - Windows 관련](https://github.com/anthropics/claude-code/issues)

---

*조사일: 2026-04-30*

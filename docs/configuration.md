# zm-mux 설정 (config.toml)

zm-mux는 시작 시 설정 파일을 읽는다. 없으면 전부 기본값, 미지 필드는 무시, 파싱 실패 시 경고 후 기본값.

## 경로
- 환경변수 `ZM_MUX_CONFIG` 가 있으면 그 경로.
- Windows: `%APPDATA%\zm-mux\config.toml`
- Linux/macOS: `$XDG_CONFIG_HOME/zm-mux/config.toml` 또는 `~/.config/zm-mux/config.toml`

## 스키마 (기본값)

```toml
[font]
family = ""          # 비우면 플랫폼 기본 모노스페이스 자동 탐색
                     #  Windows: Cascadia Mono → Cascadia Code → Consolas → Lucida Console
                     #  (지정 폰트가 미설치면 자동 폴백)
size = 14.0          # 논리 px (HiDPI는 scale 자동 적용)

[colors]
background = "#0c0c0c"
foreground = "#cccccc"
cursor     = ""      # 비우면 foreground 사용

[scrollback]
max_lines = 10000

[shell]
program = ""         # 비우면 %COMSPEC%(cmd.exe) / $SHELL
args = []            # 예: program="pwsh", args=["-NoLogo"]

[keybindings]        # 프리픽스 없는 직접 단축키(문자열)
new_tab          = "Ctrl+T"
close_tab        = "Ctrl+Shift+W"
close_pane       = "Ctrl+Shift+P"
split_horizontal = "Ctrl+Shift+D"   # 상하 분할(가로 divider)
split_vertical   = "Ctrl+Shift+E"   # 좌우 분할(세로 divider)
next_tab         = "Ctrl+Tab"
prev_tab         = "Ctrl+Shift+Tab"
focus_left       = "Alt+Left"
focus_right      = "Alt+Right"
focus_up         = "Alt+Up"
focus_down       = "Alt+Down"
zoom             = "Ctrl+Shift+Z"   # 활성 pane 전체화면 토글

[agent]              # 에이전트 연동(트랙 A)
tmux_shim = true            # pane에 tmux-shim env 주입 + PATH에 tmux shim
claude_agent_teams = true   # CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1 주입
```

## 단축키 문자열 형식
- `수식어+...+키`. 수식어: `Ctrl`/`Control`, `Shift`, `Alt`/`Option`. (`Super`/`Win`/`Cmd`는 무시)
- 키: 한 글자(`T`, `D`, …) 또는 이름(`Tab`, `Enter`, `Space`, `Esc`, `Left/Right/Up/Down`,
  `Backspace`, `Delete`, `Home`, `End`, `PageUp`, `PageDown`).
- 빈 문자열 = 해당 동작 미바인딩.
- 수식어는 **정확히 일치**해야 발동(예: `Ctrl+T`는 `Ctrl+Shift+T`에 반응 안 함).

## 동작
- **분할**: split_vertical = 좌우(세로 분할선), split_horizontal = 상하(가로 분할선).
- **포커스 이동**: 활성 pane 기준 해당 방향에서 (수직/수평 겹침이 있는) 가장 가까운 pane.
- **마우스**: 클릭 → 그 pane 포커스. divider 드래그 → 분할 비율 조정.
- **마우스 휠**: 활성 pane 스크롤백. 키 입력 시 자동으로 최하단(라이브)으로 스냅.
- **줌**: 활성 pane을 탭 전체로 확대/복귀 토글. (분할/닫기 시 자동 해제)
- **탭/pane 닫기**: 마지막 pane을 닫으면 앱 종료.

## 참고
- 셀별 배경색·블록/빔/언더라인 커서·sRGB 정합 렌더(현재). CPU 폴백은 후속.
- 분할 사이 divider 6px(고정), 활성 pane은 파란 프레임.

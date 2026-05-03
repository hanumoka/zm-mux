# Known Mistakes Registry

실수 패턴을 기록하여 반복을 방지합니다.
- `[BLOCK]`: mistake-guard.sh에서 자동 차단
- `[WARN]`: post-review.sh에서 경고

---

### M-001 [WARN] Electron 기본 선택

- **실수**: 성능 요구사항 분석 없이 Electron을 기본 선택
- **올바른 방법**: GPU 가속 렌더링이 핵심 목표이므로, 네이티브 렌더링 옵션(DirectX/Vulkan, WebGPU)을 우선 검토
- **탐지**: `electron`

### M-002 [BLOCK] 시크릿 하드코딩

- **실수**: 코드/문서에 API 키, 비밀번호, 토큰 직접 포함
- **올바른 방법**: 환경변수 또는 설정 파일(.env) 사용
- **탐지**: `(sk-[a-zA-Z0-9]{20,}|ghp_[a-zA-Z0-9]{30,}|password\s*[:=]\s*["'][^"']{8,})`

### M-003 [WARN] WSL 의존

- **실수**: WSL 필수 의존으로 설계 (네이티브 Windows가 목표)
- **올바른 방법**: ConPTY 기반 네이티브 Windows 구현, WSL은 선택적 지원
- **탐지**: `wsl.{0,20}require`

### M-004 [WARN] VT 에뮬레이션 직접 구현

- **실수**: VT100/xterm 파싱을 처음부터 직접 구현
- **올바른 방법**: `alacritty_terminal` 크레이트 재사용 (COSMIC Terminal 검증 패턴, ARCH-07)
- **탐지**: `vte.*parser.*impl|custom.*vt.*parse`

### M-005 [WARN] alacritty_terminal EventListener no-op

- **실수**: `EventListener::send_event` 를 빈 구현으로 두면 `Event::PtyWrite` 가 모두 누락됨 → DSR(`\x1b[6n`) 같은 터미널 쿼리에 응답 못함 → 셸이 응답 대기로 무한 정지 (`452ce1c` 검은 화면 원인)
- **올바른 방법**: 이벤트를 `Arc<Mutex<Vec<Event>>>` 같은 공유 큐에 push. `feed_bytes` 호출 후 큐를 drain 해 `Event::PtyWrite` 의 String을 PTY writer로 다시 write. `zm-term::ZmTerm::drain_pty_writes()` 참조.
- **탐지**: `fn\s+send_event\s*\([^)]*\)\s*\{\s*\}`

### M-006 [WARN] winit `about_to_wait` 무조건 request_redraw

- **실수**: winit 0.30+ `ApplicationHandler::about_to_wait` 에서 조건 없이 `window.request_redraw()` 호출하면 즉시 RedrawRequested 이벤트 → about_to_wait 재호출 → busy loop. CPU 100%, 락 starvation, PTY reader thread 굶음 (`452ce1c` CPU 114s/min 원인)
- **올바른 방법**: dirty flag 로 게이팅 + `ControlFlow::WaitUntil(now + 16ms)` 폴링 또는 `EventLoopProxy::send_event` 로 외부 스레드 신호
- **탐지**: `fn\s+about_to_wait[^{]*\{[^}]*request_redraw\(\)`

### M-007 [WARN] winit `LogicalSize` 에 physical pixel 사용

- **실수**: 셀 크기(픽셀) × cols/rows 결과를 `LogicalSize::new(...)` 에 넘김 → DPI 1.25/1.5x 환경에서 윈도우가 25~50% 더 크게 뜸 (`452ce1c` 첫 스크린샷의 거대 윈도우 원인)
- **올바른 방법**: 픽셀 단위면 `PhysicalSize::new(...)`. `LogicalSize` 는 DPI 미적용 좌표일 때만.
- **탐지**: `LogicalSize::new\([^)]*\b(cell|pixel|width|height|req_w|req_h)\b`

### M-008 [WARN] cosmic-text 메이저 버전을 통합 크레이트(glyphon 등)와 lockstep 없이 올림

- **실수**: `cosmic-text = "0.19"` 같은 직접 의존을 통합 크레이트(`glyphon`, `iced`, 그 외 cosmic 종속)가 받을 수 있는 버전보다 앞서 올림 → 두 cosmic-text 인스턴스가 graph 에 공존하는데 타입(`Buffer`, `FontSystem`)이 서로 호환 안 됨 → 통합 크레이트로 buffer 를 넘기는 순간 컴파일 실패. (`aa18eed` 1.3.9-B-1 에서 cosmic-text 0.19 → 0.18 다운그레이드로 해소)
- **올바른 방법**: cosmic-text 의존을 직접 두기 전에 **함께 쓸 통합 크레이트의 Cargo.toml 에서 cosmic-text 버전 핀**을 먼저 확인하고 그것에 맞춤. 통합 크레이트가 새 cosmic-text 를 받을 때까지 직접 의존을 올리지 않음.
- **탐지**: `cosmic-text\s*=` 와 함께 `glyphon\s*=` 또는 `iced\s*=` 가 같은 워크스페이스에 있을 때 두 의존이 서로 다른 cosmic-text 버전 범위를 가리키면 경고

### M-009 [WARN] cosmic-text Shaping::Basic 으로 CJK 렌더링

- **실수**: `Shaping::Basic` 은 시스템 폰트 폴백을 수행하지 않음. 주 폰트(JetBrains Mono)에 한글/CJK 글리프가 없으면 빈 상자 또는 깨진 문자로 표시됨.
- **올바른 방법**: 다국어 텍스트가 필요한 모든 렌더링 경로에서 `Shaping::Advanced` 사용. Advanced 는 harfrust 를 통해 시스템 폰트에서 누락 글리프를 자동 검색.
- **탐지**: `Shaping::Basic` (셀 프로브용 "M" 측정 제외)

### M-010 [WARN] winit 마우스 좌표와 pane layout 좌표 DPI 불일치

- **실수**: 고 DPI 환경(125%, 150%)에서 winit 의 `CursorMoved` 물리 픽셀 좌표와 pane layout 좌표 사이 불일치 발생 가능. 보더 감지 tolerance 가 너무 작으면(4px) 보더를 클릭할 수 없음.
- **올바른 방법**: tolerance 를 충분히 크게(8px+) 설정. 드래그 중에는 매 프레임 start position 을 업데이트하여 보더 위치 변경에 따른 미스매치 방지.
- **탐지**: `border_hit_test.*tolerance.*[1-4]\b`

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

# Claude Tank

<img src="img/icon.png" alt="Claude Tank Icon" width="64">

> Windows용 Claude 플랜 사용량 모니터

**🌐 언어:** [English](../README.md) | [日本語](README.ja.md) | [Deutsch](README.de.md) | 한국어 | [Français](README.fr.md)

---

**Claude Tank**는 Claude Pro/Max/Team 플랜의 사용량 제한을 Windows 시스템 트레이에서 실시간으로 모니터링하는 경량 앱입니다.

더 이상 갑작스러운 속도 제한에 놀라지 마세요 — 5시간 및 7일 잔여 용량을 한눈에 확인하세요.

## 기능

- **듀얼 게이지 트레이 아이콘** — 5h와 7d 잔여량을 색상 바로 표시
- **사이버 테마 대시보드** — 트레이 클릭으로 탱크형 게이지 팝업
- **자동 로그인** — 내장 브라우저에서 claude.ai에 로그인, 쿠키 자동 감지
- **백그라운드 폴링** — 1/3/5분 간격으로 사용량 자동 업데이트
- **임계값 알림** — 5h와 7d 독립 알림
- **암호화 저장** — Windows DPAPI로 세션 키 암호화
- **자동 시작** — Windows 로그인 시 자동 실행 (선택)
- **다국어** — 영어, 일본어, 독일어, 한국어, 프랑스어 (OS 언어 자동 감지)
- **경량** — exe ~2MB, RAM ~20MB, 설치 프로그램 불필요

## 요구사항

- Windows 10 (1903+) 또는 Windows 11
- [Microsoft Edge WebView2 런타임](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) — Windows 11에는 보통 미리 설치되어 있지만 보장되지는 않습니다. 앱이 실행되지 않으면 링크에서 **Evergreen Standalone Installer**를 설치하세요.

## 빠른 시작

1. [Releases](https://github.com/QuatrexEX/claude-tank/releases)에서 `claude-tank.exe` 다운로드
2. 실행 → claude.ai 로그인 창이 열림
3. 정상적으로 로그인 (Google, 이메일, SSO)
4. 완료! 트레이 아이콘에 사용량 게이지가 표시됩니다

## 소스에서 빌드

필요: [Rust](https://rustup.rs/) (stable)

```bash
cargo build --release
```

## 라이선스

[MIT](../LICENSE)

## 작성자

**Quatrex** — [@QuatrexEX](https://github.com/QuatrexEX)

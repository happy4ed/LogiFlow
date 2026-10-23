# LogiFlow 0.15.0 — Windows · Mac · Linux

## 받을 파일

| 운영체제 | 파일 | 상태 |
|---|---|---|
| Windows | `LogiFlow-v0.15.0-setup.exe` | 실기 확인(0.14.3 과 같은 코드) |
| Mac | `LogiFlow-v0.15.0-mac.dmg` | 실기 미검증(첫 전환판) |
| Linux x64 | `LogiFlow-v0.15.0-linux-x64.tar.gz` | 실기 미검증(CLI) |
| Linux ARM | `LogiFlow-v0.15.0-linux-arm64.tar.gz` | 실기 미검증(CLI) |

늘 최신을 받는 고정 링크(북마크용): `releases/latest/download/` 뒤에
`LogiFlow-windows-setup.exe` · `LogiFlow-mac.dmg` · `LogiFlow-linux-x64.tar.gz` · `LogiFlow-linux-arm64.tar.gz`.

이번 판부터 세 OS 를 한 릴리스에 함께 낸다(맨 위 Latest).

## Windows

0.14.3 그대로(버전만 0.15.0). 서비스가 바탕화면·로그온 화면 전환을 맡는 B안.

- glow: 미는 동안 80% → 전환 요청 90% → setHost 가 실제로 나간 순간 흰빛 최고점 → 사라짐.
- 전 기기 setHost 연달아 보내고 확인은 그 뒤 · 실패 기기만 깨워 재시도.
- 백그라운드 스캔 60초 간격·전환 중/후 10초 금지(HID 경합 방지).

## Mac (처음 전환 되는 판)

- 채널 전환 버튼·메뉴 막대 ⇄ · 변으로 밀어 넘기기(채널마다 화면·변 지정) · glow(윈도우와 같은 규칙).
- 미서명: 처음 한 번 LogiFlow.app 우클릭 → 열기.
- 로그: `~/Library/Logs/LogiFlow/logiflow-diag.log`.

## Linux (터미널 CLI)

- `sudo ./install.sh` 후 `logiflow list` · `logiflow switch 2`.
- GNOME 단축키에 `logiflow switch N` 을 걸면 키 하나로 전환. 변 밀기·glow 는 다음 단계(GNOME 확장).

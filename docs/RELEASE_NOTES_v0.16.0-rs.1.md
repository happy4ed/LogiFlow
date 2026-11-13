# LogiFlow 0.16.0-rs.1 — Windows 러스트 시험판(실기 대기)

**정식 아님 — 회장님 PC 확인용 시험판.** C# 판 0.15.0 과 같은 동작·같은 화면을 러스트로 옮긴 첫 판. C# 판과 나란히 설치된다.

## 받을 파일

| 파일 | 내용 |
|---|---|
| `LogiFlowRs-v0.16.0-rs.1-setup.exe` | 설치 파일(x64·ARM64 한 파일, PC 에 맞는 쪽 설치) |
| `LogiFlowRs-v0.16.0-rs.1-win-x64-arm64.zip` | 실행 파일만 |

받는 곳: `rust-port` 브랜치 GitHub Actions 실행 페이지의 아티팩트(로그인 필요). 릴리스(Latest)에는 올리지 않음.

## 나란히 설치

1. 설치 폴더 `%LOCALAPPDATA%\Programs\LogiFlowRs`, 실행 파일 `LogiFlowRs.exe`, 시작 메뉴 「LogiFlow (러스트 시험판)」.
2. 설정 `%APPDATA%\LogiFlowRs` · 머신 공유 `%ProgramData%\LogiFlowRs` · 서비스 `LogiFlowRsLogonSwitch` — C# 판과 겹치지 않음.
3. 첫 실행 때 C# 판 `config.txt`·`devices.txt` 를 이어받음(C# 원본은 읽기만). 시험판 한정: 자동 시작 꺼짐 · 로그온 전에도 실행 꺼짐.
4. C# 판 트레이가 떠 있거나 C# 판 서비스가 등록돼 있으면 러스트 판은 가장자리 감시를 켜지 않음(두 번 전환 방지).

## 지금 판과 다른 점

1. 「드래그 중 전환 안 함」 토글이 실제 판정에 쓰임(트레이·바탕화면 워커). 버튼 상태는 실제 버튼(GetAsyncKeyState)으로 읽음.
2. 프로그램을 지우면 서비스·`%ProgramData%\LogiFlowRs`(워커 사본·logon-edges)·예약 작업·방화벽 규칙까지 정리(필요할 때만 UAC). 사용자 설정은 보존.
3. 클립보드 암호문구를 평문 대신 Windows DPAPI(사용자 범위)로 저장(`clipSecretDpapi=`). C# 판 설정을 이어받을 때 옮겨 담고, C# 원본 평문은 건드리지 않음.
4. 화면: 메뉴는 Windows 기본 메뉴 모양, 글자는 egui 렌더러, 배율 화면에서 선명, 설정 창은 열 때 따로 뜨고 닫으면 끝남(첫 표시 지연은 실측 대상), 라이선스 창 구성 목록은 러스트 구성요소로.
5. 설정 창이 상태 갱신(스캔 완료 등) 때 맨 위로 돌아가지 않음(화면을 바꿀 때만 맨 위로).

## 회장님 PC 확인 목록

계획서 `docs/RUST_PORT_PLAN.md` 10절.

# LogiFlow 러스트 이전 계획서 — 1단계 Windows · 2단계 Mac·Linux

작성 2026-11-13 · 브랜치 `rust-port` · 기준 코드 `main` 4999df9(0.15.0). 구현 전 main 보고용.

## 0. 정의

1. 지금 판 — `main` 의 0.15.0. Windows 는 C#(.NET Framework 4.6.2 · WinForms), Mac 은 Swift(SwiftUI), Linux 는 C#(.NET 8 CLI).
2. 러스트 판 — 이 계획으로 새로 만드는 판. 새 폴더 `rust/`, 브랜치 `rust-port`. 지금 판 코드는 손대지 않음.
3. 상주 프로세스 — 로그인 동안 늘 떠 있는 프로세스. 트레이, 서비스, 서비스가 띄우는 워커 2종.
4. 쓰기 — 로지텍 기기 상태를 바꾸는 HID++ 명령. 조회(읽기) 명령과 구분.
5. 1:1 — 같은 입력에 같은 동작·같은 화면·같은 파일. 다르면 7절 「바뀌는 곳」에 올림.
6. 실기 대기 — 로지텍 기기·모니터 가장자리·로그온 화면이 필요해 회장님 PC 에서만 확인 가능한 항목.

## 1. 지시 원문과 목표

1. 회장님 원문(main 중계): "일단 위에 2개부터 한번 옮겨보자." (대상 2개 = LogiFlow · KM Bridge, 이 문서는 LogiFlow 분)
2. main 추천 이유(main 지시문): Windows(C#) · Mac(Swift) · Linux(CLI) 세 갈래 코드를 러스트 하나로 합침, 마우스 가장자리 감지처럼 늘 도는 상주 프로그램이 가벼워짐.
3. 1단계 목표 — Windows 판을 러스트로, 지금 판과 같은 동작·같은 화면. 새 디자인 없음.
4. 2단계 목표 — 같은 코어로 Mac · Linux 합치기.
5. 안전 규칙 그대로 — <u>**쓰기는 0x1814 func1 setHost 하나뿐, (a)0x1814 지원 기기 (b)채널 0~hostCount-1 (c)명시 트리거(가장자리·트레이·버튼) 세 조건에서만**</u>.

## 2. 지금 판 기능 목록 (코드 실측)

### 2.1 Windows — 실행 경로

한 실행 파일(`LogiFlow.exe`)이 인자로 역할을 나눔. 인자 없으면 트레이.

| 인자 | 실행 주체 | 하는 일 |
|---|---|---|
| (없음) | 사용자 | 트레이 상주. 단일 인스턴스 뮤텍스 `LogiFlow-A2E1C4D6-8B3F-4A91-9C72-1E5D7F0A2B34` |
| `--service` | SCM(LocalSystem·세션0) | 서비스 `LogiFlowLogonSwitch`. 2초마다 활성 콘솔 세션 확인, 워커 2종 감독 |
| `--logon-switch` | 서비스가 winlogon 토큰으로 `winsta0\winlogon` 에 투입 | 로그온·잠금 화면 전환(폴링) |
| `--desktop-switch` | 서비스가 `winsta0\default` 에 투입 | 바탕화면 전환(raw 입력) + glow |
| `--install-prelogon=N` / `--uninstall-prelogon` | UAC 상승 자기 자신 | 서비스 설치(워커 사본을 `%ProgramData%\LogiFlow\worker` 로 복사)·삭제, 옛 예약 작업 `LogiFlow-PreLogon` 제거 |
| `--prelogon-pull=N` | (옛 경로) | 부팅 시 이 PC 채널로 당기기. 지금 UI 에서 안 쓰나 코드 유지 |
| `--desktop-probe=N` · `--desktop-probe-launch` · `--logon-switch-launch` | SYSTEM | 로그온 화면 접근 테스트(읽기 전용)·세션0 런처 |
| `--install-probe` / `--uninstall-probe` | UAC 상승 | 예약 작업 `LogiFlow-DesktopProbe`(ONSTART SYSTEM HIGHEST) 설치·즉시 실행·삭제 |
| `--allow-firewall=P` | UAC 상승 | `netsh` 인바운드 규칙 `LogiFlow Clipboard` (TCP P) |

### 2.2 Windows — 기기 열거와 HID++

1. 열거 — SetupAPI 로 HID 인터페이스 전부, 경로의 `vid_046d` 로 1차 거름. 공유 모드(읽기·쓰기 공유)로 열어 속성·caps·제품 문자열. caps 못 읽은 인터페이스도 진단용으로 남김.
2. 후보 — caps 읽힘 + (usage 0x0202 long 또는 0x0201 short 또는 usagePage≥0xFF00 이고 출력 리포트 ≥7바이트).
3. 프로브 — deviceIndex 0xFF·1~6 순서, ping(IRoot func1, 표식 0x5A) 600ms 2회. 응답(오류 응답 포함)이면 기기 있음.
4. 조회 — IRoot getFeature(0x1814) 800ms → 0 이면 미지원. getHostInfo → 호스트 개수·현재 채널. 이름은 0x0005(getCount → getName 조각, 최대 16회·64자) → 실패 시 제품 문자열 → 「(이름 미상)」.
5. 중복 제거 — 키 = 이름 + deviceIndex + 연결 방식, 0x1814 정보 있는 쪽 유지.
6. 프레임 — 출력 리포트 길이 ≥20 이면 long(0x11·20바이트) 아니면 short(0x10·7바이트). software id 0x0A. HID++2.0 오류(0xFF)·1.0 오류(0x8F) 구분, 그 밖 비동기 알림은 버리고 계속 읽음.
7. 송수신 — 오버랩드 WriteFile/ReadFile + 타임아웃, 시간 초과 때 CancelIo 후 완료 대기(버퍼 안전).
8. 지문 — 로지텍 인터페이스 경로 정렬 연결 문자열. 기기 변화 판단용(HID++ 송신 없음).

### 2.3 Windows — 전환(쓰기)

1. setHost — 응답 안 기다림(기기가 바로 끊김). 쓰기 400ms, 짧게 120ms 읽어 버림(묶음 전송은 0ms).
2. SwitchOne / SwitchAll — 설정 창 기기별 전환·트레이 빠른 전환·트레이 가장자리 전환. 재시도 없음.
3. SwitchAllVerbose — 로그온 워커. 기기마다 ping 200ms → setHost → getHostInfo 300ms 확인, 최대 3회·150ms 간격. 응답 없음 = 넘어감으로 판정.
4. SwitchAllBurst — 바탕화면 워커. 전 기기 setHost 를 대기 없이 연달아 쓰고 확인은 그 뒤, 실패 기기만 ping 으로 깨워 재시도. 첫 쓰기 성공 순간 glow 최고점 신호.
5. 가드 — 네 경로 모두 (a)(b) 검사 후 HidppDevice.SetHost 한 곳으로만 씀.

### 2.4 Windows — 가장자리 감지(변 밀기)

1. 판정기 — 띠 2px, dwell 기본 330ms(설정 50~3000, 메뉴 0.2·0.33·0.5·0.8·1초), 바깥 밀기 누적 10px, 버튼 눌림이면 리셋, 발동 뒤 변에서 16px 떨어져야 재무장.
2. 트레이 감시 — 숨은 메시지 창 + raw 입력(RIDEV_INPUTSINK, 상대 이동만, 절대 좌표 무시). 커서 모니터(이름 일치)의 배치만 먹임. 400ms 마다 변 8px 안 샘플 로그.
3. 켜는 조건 — 「로그온 전에도 실행」 꺼짐 + 가장자리 자동 전환 켜짐 + 배치 있음 + 전환 가능 기기 있음. 서비스가 켜져 있으면 트레이는 감시 안 함(중복 방지).
4. 좌표계 — 프로세스가 DPI 미인식이라 GetCursorPos 와 모니터 경계가 같은 논리 픽셀.
5. 로그온 워커 — 50ms 폴링, OpenInputDesktop → 데스크톱 이름, 「보안 데스크톱(이름≠Default) 또는 잠금 플래그」 일 때만 동작(ARM 잠금 = Default 대응). 밀기 요건 0px(커서가 막혀 밀기를 못 읽음), 전환 뒤 1.2초 쉼. 켜진 동안만 백그라운드 스캔(기기 있으면 5초·없으면 1.5초). 4초마다 설정 다시 읽어 디버그 반영. 콘솔 세션 이탈 3초 뒤 스스로 종료.
6. 바탕화면 워커 — 트레이와 같은 raw 판정기 재사용. 백그라운드 스캔은 기기 있으면 60초·없으면 1.5초, 전환 중·전환 뒤 10초 금지. 전환은 별도 스레드, 진행 중 다시 밀면 건너뜀(glow 흐림), 전부 실패면 재무장. 세션 바뀌면 3초 뒤 종료.

### 2.5 Windows — glow

1. 창 — 레이어드·클릭 통과·비활성·최상위·작업 표시줄 없음, UpdateLayeredWindow per-pixel alpha, 16ms 타이머.
2. 단계 — dwell 동안 최대 80% · 전환 요청 뒤 90% 로 대기 · setHost 실제 전달 순간 흰빛 최고점 3틱(약 50ms) 뒤 숨김 · 1.5초 안에 전달 신호 없거나 실패면 최고점 없이 흐려짐.
3. 움직임 — 오를 때 0.5·내릴 때 0.3 비율로 따라감, 0.015 미만이면 숨김. 진행 0 이면 안 그림. 전환 직후 재무장 전엔 안 켬.
4. 그림 — 폭 90px 띠. 안쪽 투명 → 변쪽 강조색 그라데이션(최대 알파 190, 최고점 235) · 두꺼워지는 코어(2+32×진행, 최고점 40) · 변 선 2.5px. 변 방향대로 회전.

### 2.6 Windows — 트레이

1. 아이콘 — 내장 `logiflow.ico`. 두 번 클릭 = 설정 창.
2. 우클릭 메뉴(열 때마다 새로 만듦) — 「현재 채널: N · 별칭」(비활성) · 구분선 · 현재 외 채널 「채널 N · 별칭」(없으면 「전환할 채널 없음」) · 구분선 · 「설정 열기」 · 「새로 고침(스캔)」 · 구분선 · 「종료」.
3. 툴팁 — 「LogiFlow — 채널 N · 별칭」 또는 「LogiFlow — 스캔 중...」(최대 62자).
4. 풍선 알림 — 「클립보드 받음: 텍스트 n자/이미지」 · 암호문구 불일치 경고.
5. 기기 변화 — WM_DEVICECHANGE(숨은 최상위 창), 1.2초 디바운스 + 0.6초 안정화 뒤 지문이 다를 때만 재스캔.
6. 스캔 끝 — 캐시 저장, 이 PC 채널을 `myHost` 로 기억, 감시 재구성.
7. 세션 잠금/해제 — `%ProgramData%\LogiFlow\locked.flag` 만들기/지우기(워커가 읽음).

### 2.7 Windows — 설정 창(승인 시안 v2 구현)

1. 창 — 820×660(최소 760×600), 화면 가운데. 닫기 = 숨김. 사이드바 182px(마크·「LogiFlow」 · 전환 관리·설정·진단 · 「LogiFlow 실행 중」 · 좌하단 버전). 콘텐츠 열 폭 상한 560, 테마 세로 스크롤바.
2. 글꼴 — Segoe UI, 기준 pt + 글자 크기 단계(작게 −1 · 보통 +1 기본 · 크게 +3), 최소 9.5pt.
3. 테마 — 시스템(레지스트리 `AppsUseLightTheme`)·라이트·다크. 색 토큰 `Ui/Theme.cs`(brief v3).
4. 전환 관리 — 모니터 배치 캔버스(300px, 22px 격자, 「모니터 N」·「주 모니터」, 채널 박스 = 모니터 평균 크기, 30px 안이면 변에 붙음, 붙으면 「접촉 · 자동」 파랑 실선·떨어지면 「분리 · 수동」 회색 점선, 미배치는 아래 한 줄, 끌기는 칸 안으로 제한, 넘치면 스크롤) · 채널 별칭 칩(같은 폭·최소 110, 연필 → 입력 상자) · 등록된 기기(아이콘·이름·채널 배지·[전환] → 채널 메뉴 → 확인 상자) · 가장자리 자동 전환 토글 · 대기 메뉴 · 드래그 중 전환 안 함 · 배치 초기화.
5. 설정 — 모양(테마 3단·글자 크기 3단) · 실행(Windows 시작 시 실행 · 로그온 전에도 실행[고급]) · 전환·진단(가장자리 자동 전환 사용 · 진단 로그 남기기 · 상세 디버그 로그[고급]) · 클립보드 공유(사용 · 상대 PC 주소 · 암호문구 · 이 PC 주소 · 연결 테스트 · 테스트 보내기 · 방화벽 허용) · 정보(오픈소스 라이선스 보기).
6. 로그온 전에도 실행 — 배치 없으면 안내 후 중단. 켜기/끄기 모두 진행 모달(단계 문구 → 같은 창에서 결과 + [확인]). 끄기 뒤 서비스 존재 재조회로 실제 상태 반영. 설치 여부 = 서비스 존재(백그라운드 1회 조회 후 캐시).
7. 진단 — 모두 보기 토글 · 새로 고침(스캔 중엔 「스캔 중…」 비활성) · 로그 폴더 열기 · 표(기기 30% · VID:PID 18% · 연결 22% · 0x1814 인덱스 18% · 채널 12%) · 안내 줄 · 로그온 화면 접근 테스트(테스트 설치·실행 / 로그 열기 / 제거 + 설명).
8. 대화상자 — 입력 상자(340×130, Windows 기본 입력란·버튼) · 진행 모달(440×244) · 라이선스(560×460, MIT 전문 + 구성 목록) · 메시지 상자(Windows 기본).

### 2.8 Windows — 클립보드 공유

1. 감지 — AddClipboardFormatListener. 텍스트(UTF-8, 종류 0) 우선, 없으면 이미지(PNG, 종류 1, 8MB 상한).
2. 전송 — 상대 IP:포트로 TCP 직접 연결(3초), 프레임 = 길이 4바이트(little-endian) + IV 16 + AES-256-CBC(PKCS7) + HMAC-SHA256 32.
3. 키 — 암호문구 NFKC 정규화 → PBKDF2-HMAC-SHA1(소금 `LogiFlow-Clip-v1`, 20000회) 64바이트 → 앞 32 AES · 뒤 32 HMAC.
4. 수신 — 모든 주소 포트 수신(기본 48737), 인증 실패면 경고 풍선. 적용 뒤 1.2초 동안·같은 해시는 되돌려 보내지 않음(핑퐁 방지).
5. 상대 주소·암호문구 입력도 NFKC(한글 IME 전각 대응).

### 2.9 Windows — 설정·캐시·로그·설치

1. 설정 `%APPDATA%\LogiFlow\config.txt`(key=value) — edgeEnabled · dwellMs · autostart · dragGuard · diagLog · preLogon · myHost · fontOffset · clipEnabled · clipHost · clipPort · clipSecret · debug · theme · alias(채널\t이름) · attach(채널\t모니터 이름\t변).
2. 기기 캐시 `%APPDATA%\LogiFlow\devices.txt` — 13칸 탭 구분 v1. 시작 때 캐시로 바로 표시, 비었을 때만 첫 스캔.
3. 머신 공유 `%ProgramData%\LogiFlow\logon-edges.txt` — dwellMs · myHost · debug · edge(모니터 이름·순번·변·채널). 사용자 앱이 쓰고 워커가 읽음.
4. 로그 — `<실행 파일 폴더>\logs\logiflow-diag.log`(TX/RX hex, 시각 `yyyy-MM-dd HH:mm:ss.fff`, 끄면 메모리만), `%ProgramData%\LogiFlow\desktop-probe.log`(서비스·워커·설치 결과).
5. 자동 시작 — HKCU Run `LogiFlow` + 예약 작업 `LogiFlow-Tray`(ONLOGON · LIMITED) 둘 다.
6. 설치 파일 — Inno Setup, 사용자 범위 `%LOCALAPPDATA%\Programs\LogiFlow`, 한국어, 바탕화면 아이콘 선택, HKCU Run 등록, 설치·제거 전 `taskkill /IM LogiFlow.exe`, 설치 뒤 실행. 산출 setup.exe · zip · SHA256SUMS.
7. 판별 기록 — `docs/RELEASE_NOTES_v<판>.md`, 커밋 `ship:` → `release-all.yml` 이 세 OS 를 한 릴리스(Latest)로.

### 2.10 Mac · Linux (2단계 대상)

1. Mac(Swift) — 760×620 단일 창(채널 버튼 · 채널마다 화면·변 고르기 · 기기 목록) · 메뉴 막대 ⇄(채널 · 변으로 밀어 넘기기 · 기기 다시 찾기 · 창 열기 · 종료) · NSEvent 전역 마우스 감시 · glow · dwell 500ms 고정 · 설정 UserDefaults(edgeEnabled · edgeBindings, 화면은 CGDirectDisplayID) · 로그 `~/Library/Logs/LogiFlow` · 미서명 dmg.
2. Linux(C# CLI) — `logiflow list` · `switch N` · `N` · `--rescan` · `--debug` · `selftest` · `--version`, hidraw 직접, 캐시 `$XDG_CACHE_HOME/.../devices.tsv`, udev 규칙(uaccess·plugdev), install.sh · x64·arm64 tar.gz.

### 2.11 코드 읽다 찾은 기존 동작 특이점 (1:1 유지 제안 · 고칠지는 main 판단)

1. 「드래그 중 전환 안 함」 토글 — 저장만 되고 판정에 안 쓰임. 꺼도 드래그 중엔 늘 전환 안 함(EdgeWatcher 가 버튼 상태로 항상 막음).
2. 프로그램 제거 — 토글을 끄지 않고 지우면 서비스와 `%ProgramData%\LogiFlow\worker` 사본이 남아 계속 돎.
3. 클립보드 암호문구 — config.txt 에 평문 저장.

## 3. 러스트 구성

### 3.1 폴더·크레이트 구조

```
rust/
  Cargo.toml                 워크스페이스
  crates/logiflow-core/      OS 무관 — HID++ 프레임·스캔·전환 안전·가장자리 판정·배치 스냅·
                             설정/캐시/logon-edges 형식·클립보드 암호·glow 계산·테마 토큰
  crates/logiflow-win/       Windows 전용 — HID 전송·모니터·raw 입력·glow 창·트레이·자동 시작·
                             클립보드·서비스·워커·프로브·방화벽·IPC
  crates/logiflow-ui/        설정 창(egui) — 3화면·캔버스·컨트롤·아이콘
  app/                       실행 파일 LogiFlowRs.exe — 인자 분기(2.1 표와 같은 인자)
  installer/LogiFlowRs.iss   Inno Setup
```

1. 코어는 `HidTransport`(열기·쓰기·읽기) 같은 trait 만 보고 동작 — 2단계 Mac·Linux 는 전송·입력·모니터 구현만 더함.
2. 안전 규칙을 구조로 강제 — 코어 밖으로는 조회 함수와 `HostSwitcher` 만 공개, 원시 쓰기는 비공개. 시험용 가짜 전송이 모든 쓰기를 기록해 「setHost 외 쓰기 0건」을 자동 확인.

### 3.2 크레이트 선택

| 용도 | 선택 | 고른 이유 | 안 고른 후보 |
|---|---|---|---|
| Win32 전반 | `windows`(windows-rs, Microsoft) | HID·SetupAPI·raw 입력·GDI·토큰·데스크톱·SCM·셸 API 를 한 크레이트로, 지금 P/Invoke 를 줄 단위로 옮김 | `windows-sys`(래핑 없음, 실수 여지↑) |
| Windows HID 전송 | windows-rs 로 지금 코드 직접 이식 | 실기 검증된 열기 공유 모드·오버랩드 타임아웃·CancelIo 순서·출력 리포트 길이로 long/short 판정을 그대로 유지 | `hidapi`(C 라이브러리 빌드 추가, 출력 리포트 길이 직접 안 줌 — 2단계 Mac 에서 재검토) |
| 서비스 | `windows-service` | SCM 디스패처·세션 변경 알림·설치/삭제, 널리 쓰임 | 직접 구현 |
| 설정 창 | `eframe` + `egui`(wgpu 백엔드) | 3.3 참고 | 3.3 표 |
| 트레이 | 직접(Shell_NotifyIconW) | 풍선 알림·탐색기 재시작(TaskbarCreated) 대응을 지금 그대로 | `tray-icon`(풍선 알림 없음) |
| 메뉴·입력 상자·메시지 상자 | 직접(TrackPopupMenu · Win32 대화상자 · MessageBoxW) | 지금 입력 상자·메시지 상자가 원래 Windows 기본 컨트롤, 한글 IME 안정 | egui 자체 위젯 |
| 클립보드 읽기·쓰기 | `arboard` + `png` | 텍스트·이미지(RGBA↔PNG), 2단계 Mac·Linux 공용. 변화 감지는 AddClipboardFormatListener 직접 | 직접 CF_DIB 처리 |
| 암호 | `aes` · `cbc` · `hmac` · `sha2` · `sha1` · `pbkdf2` · `subtle` · `getrandom`(RustCrypto) | 지금 판과 바이트 호환(같은 알고리즘·반복 수·소금) | `ring`(PBKDF2-SHA1·CBC 조합 불편) |
| 유니코드 정규화 | `unicode-normalization` | NFKC 동일 | — |
| 아이콘·버전·매니페스트 | `embed-resource`(.rc) | 지금 ico·버전 정보·asInvoker 매니페스트 그대로 | `winresource` |
| 시각 | `chrono` | 로그 시각 형식 동일 | — |
| 화면 스냅샷 시험 | `egui_kittest` | CI 에서 3화면 × 라이트·다크 PNG 렌더 → 대조 자료 | — |

설정·캐시 파일은 직렬화 크레이트 없이 지금 key=value·탭 형식을 그대로 읽고 씀(교체 때 파일 그대로 이어받기 위해).

### 3.3 화면 도구 후보와 고른 이유

| 후보 | 지금 모양 재현 | Mac·Linux 공용 | 라이선스 | 상주 부담 | 판단 |
|---|---|---|---|---|---|
| egui/eframe | 지금 코드가 전부 절대 좌표 + 직접 그리기라 같은 좌표·같은 도형으로 옮기기 쉬움 | 됨 | MIT/Apache | 설정 창 별도 프로세스로 0 | 선택 |
| Win32 + GDI 직접 | 가장 같음(같은 렌더러) | 안 됨(Mac·Linux 화면 따로) | — | 가장 작음 | 2단계 목표와 어긋나 보류 |
| Slint | 좋음 | 됨 | GPLv3 또는 무료판은 「Made with Slint」 표기 의무 → 화면 추가 | 보통 | 제외 |
| iced | 위젯 직접 구현 분량 큼 | 됨 | MIT | 보통 | 제외 |
| WebView2(Tauri) | 시안 HTML 재사용 가능 | 됨 | MIT/Apache | 무거움(브라우저 엔진) | 「가벼워짐」과 반대라 제외 |

1. egui 그래픽 백엔드는 wgpu(Windows = DirectX 12, 안 되면 WARP 소프트웨어) — ARM Windows 에서 OpenGL 드라이버가 없을 수 있어 glow(OpenGL) 백엔드 대신 고름.
2. 글꼴은 `C:\Windows\Fonts` 의 Segoe UI(보통·굵게) + 맑은 고딕(한글 대체)을 실행 때 읽음 — 지금 WinForms 의 글꼴 연결과 같은 조합.

### 3.4 상주 방식

| 프로세스 | 언제 | 하는 일 | 화면 도구 |
|---|---|---|---|
| `LogiFlowRs.exe`(트레이) | 로그인 ~ 종료 | 트레이·스캔·전환·가장자리 감시(서비스 없을 때)·glow·클립보드·잠금 플래그·기기 변화 | 없음(Win32 메시지 루프) |
| `LogiFlowRs.exe --ui` | 설정 창 열 때만, 닫으면 종료 | 3화면·대화상자 | egui |
| `--service` | 부팅 ~ | 워커 감독 2초 | 없음 |
| `--logon-switch` · `--desktop-switch` | 서비스가 투입 | 2.4 의 5·6 그대로 | 없음 |

1. 트레이가 상태(설정·기기 목록·HID)의 단일 주인 — 설정 창은 보기·편집 요청만 보내고(숨은 창 WM_COPYDATA), 트레이가 저장·적용 후 갱신 알림. 두 프로세스가 동시에 HID 를 쓰거나 설정 파일을 덮어쓰는 일 없음.
2. 상주 프로세스는 지금처럼 DPI 미인식 유지 → 커서·모니터 좌표 계산 1:1. 설정 창만 배율 인식(선명).
3. 콘솔 창 없음(`windows_subsystem = "windows"`).
4. 가벼워짐 효과는 미실측 — 회장님 PC 작업 관리자로 지금 판과 비교하기 전엔 수치로 쓰지 않음.

## 4. 같은 동작 확인 목록 (지금 판과 1:1)

확인 방법: 자동 = 로컬·CI 자동 시험 · CI = Windows 러너 실행 확인 · 실기 = 회장님 PC(실기 대기).

| 번호 | 항목 | 기준(지금 판) | 확인 |
|---|---|---|---|
| 4.1 | HID++ 프레임 | long/short 선택·software id 0x0A·오류 2종 구분·알림 건너뜀 | 자동(바이트 대조) |
| 4.2 | 스캔 순서·시간 | 0xFF·1~6, ping 600ms×2, 조회 800ms, 이름 16조각 | 자동(가짜 기기) |
| 4.3 | 중복 제거·이름 대체 | 2.2 의 4·5 | 자동 |
| 4.4 | 안전 규칙 | setHost 외 쓰기 0건, (a)(b) 위반 시 송신 0 | 자동(모든 쓰기 기록) |
| 4.5 | 전환 4경로 | 재시도 횟수·대기·확인 판정(응답 없음 = 넘어감) | 자동 + 실기 |
| 4.6 | 가장자리 판정 | 띠 2·dwell·밀기 10·재무장 16·버튼 리셋 | 자동(입력 대본) + 실기 |
| 4.7 | 트레이 감시 켜는 조건 | 2.4 의 3 | 자동 |
| 4.8 | glow 단계·그림 | 2.5 수치, 픽셀 계산 | 자동(픽셀 대조) + 실기(눈) |
| 4.9 | 설정·캐시·logon-edges 형식 | 지금 판 파일을 읽고 같은 내용으로 씀(키 순서·범위 검사) | 자동(왕복) |
| 4.10 | 클립보드 프레임 | C# 판과 상호 복호(시험 벡터는 CI Windows 러너의 .NET 으로 생성) | 자동 + 실기(C#↔Rust 양방향) |
| 4.11 | 트레이 메뉴·툴팁·풍선 | 2.6 문구·순서 | CI(실행) + 실기 |
| 4.12 | 기기 변화 재스캔 | 디바운스 1.2초·안정화 0.6초·지문 비교 | 실기 |
| 4.13 | 설정 창 3화면 | 2.7 크기·문구·배치·토큰 | CI 스냅샷(라이트·다크·글자 3단) + 실기 |
| 4.14 | 캔버스 조작 | 스냅 30·미배치 줄·끌기 제한·초기화 | 자동(배치 계산) + 실기 |
| 4.15 | 로그온 전에도 실행 | 사본 복사·서비스 등록/삭제·모달 단계·상태 재조회 | 실기 |
| 4.16 | 서비스·워커 | 감독 2초·토큰·데스크톱·세션 이탈 종료·잠금 플래그 | 실기(콜드부팅·잠금·로그온·사용자 전환·ARM) |
| 4.17 | 로그온 화면 접근 테스트 | 예약 작업·런처·읽기 전용 | 실기 |
| 4.18 | 자동 시작 | Run 값 + ONLOGON 작업 | CI(등록 확인) + 실기 |
| 4.19 | 로그 형식·경로 | 2.9 의 4 | 자동 + 실기 |
| 4.20 | 방화벽 허용 | netsh 규칙 | 실기 |

## 5. 빌드

1. 로컬(리눅스) — 서버 `/usr/bin/cargo` = 1.75.0(배포판 패키지, Windows 표준 라이브러리 없음). `logiflow-core` 만 따로 시험(의존 크레이트를 1.75 호환 판으로 고정). Windows 코드는 로컬 컴파일 불가.
2. 로컬 확장(선택 · 승인 필요) — rustup 으로 최신 안정판 1.100.0 + `x86_64-pc-windows-msvc` 표준 라이브러리를 사용자 홈에 설치하면 Windows 코드도 로컬 형식 검사(`cargo check`·`clippy`, 링크 없음) 가능. 다운로드 실측 약 155MB(rustc 85 · cargo 11 · 리눅스 std 31 · Windows std 23 · clippy 5), 설치 뒤 디스크 0.8~1GB 추정(설치 전 실측 불가). 디스크 여유 517GB. 안 하면 Windows 확인은 CI 만.
3. CI — 새 워크플로 `.github/workflows/rust-windows.yml`(브랜치 `rust-port` 의 `rust/**` 변경 때). windows-latest · 최신 안정판 · 대상 x86_64·aarch64 MSVC. 순서: fmt → clippy → 시험 → 두 대상 릴리스 빌드 → 화면 스냅샷 PNG → 설치 파일(x64·arm64 한 파일) · zip · SHA256SUMS → 아티팩트. 릴리스 발행 없음.
4. 리눅스 CI 잡 — ubuntu-latest 에서 코어 시험(2단계 준비).
5. 기존 워크플로 4개(windows·mac·linux·release-all)는 `main`·`logiflow-plus` 에서만 돌아 영향 없음.

## 6. 판 번호·배포·나란히 설치·설정 이어받기

1. 판 번호 — 시험판 `0.16.0-rs.N`(N=1,2,…) 제안. 실행 파일 버전 0.16.0.N. 사이드바 좌하단 표기로 구분.
2. 배포 — 시험 기간엔 `rust-port` CI 아티팩트만(GitHub 로그인 뒤 Actions 실행 페이지에서 받기). 기본 브랜치 `main` 에 합치기·릴리스는 회장님 PC 확인 뒤 main 판단.
3. 나란히 설치 — 모든 이름을 C# 판과 다르게.

| 항목 | C# 판 | 러스트 시험판 |
|---|---|---|
| 설치 폴더 | `%LOCALAPPDATA%\Programs\LogiFlow` | `...\Programs\LogiFlowRs` |
| 실행 파일 | `LogiFlow.exe` | `LogiFlowRs.exe`(서로의 설치 파일 taskkill 에 안 걸림) |
| 설치 AppId | `{B1F3C2A4-…}` | 새 GUID |
| 시작 메뉴 | LogiFlow | LogiFlow (러스트 시험판) |
| 뮤텍스 | `LogiFlow-A2E1…` | 새 이름 |
| 사용자 설정 | `%APPDATA%\LogiFlow` | `%APPDATA%\LogiFlowRs` |
| 머신 공유 | `%ProgramData%\LogiFlow` | `%ProgramData%\LogiFlowRs` |
| 서비스 | `LogiFlowLogonSwitch` | `LogiFlowRsLogonSwitch` |
| 예약 작업 | `LogiFlow-Tray` · `LogiFlow-DesktopProbe` | `LogiFlowRs-Tray` · `LogiFlowRs-DesktopProbe` |
| Run 값 · 방화벽 규칙 | `LogiFlow` · `LogiFlow Clipboard` | `LogiFlowRs` · `LogiFlowRs Clipboard` |

4. 중복 전환 막기 — C# 판 코드는 손대지 않으므로 러스트 쪽에서만 막음.
   1. 러스트 트레이 시작 때 C# 트레이 뮤텍스가 있거나 C# 서비스가 등록돼 있으면 가장자리 감시를 켜지 않음(로그 한 줄, 화면 변화 없음).
   2. C# 서비스가 있을 때 러스트 「로그온 전에도 실행」 켜기 → 안내 상자 후 중단(새 문구 1건, 7절).
   3. 시험 절차 — C# 트레이 종료·C# 자동 시작 끔 상태에서 러스트 판 시험. 화면 비교만 할 땐 둘 다 띄워도 됨(위 1 로 전환은 C# 판만).
5. 설정 이어받기 — 첫 실행 때 `%APPDATA%\LogiFlowRs\config.txt` 가 없고 `%APPDATA%\LogiFlow\config.txt` 가 있으면 config.txt·devices.txt 복사(형식 같아 그대로 읽음). C# 원본은 읽기만.
   1. 복사 때 `preLogon=0`(러스트 서비스 아직 없음) · `autostart=0`(두 트레이 동시 자동 시작 방지) — 시험판 한정.
   2. 러스트 시험판 설치 파일은 HKCU Run 을 등록하지 않음(C# 설치 파일과 다른 점, 시험판 한정).
6. 최종 교체(확인 뒤 main 판단) — 이름을 C# 판과 같게(LogiFlow.exe · 같은 AppId · 같은 폴더) 돌리면 기존 설치 위 덮어쓰기, 설정 파일 그대로 읽음. 서비스 워커 사본 갱신 방법은 그때 확정.

## 7. 화면에서 바뀌는 곳 (main 보고 대상)

1. 메뉴 모양 — 트레이 우클릭·기기 [전환]·[대기] 메뉴가 WinForms 그림에서 Windows 기본 메뉴로. 항목·순서·문구 같음.
2. 글자 그리기 — GDI(ClearType) 대신 egui 렌더러. 같은 글꼴 파일이나 획 두께·자간 미세 차이 가능.
3. 선명도 — 지금 앱은 DPI 미인식이라 125%·150% 배율 화면에서 흐릿하게 늘어남 → 러스트 설정 창은 선명(크기는 같게 맞춤).
4. 설정 창 첫 표시 — 지금은 미리 만든 창을 보여 줘 즉시 → 열 때 프로세스를 띄워 지연(0.3~1초 추정, 실측 대상). 닫으면 종료(지금은 숨김).
5. 스크롤바 — 지금 테마 스크롤바 색·폭에 맞추나 끌기 느낌 미세 차이 가능.
6. 라이선스 창 「구성 / 사용 기술」 문단 — .NET Framework·WinForms 대신 실제 쓰인 러스트 구성요소와 라이선스 목록(오픈소스 고지 의무).
7. 새 안내 상자 1건 — 6절 4의 2(시험 기간 한정).
8. 버전 표기 — v0.16.0-rs.N.

## 8. 위험

1. 서비스·워커 — SYSTEM 토큰 복제·winlogon 데스크톱 투입·세션 경합. 가장 어려운 이식이고 회장님 PC 에서만 확인(콜드부팅·잠금·로그온·사용자 전환·화면보호기).
2. 회사 관리 PC — 서명 없는 새 실행 파일이 보안 정책(앱 제어·백신)에 막힐 가능성. 지금 판이 설치·실행되니 사용자 범위 설치는 허용으로 보이나 러스트 실행 파일 판정은 미확인. 막히면 우회하지 않고 보고.
3. ARM Windows — arm64 네이티브 빌드 필요. CI 는 빌드만, 실행 확인은 실기 대기.
4. 화면 1:1 — 글자 렌더러 차이로 자동 크기 라벨 위치가 몇 px 어긋날 수 있음. CI 스냅샷과 회장님 눈으로 확인.
5. 클립보드 상호 호환 — 한쪽 C#·한쪽 러스트일 때 반드시 실기 확인.
6. HID 오버랩드 취소 — 버퍼 수명. 지금 순서(CancelIo → 완료 대기) 그대로.
7. 동시 실행 — 6절 4 로 완화, 시험 절차로 차단.
8. 개발 왕복 — 로컬 Windows 컴파일 불가 시 CI 1회 3~6분 추정.
9. egui 한글 입력 — 입력 칸은 Windows 기본 대화상자로 피함.
10. 가벼워짐 — 미실측. 비교 전 효과 단정 금지.

## 9. 단계

1. 1a 코어 + 로컬 시험(4.1~4.4·4.6·4.8~4.10 자동분).
2. 1b Windows 전송·모니터·raw 입력·glow 창·트레이·자동 시작·클립보드 → CI 빌드.
3. 1c 서비스·워커 2종·프로브·로그온 전 설치/삭제·방화벽 → CI 빌드.
4. 1d 설정 창(egui) 3화면·대화상자·IPC → CI 스냅샷.
5. 1e 설치 파일(x64·arm64)·아티팩트·코드 리뷰·처음 쓰는 사람 점검(ux-first-use-reviewer).
6. 회장님 PC 확인 1회(10절) → main 판단(교체·릴리스).
7. 2단계 — Mac(IOKit HID · NSEvent · NSStatusItem · LaunchAgent, `objc2` 계열 · `hidapi` 재검토), Linux(hidraw 직접 · udev · 같은 CLI 명령). 코어·설정 창 공용. 지금 Mac 화면은 단일 창·피커 방식이라 Windows 3화면을 쓰면 Mac 화면이 바뀜 — 2단계 착수 때 결정.

## 10. 회장님 PC 확인 목록 (초안 · 실기 대기)

1. 설치 — 설치 파일 실행·회사 정책 통과·C# 판과 나란히.
2. 이어받기 — 별칭·배치·대기·테마·글자 크기·클립보드 값 그대로.
3. 화면 — 3화면 라이트·다크·글자 3단, 창 크기 조절·스크롤, C# 판과 나란히 비교.
4. 진단 — 기기·0x1814 인덱스·채널 값이 C# 판과 같음.
5. 수동 전환 — 트레이 메뉴·기기 [전환].
6. 가장자리(서비스 끈 상태) — 밀면 넘어감 · 가장자리에 두기만 하면 안 넘어감 · 드래그 중 안 넘어감 · glow 80% → 90% → 흰빛 → 사라짐.
7. 로그온 전에도 실행 — 켜기·끄기(UAC·모달 단계).
8. 서비스 켠 상태 — 콜드부팅 로그온 화면 · 잠금 화면 · 로그인 직후 바탕화면 · 사용자 전환 · 중복 전환 없음.
9. ARM 기기 — 8 과 같은 항목(잠금 = Default 데스크톱).
10. 클립보드 — C#↔러스트 텍스트·이미지 양방향, 암호 다르면 경고.
11. 기기 꽂기·뽑기 → 재스캔.
12. 가벼움 실측 — 작업 관리자 메모리·CPU(트레이·워커), C# 판 대비.
13. 제거·재설치.

## 11. main 결정 필요

1. 화면 도구 egui + 설정 창 별도 프로세스.
2. 7절 바뀌는 곳 1~8 수용(특히 메뉴 모양·글자 렌더링).
3. 시험판 번호 `0.16.0-rs.N`.
4. 나란히 설치 이름표와 중복 전환 막기(새 안내 문구 1건).
5. 시험판 한정 차이 — 설치 때 자동 시작 등록 안 함, 이어받을 때 autostart=0·preLogon=0.
6. 2.11 기존 특이점 3건 1:1 유지.
7. rustup 로컬 설치(다운로드 약 155MB · 디스크 0.8~1GB 추정) 여부.
8. 2단계 Mac 화면 방향(착수 때).

## 12. 구현 반영 — 지금 판과 다른 점 (main 결정 2026-11-13)

main 결정 6) "지금 판 문제 3건은 러스트 판에서 고칠 것" 에 따라 1:1 에서 벗어난 곳.

1. 「드래그 중 전환 안 함」 — 토글 값이 트레이·바탕화면 워커 판정에 쓰임(`logon-edges.txt` 에 `dragGuard=` 줄 추가). 버튼 상태는 GetAsyncKeyState(실제 버튼). 로그온 화면 워커는 지금처럼 버튼 판정 없음.
2. 제거 정리 — 설치 파일이 제거 때 `--uninstall-cleanup` 실행 → 자동 시작 해제, 머신 범위에 남은 것이 있으면 그때만 UAC 로 `--uninstall-machine`(서비스 중지·삭제 · 예약 작업 · 방화벽 규칙 · `%ProgramData%\LogiFlowRs`). 사용자 설정은 보존.
3. 클립보드 암호문구 — `clipSecretDpapi=<hex>`(DPAPI 사용자 범위). 보호 실패 시 암호문구를 아예 쓰지 않음(평문 대체 없음). C# 판 이어받기 때 옮겨 담고 C# 원본은 그대로.

그 밖의 차이: 7절 1~8(승인) · 설정 창이 상태 갱신 때 맨 위로 돌아가지 않음 · 처음 설치도 자동 시작 꺼짐(시험판 한정) · 서비스 켜기 때 실행 중인 서비스를 먼저 멈추고 워커 사본을 새 판으로 덮어씀(지금 판은 사본이 잠겨 옛 판이 남을 수 있었음).

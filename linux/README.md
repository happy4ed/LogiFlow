# LogiFlow 리눅스 CLI

터미널 명령으로 로지텍 Easy-Switch 채널을 넘긴다(우분투 기준). 윈도우판과 같은 HID++ 코드
(`../LogiFlow/Hidpp`, `../LogiFlow/Diagnostics`)를 소스 링크로 쓰고, 리눅스 전용은
hidraw 전송·열거(`src/HidStream.cs`, `src/HidEnumerator.cs`)와 CLI(`src/Program.cs`)뿐이다.

## 설치

받기: GitHub 릴리스(Latest)의 `LogiFlow-v<버전>-linux-x64.tar.gz`(arm 이면 arm64). 고정 링크: `releases/latest/download/LogiFlow-linux-x64.tar.gz`.

```sh
tar xzf LogiFlow-linux-x64.tar.gz
cd LogiFlow-v*-linux-x64
sudo ./install.sh        # /usr/local/bin/logiflow + udev 규칙
```

## 사용

```sh
logiflow list            # 기기·현재 채널(처음 한 번 — 기기 정보 저장)
logiflow switch 2        # 마우스·키보드를 채널 2로
logiflow 2               # 같음
```

- 기기가 지금 이 PC 에 연결돼 있을 때 다른 채널로 보내는 명령이다(다른 PC 에 가 있는 기기를 당겨오기는 안 됨).
- GNOME 단축키: 설정 → 키보드 → 사용자 지정 단축키 → 명령 `logiflow switch 2`.
- 문제 시 `logiflow switch 2 --debug` → 마지막 줄의 HID 로그 경로 파일을 보내 주기.

## 빌드

CI(`.github/workflows/linux-release.yml`, ubuntu-latest): `dotnet publish -r linux-x64|linux-arm64`
self-contained 단일 파일 → tar.gz. 버전은 `linux/VERSION`.

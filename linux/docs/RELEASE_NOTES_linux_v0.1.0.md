# LogiFlow 리눅스 0.1.0 (CLI 첫 판 · 실기 미검증)

우분투 터미널에서 명령으로 로지텍 Easy-Switch 채널을 넘긴다.

- `logiflow list` — 기기·현재 채널 보기(기기 정보 저장).
- `logiflow switch N` / `logiflow N` — 마우스·키보드를 채널 N 으로(저장된 정보로 바로 전송, 실패하면 다시 찾아 한 번 더).
- 전송 방식은 윈도우 0.14.3 과 같음: 전 기기 setHost 를 연달아 보내고 확인은 그 뒤.
- `sudo ./install.sh` — `/usr/local/bin/logiflow` + udev 규칙(sudo 없이 장치 사용).
- x64 · arm64 단일 실행 파일(.NET 런타임 설치 불필요).

한계: 기기가 이 PC 에 연결돼 있을 때 보내기만(당겨오기 불가). 엣지 밀기 전환·glow 는 다음 단계(GNOME 확장).

## 확인 부탁

1. `sudo ./install.sh` → `logiflow list` 에 마우스·키보드와 채널이 나오는지.
2. `logiflow switch 2`(다른 PC 채널) → 둘 다 넘어가는지.
3. 안 되면 `logiflow list --debug` 출력과 마지막 줄 로그 파일 붙여 주기.

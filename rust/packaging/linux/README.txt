LogiFlow(러스트 판) — 리눅스 터미널 명령

  sudo ./install.sh          설치(/usr/local/bin/logiflow-rs + udev 규칙, 한 번만)
  logiflow-rs list           기기·현재 채널 보기
  logiflow-rs switch 2       이 PC 에 붙은 마우스·키보드를 채널 2 로
  logiflow-rs 2              위와 같음
  --rescan  저장된 기기 정보 무시하고 다시 찾기 · --debug  HID 송수신 로그
  sudo ./uninstall.sh        삭제

지금 판(logiflow)과 같은 명령이고 나란히 깔린다.

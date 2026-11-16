#!/bin/sh
# LogiFlow(러스트 판) 리눅스 설치: /usr/local/bin/logiflow-rs + udev 규칙(sudo 없이 장치 사용).
# 지금 판(C#, logiflow · 42-logiflow.rules)과 이름이 달라 나란히 깔린다.
set -e
DIR=$(cd "$(dirname "$0")" && pwd)
if [ "$(id -u)" -ne 0 ]; then
  echo "sudo ./install.sh 로 실행"
  exit 1
fi
install -m 0755 "$DIR/logiflow-rs" /usr/local/bin/logiflow-rs
install -m 0644 "$DIR/43-logiflow-rs.rules" /etc/udev/rules.d/43-logiflow-rs.rules
udevadm control --reload-rules
udevadm trigger --subsystem-match=hidraw --action=change
echo "설치 끝 — 예: logiflow-rs list · logiflow-rs switch 2"

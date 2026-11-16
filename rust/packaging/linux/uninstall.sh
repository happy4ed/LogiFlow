#!/bin/sh
set -e
if [ "$(id -u)" -ne 0 ]; then
  echo "sudo ./uninstall.sh 로 실행"
  exit 1
fi
rm -f /usr/local/bin/logiflow-rs /etc/udev/rules.d/43-logiflow-rs.rules
udevadm control --reload-rules
echo "삭제 끝(기기 캐시 ~/.cache/logiflow-rs 는 남김)"

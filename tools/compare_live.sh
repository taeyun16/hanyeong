#!/bin/sh
# gksdud, 도구 없음, hanyeong(설치본)을 번갈아 돌며 같은 조건에서 잰다. 결과는 docs/perf-progress.md 에 옮긴다.
#
#   tools/compare_live.sh <새 출력 폴더> [바퀴 수, 기본 3]
#
# 재는 것: 보통 키의 통과 지연(probe passthrough, 두 도구 모두 손대지 않는 키), 가만히 둔 30초의
# CPU 시간과 문맥 전환, 메모리. 전환 지연은 재지 않는다: 합성한 F19 는 도구가 다시 보낸 복사본에
# 측정 프로그램의 표시가 남아 도구가 자기 누름을 알아보지 못한다(2026-11-16 확인).
#
# 접근성 권한이 있는 터미널에서 돌린다. 재는 동안 키보드와 마우스를 만지지 않는다.
# 끝나면 hanyeong 설치본을 다시 올리고 gksdud 는 꺼 둔다.
set -eu
out=${1:?출력 폴더}
rounds=${2:-3}
bin=$(cd "$(dirname "$0")/.." && pwd)/target/release/hanyeong
domain="gui/$(id -u)"
agent="$HOME/Library/LaunchAgents/dev.hanyeong.plist"
[ -e "$out" ] && { echo "이미 있는 폴더입니다: $out" >&2; exit 2; }
mkdir -p "$out"

stop_all() {
    launchctl bootout "$domain/dev.hanyeong" 2>/dev/null || true
    osascript -e 'tell application "gksdud" to quit' 2>/dev/null || true
    for _ in 1 2 3 4 5 6 7 8 9 10; do
        pgrep -x gksdud >/dev/null || pgrep -x hanyeong >/dev/null || return 0
        sleep 1
    done
    echo "도구가 끝나지 않았습니다" >&2
    exit 1
}

start() {
    case $1 in
        gksdud) open -a gksdud ;;
        hanyeong) launchctl bootstrap "$domain" "$agent" ;;
        none) return 0 ;;
    esac
    # 매핑과 단축키를 걸 시간을 준다.
    sleep 6
    pgrep -x "$1" >/dev/null || { echo "$1 이(가) 뜨지 않았습니다" >&2; exit 1; }
}

idle() {
    pid=$(pgrep -x "$1" | head -1)
    {
        footprint "$pid" | grep phys_footprint
        top -l 1 -pid "$pid" -stats time,csw,threads | tail -1
        sleep 30
        top -l 1 -pid "$pid" -stats time,csw,threads | tail -1
    } >"$2" 2>&1
}

for round in $(seq 1 "$rounds"); do
    for tool in gksdud none hanyeong; do
        stop_all
        start "$tool"
        label="r$round-$tool"
        echo "$label $(date +%T) $(uptime | sed 's/.*load averages: //')" | tee -a "$out/order.txt"
        "$bin" status >"$out/$label.status.txt" 2>&1
        "$bin" probe --mode passthrough --count 200 --gap-ms 20 >"$out/$label.passthrough.txt" 2>&1 || true
        [ "$tool" = none ] || idle "$tool" "$out/$label.idle.txt"
    done
done
stop_all
launchctl bootstrap "$domain" "$agent"
echo "끝났습니다: $out"
grep -h "중앙값" "$out"/*.passthrough.txt | head -20

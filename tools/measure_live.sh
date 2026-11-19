#!/bin/sh
# 지금 돌고 있는 한영 도구 하나를 실기기에서 잰다. 결과는 docs/perf-progress.md 에 옮겨 적는다.
#
#   tools/measure_live.sh <gksdud|hanyeong|none> <새 출력 폴더> [가만히 두는 초, 기본 60]
#
# probe 는 입력 소스를 실제로 바꾼다(짝수 번이라 원래대로 돌아온다). 재는 동안 키보드를 만지지 않는다.
# 화면이 잠겨 있으면 키 이벤트가 탭에 오지 않아 probe 가 전부 놓친다.
set -eu
tool=${1:?도구 이름}
out=${2:?출력 폴더}
idle=${3:-60}
bin=$(dirname "$0")/../target/release/hanyeong
[ -e "$out" ] && { echo "이미 있는 폴더입니다: $out" >&2; exit 2; }
mkdir -p "$out"
{
    date
    sw_vers
    uptime
    "$bin" status
} >"$out/host.txt" 2>&1

if [ "$tool" != none ]; then
    pid=$(pgrep -x "$tool" | head -1)
    [ -n "$pid" ] || { echo "$tool 이(가) 돌고 있지 않습니다" >&2; exit 2; }
    {
        ps -o pid,rss,time,etime -p "$pid"
        echo "threads: $(ps -M -p "$pid" | tail -n +2 | wc -l)"
        footprint "$pid" | grep -E "phys_footprint"
        echo "--- $idle 초 가만히 둔 앞뒤의 누적 CPU 시간, 깨어남, 문맥 전환"
        top -l 1 -pid "$pid" -stats time,idlew,csw | tail -1
        sleep "$idle"
        top -l 1 -pid "$pid" -stats time,idlew,csw | tail -1
    } >"$out/resident.txt" 2>&1
fi

"$bin" probe --mode passthrough --count 300 --gap-ms 20 >"$out/passthrough.txt" 2>&1 || true
"$bin" probe --mode switch --count 40 --hold-ms 30 --gap-ms 250 >"$out/switch.txt" 2>&1 || true
uptime >>"$out/host.txt"
head -40 "$out"/*.txt

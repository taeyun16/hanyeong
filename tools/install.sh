#!/bin/sh
# hanyeong 을 앱 번들로 묶어 로그인할 때 뜨게 한다.
#
#   tools/install.sh            빌드 → ~/Applications/hanyeong.app → LaunchAgent 등록·시작
#   tools/install.sh uninstall  멈추고(스스로 되돌린다) LaunchAgent 와 앱을 지운다
#
# tools/setup_signing.sh 로 만든 전용 인증서가 있으면 그것으로 서명한다. 그러면 접근성 권한이
# "이 인증서로 서명한 dev.hanyeong" 에 묶여 다시 설치해도 유지된다. 없으면 임시(ad-hoc) 서명이고, 권한이
# 빌드 내용에 묶여 내용이 바뀐 빌드를 설치할 때마다 풀린다. 서명 방식을 바꾸면 권한이 한 번 풀리는데,
# 그때 목록에 남은 옛 항목은 껐다 켜거나 다시 더해도 새 서명으로 바뀌지 않는다(2026-11-16 에 겪었다).
# 옛 항목을 지우고 다시 더해야 한다:
#
#   tccutil reset Accessibility dev.hanyeong
#   시스템 설정 > 개인정보 보호 및 보안 > 손쉬운 사용 > + 로 ~/Applications/hanyeong.app
set -eu
label=dev.hanyeong
root=$(cd "$(dirname "$0")/.." && pwd)
app="$HOME/Applications/hanyeong.app"
agent="$HOME/Library/LaunchAgents/$label.plist"
log="$HOME/Library/Logs/hanyeong.log"
domain="gui/$(id -u)"

launchctl bootout "$domain/$label" 2>/dev/null || true
# 권한 안내를 새로 하도록 지난 표시를 지운다.
defaults delete "$label" trust.prompted 2>/dev/null || true
if [ "${1:-}" = uninstall ]; then
    rm -f "$agent"
    rm -rf "$app"
    echo "지웠습니다: $agent, $app"
    exit 0
fi

(cd "$root" && cargo build --release --locked -p hanyeong)
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources" "$(dirname "$agent")" "$(dirname "$log")"
cp "$root/assets/AppIcon.icns" "$app/Contents/Resources/AppIcon.icns"
cp "$root/target/release/hanyeong" "$app/Contents/MacOS/hanyeong"
cat >"$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleIdentifier</key><string>$label</string>
    <key>CFBundleName</key><string>hanyeong</string>
    <key>CFBundleExecutable</key><string>hanyeong</string>
    <key>CFBundleIconFile</key><string>AppIcon</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>0.1.0</string>
    <key>LSUIElement</key><true/>
</dict>
</plist>
PLIST
# 전용 인증서(tools/setup_signing.sh)가 있으면 그것으로 서명한다. 신뢰 설정을 바꾸지 않았으므로
# 이름이 아니라 인증서의 SHA-1 로 고른다.
identity=$(security find-certificate -c "hanyeong Local Signing" -Z "$HOME/Library/Keychains/login.keychain-db" 2>/dev/null \
    | awk '/SHA-1 hash:/ { print $3; exit }')
if [ -n "$identity" ]; then
    codesign --force --sign "$identity" --identifier "$label" "$app"
    echo "고정 서명으로 서명했습니다($identity). 권한은 다시 설치해도 유지됩니다."
else
    codesign --force --sign - --identifier "$label" "$app"
    echo "임시 서명입니다. 내용이 바뀐 빌드를 다시 설치하면 권한이 풀립니다(tools/setup_signing.sh)."
fi

cat >"$agent" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key><string>$label</string>
    <key>ProgramArguments</key>
    <array>
        <string>$app/Contents/MacOS/hanyeong</string>
        <string>run</string>
    </array>
    <key>RunAtLoad</key><true/>
    <key>KeepAlive</key><dict><key>SuccessfulExit</key><false/></dict>
    <key>ThrottleInterval</key><integer>10</integer>
    <key>ProcessType</key><string>Interactive</string>
    <key>StandardOutPath</key><string>$log</string>
    <key>StandardErrorPath</key><string>$log</string>
</dict>
</plist>
PLIST
launchctl bootstrap "$domain" "$agent"
echo "설치했습니다: $app"
echo "로그: $log"
echo "처음이거나 서명 방식을 바꿨으면: tccutil reset Accessibility $label 뒤 손쉬운 사용에서 + 로 $app 을 더해 켜세요."
echo "켜질 때까지 10초마다 다시 시도하고, 켜지면 메뉴바에 아이콘이 뜹니다. 메뉴에서 종료하면 다시 뜨지 않습니다."

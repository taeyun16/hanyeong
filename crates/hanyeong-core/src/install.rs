//! `install` 이 만드는 것: 앱 번들의 위치와 Info.plist, 로그인할 때 띄우는 LaunchAgent.
//!
//! 여기서는 내용만 만든다. 파일을 쓰고 `codesign`·`launchctl` 을 부르는 일은 `hanyeong` 이 한다.

use crate::args::Keys;

/// 번들 식별자이자 LaunchAgent 이름이자 설정 도메인. 접근성 권한도 이 이름에 붙는다.
pub const LABEL: &str = "dev.hanyeong";

/// `tools/setup_signing.sh` 가 로그인 키체인에 만드는 자체 서명 인증서의 이름.
pub const SIGNING_CERT: &str = "hanyeong Local Signing";

/// 설치에 쓰는 경로들. 모두 홈 폴더 아래라 관리자 권한이 필요 없다.
#[derive(PartialEq, Eq, Debug)]
pub struct Paths {
    pub app: String,
    pub macos: String,
    pub resources: String,
    pub exe: String,
    pub info_plist: String,
    pub agents: String,
    pub agent: String,
    pub logs: String,
    pub log: String,
}

impl Paths {
    #[must_use]
    pub fn under(home: &str) -> Self {
        let app = format!("{home}/Applications/hanyeong.app");
        let agents = format!("{home}/Library/LaunchAgents");
        let logs = format!("{home}/Library/Logs");
        Self {
            macos: format!("{app}/Contents/MacOS"),
            resources: format!("{app}/Contents/Resources"),
            exe: format!("{app}/Contents/MacOS/hanyeong"),
            info_plist: format!("{app}/Contents/Info.plist"),
            agent: format!("{agents}/{LABEL}.plist"),
            log: format!("{logs}/hanyeong.log"),
            app,
            agents,
            logs,
        }
    }
}

/// plist 의 문자열 자리에 넣을 수 있게 XML 특수 문자를 바꾼다.
#[must_use]
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            other => out.push(other),
        }
    }
    out
}

const PLIST_HEAD: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
"#;

/// 앱 번들의 Info.plist. 메뉴바에만 뜨도록 Dock 아이콘을 감춘다(`LSUIElement`).
#[must_use]
pub fn info_plist(version: &str) -> String {
    let version = escape(version);
    format!(
        "{PLIST_HEAD}    <key>CFBundleIdentifier</key><string>{LABEL}</string>
    <key>CFBundleName</key><string>hanyeong</string>
    <key>CFBundleExecutable</key><string>hanyeong</string>
    <key>CFBundleIconFile</key><string>AppIcon</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>{version}</string>
    <key>LSUIElement</key><true/>
</dict>
</plist>
"
    )
}

/// 로그인할 때 `hanyeong run` 을 띄우는 LaunchAgent.
///
/// 실패로 끝나면(접근성 권한이 아직 없을 때) 10초 뒤 다시 띄우고, 메뉴에서 종료해 정상으로 끝나면
/// 다시 띄우지 않는다.
#[must_use]
pub fn launch_agent(paths: &Paths, keys: Keys) -> String {
    let exe = escape(&paths.exe);
    let log = escape(&paths.log);
    let source = keys.source.name();
    let target = keys.target.name();
    format!(
        "{PLIST_HEAD}    <key>Label</key><string>{LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe}</string>
        <string>run</string>
        <string>--key</string>
        <string>{source}</string>
        <string>--target</string>
        <string>{target}</string>
    </array>
    <key>RunAtLoad</key><true/>
    <key>KeepAlive</key><dict><key>SuccessfulExit</key><false/></dict>
    <key>ThrottleInterval</key><integer>10</integer>
    <key>ProcessType</key><string>Interactive</string>
    <key>StandardOutPath</key><string>{log}</string>
    <key>StandardErrorPath</key><string>{log}</string>
</dict>
</plist>
"
    )
}

/// `security find-certificate -Z` 의 출력에서 첫 인증서의 SHA-1 을 꺼낸다.
#[must_use]
pub fn sha1_of(security_output: &str) -> Option<&str> {
    security_output
        .lines()
        .find_map(|line| line.trim().strip_prefix("SHA-1 hash:"))
        .map(str::trim)
        .filter(|hash| hash.len() == 40 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::{SourceKey, TargetKey};

    #[test]
    fn everything_lives_under_the_home_folder() {
        let paths = Paths::under("/Users/me");
        assert_eq!(paths.app, "/Users/me/Applications/hanyeong.app");
        assert_eq!(
            paths.exe,
            "/Users/me/Applications/hanyeong.app/Contents/MacOS/hanyeong"
        );
        assert_eq!(
            paths.agent,
            "/Users/me/Library/LaunchAgents/dev.hanyeong.plist"
        );
        assert_eq!(paths.log, "/Users/me/Library/Logs/hanyeong.log");
    }

    #[test]
    fn the_agent_runs_the_bundle_with_the_chosen_keys() {
        let paths = Paths::under("/Users/me");
        let keys = Keys {
            source: SourceKey::CapsLock,
            target: TargetKey::F18,
        };
        let agent = launch_agent(&paths, keys);
        assert!(agent.contains(
            "<string>/Users/me/Applications/hanyeong.app/Contents/MacOS/hanyeong</string>
        <string>run</string>
        <string>--key</string>
        <string>caps-lock</string>
        <string>--target</string>
        <string>f18</string>"
        ));
        assert!(agent.contains("<key>SuccessfulExit</key><false/>"));
        assert!(agent.ends_with("</plist>\n"));
    }

    #[test]
    fn odd_characters_in_a_home_folder_do_not_break_the_plist() {
        let agent = launch_agent(&Paths::under("/Users/a&b <c>"), Keys::default());
        assert!(agent.contains("/Users/a&amp;b &lt;c&gt;/Applications/"));
        assert!(!agent.contains("a&b"));
    }

    #[test]
    fn the_bundle_carries_its_version_and_hides_from_the_dock() {
        let plist = info_plist("0.2.0");
        assert!(plist.contains("<key>CFBundleShortVersionString</key><string>0.2.0</string>"));
        assert!(plist.contains("<key>CFBundleIdentifier</key><string>dev.hanyeong</string>"));
        assert!(plist.contains("<key>LSUIElement</key><true/>"));
    }

    #[test]
    fn the_certificate_hash_is_read_from_security_output() {
        let output = "SHA-256 hash: 00\nSHA-1 hash: 4D61CDA13513C79CF33E6B21ACAFE025EE65FEE3\nkeychain: \"x\"\n";
        assert_eq!(
            sha1_of(output),
            Some("4D61CDA13513C79CF33E6B21ACAFE025EE65FEE3")
        );
        assert_eq!(sha1_of(""), None);
        assert_eq!(sha1_of("SHA-1 hash: not-a-hash\n"), None);
    }
}

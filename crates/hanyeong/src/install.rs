//! `install` 과 `uninstall`: `~/Applications/hanyeong.app` 을 만들고 로그인할 때 뜨게 한다.
//!
//! brew·curl·cargo 어느 쪽으로 받았든 실행 파일 하나만 있으면 된다. 지금 도는 실행 파일을 번들 안에
//! 복사하므로, 새 버전을 받으면 `install` 을 다시 돌린다. 무엇을 쓸지는 `hanyeong_core::install` 이
//! 정하고, 여기서는 파일을 쓰고 `codesign`·`launchctl` 을 부르기만 한다.

use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};

use hanyeong_core::args::Keys;
use hanyeong_core::install::{self, LABEL, Paths};

/// 번들 아이콘의 원본(256×256). 설치할 때 `sips` 로 icns 로 바꾼다. icns 를 그대로 넣으면 실행 파일이
/// 두 배 가까이 커진다.
const ICON_PNG: &[u8] = include_bytes!("../../../assets/icon.png");

fn quiet(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn home() -> Option<String> {
    std::env::var("HOME")
        .ok()
        .filter(|home| home.starts_with('/'))
}

/// `launchctl` 의 사용자 도메인(`gui/<uid>`).
fn domain() -> Option<String> {
    let output = Command::new("id").arg("-u").output().ok()?;
    let uid = String::from_utf8(output.stdout).ok()?;
    let uid = uid.trim();
    (!uid.is_empty() && uid.bytes().all(|byte| byte.is_ascii_digit())).then(|| format!("gui/{uid}"))
}

/// 돌고 있으면 멈춘다. 멈출 때 `run` 이 매핑과 단축키를 스스로 되돌린다.
fn stop(domain: &str) {
    quiet("launchctl", &["bootout", &format!("{domain}/{LABEL}")]);
    // 권한 안내를 새로 하도록 지난 표시를 지운다.
    quiet("defaults", &["delete", LABEL, "trust.prompted"]);
}

fn remove_dir(path: &str) -> io::Result<()> {
    match fs::remove_dir_all(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

fn remove_file(path: &str) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

fn write_bundle(paths: &Paths, exe: &[u8]) -> io::Result<()> {
    remove_dir(&paths.app)?;
    fs::create_dir_all(&paths.macos)?;
    fs::create_dir_all(&paths.resources)?;
    fs::write(&paths.exe, exe)?;
    fs::set_permissions(&paths.exe, fs::Permissions::from_mode(0o755))?;
    fs::write(
        &paths.info_plist,
        install::info_plist(env!("CARGO_PKG_VERSION")),
    )?;
    let png = format!("{}/icon.png", paths.resources);
    let icns = format!("{}/AppIcon.icns", paths.resources);
    fs::write(&png, ICON_PNG)?;
    if !quiet("sips", &["-s", "format", "icns", &png, "--out", &icns]) {
        err!("hanyeong: 아이콘을 만들지 못했습니다. 아이콘 없이 설치합니다");
    }
    remove_file(&png)
}

fn write_agent(paths: &Paths, keys: Keys) -> io::Result<()> {
    fs::create_dir_all(&paths.agents)?;
    fs::create_dir_all(&paths.logs)?;
    fs::write(&paths.agent, install::launch_agent(paths, keys))
}

/// `tools/setup_signing.sh` 로 만든 인증서의 SHA-1. 신뢰 설정을 바꾸지 않은 인증서라 이름이 아니라
/// 해시로 고른다.
fn local_identity(home: &str) -> Option<String> {
    let keychain = format!("{home}/Library/Keychains/login.keychain-db");
    let output = Command::new("security")
        .args([
            "find-certificate",
            "-c",
            install::SIGNING_CERT,
            "-Z",
            &keychain,
        ])
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let text = String::from_utf8(output.stdout).ok()?;
    install::sha1_of(&text).map(String::from)
}

/// 서명한다. 전용 인증서가 있으면 그것으로, 없으면 임시(ad-hoc) 서명. 고정 서명이면 접근성 권한이
/// "이 인증서로 서명한 dev.hanyeong" 에 묶여 새 버전을 설치해도 남는다.
fn sign(home: &str, paths: &Paths) {
    let identity = local_identity(home);
    let with = identity.as_deref().unwrap_or("-");
    let signed = quiet(
        "codesign",
        &["--force", "--sign", with, "--identifier", LABEL, &paths.app],
    );
    match (signed, identity.is_some()) {
        (false, _) => {
            err!("hanyeong: 서명하지 못했습니다(codesign). 접근성 권한이 붙지 않을 수 있습니다")
        }
        (true, true) => {
            out!("전용 인증서로 서명했습니다. 새 버전을 설치해도 접근성 권한이 남습니다.")
        }
        (true, false) => out!(
            "임시 서명입니다. 새 버전을 설치하면 접근성 권한을 다시 줘야 할 수 있습니다(README 의 \"권한 유지하기\")."
        ),
    }
}

pub fn install(keys: Keys) -> bool {
    let (Some(home), Some(domain)) = (home(), domain()) else {
        err!("hanyeong: 홈 폴더나 사용자 번호를 알 수 없습니다");
        return false;
    };
    let paths = Paths::under(&home);
    // 번들 안의 실행 파일로 다시 설치할 수도 있으므로, 번들을 지우기 전에 읽어 둔다.
    let exe = match std::env::current_exe().and_then(fs::read) {
        Ok(exe) => exe,
        Err(error) => {
            err!("hanyeong: 실행 파일을 읽지 못했습니다: {error}");
            return false;
        }
    };
    stop(&domain);
    if let Err(error) = write_bundle(&paths, &exe).and_then(|()| write_agent(&paths, keys)) {
        err!("hanyeong: 설치하지 못했습니다: {error}");
        return false;
    }
    sign(&home, &paths);
    if !quiet("launchctl", &["bootstrap", &domain, &paths.agent]) {
        err!(
            "hanyeong: 로그인 항목을 등록하지 못했습니다: {}",
            paths.agent
        );
        return false;
    }
    out!("설치했습니다: {}", paths.app);
    out!("로그: {}", paths.log);
    out!(
        "처음이면 손쉬운 사용 화면이 열립니다. 목록에서 hanyeong 을 켜면 10초 안에 메뉴바에 '한' 이 뜹니다."
    );
    true
}

pub fn uninstall() -> bool {
    let (Some(home), Some(domain)) = (home(), domain()) else {
        err!("hanyeong: 홈 폴더나 사용자 번호를 알 수 없습니다");
        return false;
    };
    let paths = Paths::under(&home);
    stop(&domain);
    if let Err(error) = remove_file(&paths.agent).and_then(|()| remove_dir(&paths.app)) {
        err!("hanyeong: 지우지 못했습니다: {error}");
        return false;
    }
    out!("지웠습니다: {}, {}", paths.agent, paths.app);
    out!("접근성 목록의 항목은 남습니다. 지우려면: tccutil reset Accessibility {LABEL}");
    true
}

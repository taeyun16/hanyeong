//! 출력을 받는 쪽이 먼저 닫혀도(`hanyeong status | head -1`) 죽지 않는다.
//!
//! 릴리스 빌드는 `panic = "abort"` 라, 출력하다 패닉하면 `run` 은 키 매핑과 단축키를 되돌리지 못하고
//! 끝난다. 여기서는 시스템을 건드리지 않는 `help`·`version` 으로, 읽는 쪽을 미리 닫은 파이프에 쓰게 한다.
#![cfg(target_os = "macos")]

use std::io;
use std::process::{Command, Stdio};

fn run_with_closed_stdout(command: &str) -> std::process::ExitStatus {
    let (reader, writer) = io::pipe().expect("파이프를 만들지 못했습니다");
    drop(reader);
    Command::new(env!("CARGO_BIN_EXE_hanyeong"))
        .arg(command)
        .stdout(writer)
        .stderr(Stdio::null())
        .status()
        .expect("hanyeong 을 띄우지 못했습니다")
}

#[test]
fn help_survives_a_closed_stdout() {
    assert!(run_with_closed_stdout("help").success());
}

#[test]
fn version_survives_a_closed_stdout() {
    assert!(run_with_closed_stdout("version").success());
}

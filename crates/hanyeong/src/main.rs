//! `hanyeong`: 누르는 순간 전환되는 Mac 한영 키.
//!
//!   hanyeong run     [--key right-command] [--target f19]   한영 키를 켜고, 끝날 때 되돌린다
//!   hanyeong status  [--key …] [--target …]                 지금 상태를 보여 준다
//!   hanyeong restore [--key …] [--target …]                 비정상 종료로 남은 설정을 되돌린다
//!   hanyeong install [--key …] [--target …]                 ~/Applications 에 앱을 만들고 로그인할 때 켜지게 한다
//!   hanyeong uninstall                                      멈추고(스스로 되돌린다) 설치한 것을 지운다
//!   hanyeong probe   [--mode switch|passthrough|react] [--count 20] [--hold-ms 30] [--gap-ms 200]
//!                                                           돌고 있는 한영 도구의 지연을 잰다
//!
//! 메뉴바에 아이콘 하나(일시정지·종료)만 둔다. 설정 창도 툴팁도 없다.
//! 종료 코드: 0 정상, 1 실행 실패, 2 사용법 오류.
#![cfg(target_os = "macos")]
#![forbid(unsafe_code)]

mod app;
mod install;
mod report;
mod rivals;

use std::process::ExitCode;

use hanyeong_core::args::{self, Command};

const USAGE: &str = "사용법: hanyeong <run|status|restore|install|uninstall|probe|version> [옵션]
  run|status|restore|install  [--key right-command|right-option|right-control|caps-lock] [--target f13..f20]
  probe                       [--mode switch|passthrough|react] [--target f13..f20] [--count N] [--hold-ms N] [--gap-ms N]";
const EXIT_USAGE: u8 = 2;

fn main() -> ExitCode {
    let words: Vec<String> = std::env::args().skip(1).collect();
    let command = match args::parse(words.iter().map(String::as_str)) {
        Ok(command) => command,
        Err(error) => {
            eprintln!("hanyeong: {error}\n{USAGE}");
            return ExitCode::from(EXIT_USAGE);
        }
    };
    let done = match command {
        Command::Run(keys) => app::run(keys),
        Command::Status(keys) => report::status(keys),
        Command::Restore(keys) => app::restore(keys),
        Command::Probe(probe) => report::probe(probe),
        Command::Install(keys) => install::install(keys),
        Command::Uninstall => install::uninstall(),
        Command::Help => {
            println!("{USAGE}");
            true
        }
        Command::Version => {
            println!("hanyeong {}", env!("CARGO_PKG_VERSION"));
            true
        }
    };
    if done {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

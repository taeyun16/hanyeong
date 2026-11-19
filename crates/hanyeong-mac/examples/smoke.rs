//! 권한 없이 확인할 수 있는 macOS 호출을 실제로 돌려 본다. 테스트가 아니다: 실제 키보드에 쓴다.
//!
//!   smoke session    런 루프를 띄우고 1초 뒤 자기에게 SIGTERM 을 보내 멈추는지 본다
//!   smoke mapping    첫 키보드에 F24 → F24(아무 효과 없는 매핑)를 넣었다 걷어내고 다시 읽는다

use std::process::{Command, ExitCode};

use hanyeong_core::mapping::{self, Change, Pair};
use hanyeong_mac::hid::Keyboards;
use hanyeong_mac::session::{self, Events};

struct Counter {
    rechecks: u32,
}

impl Events for Counter {
    fn recheck(&mut self) -> u32 {
        self.rechecks = self.rechecks.saturating_add(1);
        if self.rechecks == 3 {
            let pid = format!("{}", std::process::id());
            let sent = Command::new("/bin/kill").args(["-TERM", &pid]).status();
            println!("SIGTERM 보냄: {sent:?}");
        }
        300
    }

    fn device_changed(&mut self) -> u32 {
        println!("키보드가 붙었다는 알림");
        300
    }
}

fn session() -> bool {
    let mut counter = Counter { rechecks: 0 };
    let ran = session::run(&mut counter, 100, None);
    println!("런 루프 종료: {ran:?}, 확인 {}번", counter.rechecks);
    ran.is_ok() && counter.rechecks >= 3
}

fn mapping() -> bool {
    let inert = Pair {
        src: 0x7_0000_0073,
        dst: 0x7_0000_0073,
    };
    let Some(keyboards) = Keyboards::open() else {
        return false;
    };
    let mut done = None;
    keyboards.each(|keyboard| {
        if done.is_some() {
            return;
        }
        let Some(before) = keyboard.mappings() else {
            return;
        };
        let Change::Write { list, displaced } = mapping::apply(&before, inert) else {
            return;
        };
        let wrote = keyboard.set_mappings(&list);
        let during = keyboard.mappings();
        let restored = match mapping::remove(during.as_deref().unwrap_or(&[]), inert, displaced) {
            Change::Write { list, .. } => keyboard.set_mappings(&list),
            Change::Keep | Change::TooMany => false,
        };
        let after = keyboard.mappings();
        println!(
            "키보드 {:#x}: 전 {before:?}, 쓰기 {wrote}, 중 {during:?}, 되돌리기 {restored}, 후 {after:?}",
            keyboard.id().unwrap_or(0)
        );
        done = Some(wrote && during.as_deref() == Some(&list[..]) && restored && after == Some(before));
    });
    done == Some(true)
}

fn main() -> ExitCode {
    let passed = match std::env::args().nth(1).as_deref() {
        Some("session") => session(),
        Some("mapping") => mapping(),
        _ => {
            eprintln!("사용법: smoke <session|mapping>");
            return ExitCode::from(2);
        }
    };
    println!("{}", if passed { "PASS" } else { "FAIL" });
    if passed {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

//! 함께 돌면 안 되는 프로세스를 찾는다.

use std::process::Command;

/// 같은 수법으로 대상 F-키를 다시 보내는 도구. 함께 돌면 서로의 키를 주고받는다.
const RIVALS: [&str; 1] = ["gksdud"];

/// `pgrep -x` 로 그 이름의 프로세스를 센다. 자기 자신은 뺀다.
pub fn running(name: &str) -> usize {
    let Ok(found) = Command::new("/usr/bin/pgrep").args(["-x", name]).output() else {
        return 0;
    };
    let own = std::process::id();
    found
        .stdout
        .split(|byte| *byte == b'\n')
        .filter_map(|line| str::from_utf8(line).ok()?.trim().parse::<u32>().ok())
        .filter(|pid| *pid != own)
        .count()
}

/// 함께 돌면 안 되는 것이 돌고 있으면 그 이름.
pub fn rival() -> Option<&'static str> {
    RIVALS
        .into_iter()
        .chain(["hanyeong"])
        .find(|name| running(name) > 0)
}

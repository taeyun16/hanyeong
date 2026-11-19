//! 명령줄을 읽는다.

use core::fmt;

use crate::keys::{SourceKey, TargetKey};

/// `probe` 가 한 번에 보내는 누름 수의 상한.
pub const MAX_PROBE_COUNT: u32 = 10_000;

/// 어느 키를 어느 F-키로 바꿀지.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Keys {
    pub source: SourceKey,
    pub target: TargetKey,
}

impl Default for Keys {
    fn default() -> Self {
        Self {
            source: SourceKey::RightCommand,
            target: TargetKey::F19,
        }
    }
}

/// `probe` 가 재는 것.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProbeMode {
    /// 대상 F-키를 누른 뒤 입력 소스가 바뀌었다는 알림까지의 시간.
    Switch,
    /// 아무 뜻 없는 키 하나가 이벤트 탭들을 지나 세션 끝에 닿기까지의 시간.
    Passthrough,
    /// 대상 F-키를 누른 뒤 한영 도구가 다시 보낸 누름이 세션 머리를 지나기까지의 시간.
    React,
}

impl ProbeMode {
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        match word {
            "switch" => Some(Self::Switch),
            "passthrough" => Some(Self::Passthrough),
            "react" => Some(Self::React),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Probe {
    pub mode: ProbeMode,
    pub target: TargetKey,
    pub count: u32,
    /// 누름과 뗌 사이(ms).
    pub hold_ms: u32,
    /// 누름과 다음 누름 사이(ms).
    pub gap_ms: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Command {
    Run(Keys),
    Status(Keys),
    Restore(Keys),
    Probe(Probe),
    Help,
    Version,
}

#[derive(PartialEq, Eq, Debug)]
pub enum ArgError {
    MissingCommand,
    UnknownCommand(Box<str>),
    UnknownFlag(Box<str>),
    MissingValue(&'static str),
    BadValue(&'static str, Box<str>),
}

impl fmt::Display for ArgError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCommand => out.write_str("명령이 없습니다"),
            Self::UnknownCommand(word) => write!(out, "모르는 명령입니다: {word}"),
            Self::UnknownFlag(word) => write!(out, "모르는 옵션입니다: {word}"),
            Self::MissingValue(flag) => write!(out, "{flag} 에 값이 없습니다"),
            Self::BadValue(flag, word) => write!(out, "{flag} 의 값을 읽을 수 없습니다: {word}"),
        }
    }
}

fn number(flag: &'static str, word: &str, max: u32) -> Result<u32, ArgError> {
    match word.parse::<u32>() {
        Ok(value) if value <= max => Ok(value),
        _ => Err(ArgError::BadValue(flag, word.into())),
    }
}

/// 실행 파일 이름을 뺀 낱말들을 읽는다.
///
/// # Errors
/// 명령이나 옵션을 읽을 수 없을 때.
pub fn parse<'a>(mut words: impl Iterator<Item = &'a str>) -> Result<Command, ArgError> {
    let command = words.next().ok_or(ArgError::MissingCommand)?;
    let mut keys = Keys::default();
    let mut probe = Probe {
        mode: ProbeMode::Switch,
        target: keys.target,
        count: 20,
        hold_ms: 30,
        gap_ms: 200,
    };
    if !matches!(
        command,
        "run" | "status" | "restore" | "probe" | "help" | "--help" | "-h" | "version" | "--version"
    ) {
        return Err(ArgError::UnknownCommand(command.into()));
    }
    while let Some(flag) = words.next() {
        let (name, slot): (&'static str, u8) = match (command, flag) {
            ("run" | "status" | "restore", "--key") => ("--key", 0),
            ("run" | "status" | "restore" | "probe", "--target") => ("--target", 1),
            ("probe", "--mode") => ("--mode", 2),
            ("probe", "--count") => ("--count", 3),
            ("probe", "--hold-ms") => ("--hold-ms", 4),
            ("probe", "--gap-ms") => ("--gap-ms", 5),
            _ => return Err(ArgError::UnknownFlag(flag.into())),
        };
        let word = words.next().ok_or(ArgError::MissingValue(name))?;
        let bad = || ArgError::BadValue(name, word.into());
        match slot {
            0 => keys.source = SourceKey::parse(word).ok_or_else(bad)?,
            1 => {
                keys.target = TargetKey::parse(word).ok_or_else(bad)?;
                probe.target = keys.target;
            }
            2 => probe.mode = ProbeMode::parse(word).ok_or_else(bad)?,
            3 => probe.count = number(name, word, MAX_PROBE_COUNT)?,
            4 => probe.hold_ms = number(name, word, 10_000)?,
            _ => probe.gap_ms = number(name, word, 10_000)?,
        }
    }
    Ok(match command {
        "run" => Command::Run(keys),
        "status" => Command::Status(keys),
        "restore" => Command::Restore(keys),
        "probe" => Command::Probe(probe),
        "version" | "--version" => Command::Version,
        _ => Command::Help,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(line: &str) -> Result<Command, ArgError> {
        parse(line.split_whitespace())
    }

    #[test]
    fn run_defaults_to_right_command_and_f19() {
        assert_eq!(read("run"), Ok(Command::Run(Keys::default())));
        assert_eq!(Keys::default().source, SourceKey::RightCommand);
        assert_eq!(Keys::default().target, TargetKey::F19);
    }

    #[test]
    fn key_and_target_are_read_for_run_status_and_restore() {
        let keys = Keys {
            source: SourceKey::CapsLock,
            target: TargetKey::F18,
        };
        assert_eq!(
            read("run --key caps-lock --target f18"),
            Ok(Command::Run(keys))
        );
        assert_eq!(
            read("status --target f18 --key caps-lock"),
            Ok(Command::Status(keys))
        );
        assert_eq!(
            read("restore --key caps-lock --target f18"),
            Ok(Command::Restore(keys))
        );
    }

    #[test]
    fn probe_reads_its_mode_and_numbers() {
        assert_eq!(
            read("probe --mode passthrough --count 50 --hold-ms 5 --gap-ms 20 --target f17"),
            Ok(Command::Probe(Probe {
                mode: ProbeMode::Passthrough,
                target: TargetKey::F17,
                count: 50,
                hold_ms: 5,
                gap_ms: 20,
            }))
        );
    }

    #[test]
    fn mistakes_are_named_not_ignored() {
        assert_eq!(read(""), Err(ArgError::MissingCommand));
        assert_eq!(read("start"), Err(ArgError::UnknownCommand("start".into())));
        assert_eq!(
            read("run --count 3"),
            Err(ArgError::UnknownFlag("--count".into()))
        );
        assert_eq!(read("run --key"), Err(ArgError::MissingValue("--key")));
        assert_eq!(
            read("run --key left-command"),
            Err(ArgError::BadValue("--key", "left-command".into()))
        );
        assert_eq!(
            read("probe --count 10001"),
            Err(ArgError::BadValue("--count", "10001".into()))
        );
        assert_eq!(
            read("probe --mode react --count 4"),
            Ok(Command::Probe(Probe {
                mode: ProbeMode::React,
                target: TargetKey::F19,
                count: 4,
                hold_ms: 30,
                gap_ms: 200,
            }))
        );
        assert_eq!(
            read("probe --mode fast"),
            Err(ArgError::BadValue("--mode", "fast".into()))
        );
    }
}

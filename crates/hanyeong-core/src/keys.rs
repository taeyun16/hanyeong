//! 한영 키로 쓸 수 있는 키와, 그 키를 바꿔 보낼 F-키.

/// HID 키보드 페이지(0x07)의 usage 를 `UserKeyMapping` 이 쓰는 64비트 값으로 만든다.
const fn usage(code: u64) -> u64 {
    0x7_0000_0000 | code
}

/// 한영 키로 삼을 물리 키.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SourceKey {
    RightCommand,
    RightOption,
    RightControl,
    CapsLock,
}

impl SourceKey {
    pub const ALL: [Self; 4] = [
        Self::RightCommand,
        Self::RightOption,
        Self::RightControl,
        Self::CapsLock,
    ];

    #[must_use]
    pub const fn usage(self) -> u64 {
        match self {
            Self::RightCommand => usage(0xE7),
            Self::RightOption => usage(0xE6),
            Self::RightControl => usage(0xE4),
            Self::CapsLock => usage(0x39),
        }
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::RightCommand => "right-command",
            Self::RightOption => "right-option",
            Self::RightControl => "right-control",
            Self::CapsLock => "caps-lock",
        }
    }

    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|key| key.name() == word)
    }
}

/// 한영 키를 바꿔 보낼 F-키. 시스템의 `이전 입력 소스 선택` 단축키도 이 키로 맞춘다.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TargetKey {
    F13,
    F14,
    F15,
    F16,
    F17,
    F18,
    F19,
    F20,
}

impl TargetKey {
    pub const ALL: [Self; 8] = [
        Self::F13,
        Self::F14,
        Self::F15,
        Self::F16,
        Self::F17,
        Self::F18,
        Self::F19,
        Self::F20,
    ];

    #[must_use]
    pub const fn usage(self) -> u64 {
        match self {
            Self::F13 => usage(0x68),
            Self::F14 => usage(0x69),
            Self::F15 => usage(0x6A),
            Self::F16 => usage(0x6B),
            Self::F17 => usage(0x6C),
            Self::F18 => usage(0x6D),
            Self::F19 => usage(0x6E),
            Self::F20 => usage(0x6F),
        }
    }

    /// macOS 의 가상 키 코드. 이벤트 탭과 단축키 설정이 이 값을 쓴다.
    #[must_use]
    pub const fn key_code(self) -> i64 {
        match self {
            Self::F13 => 105,
            Self::F14 => 107,
            Self::F15 => 113,
            Self::F16 => 106,
            Self::F17 => 64,
            Self::F18 => 79,
            Self::F19 => 80,
            Self::F20 => 90,
        }
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::F13 => "f13",
            Self::F14 => "f14",
            Self::F15 => "f15",
            Self::F16 => "f16",
            Self::F17 => "f17",
            Self::F18 => "f18",
            Self::F19 => "f19",
            Self::F20 => "f20",
        }
    }

    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|key| key.name() == word)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // gksdud 2.1.1 이 이 기기에 실제로 써 둔 값: 우측 ⌘ 30064771303 → F19 30064771182, 단축키 키 코드 80.
    #[test]
    fn right_command_and_f19_match_the_values_the_original_wrote() {
        assert_eq!(SourceKey::RightCommand.usage(), 30_064_771_303);
        assert_eq!(TargetKey::F19.usage(), 30_064_771_182);
        assert_eq!(TargetKey::F19.key_code(), 80);
    }

    #[test]
    fn every_name_parses_back_to_its_key() {
        for key in SourceKey::ALL {
            assert_eq!(SourceKey::parse(key.name()), Some(key));
        }
        for key in TargetKey::ALL {
            assert_eq!(TargetKey::parse(key.name()), Some(key));
        }
        assert_eq!(SourceKey::parse("f19"), None);
        assert_eq!(TargetKey::parse("F19"), None);
    }

    #[test]
    fn target_keys_are_consecutive_usages_with_distinct_key_codes() {
        for pair in TargetKey::ALL.windows(2) {
            assert_eq!(pair[0].usage() + 1, pair[1].usage());
        }
        let mut codes: Vec<i64> = TargetKey::ALL.iter().map(|key| key.key_code()).collect();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), TargetKey::ALL.len());
    }
}

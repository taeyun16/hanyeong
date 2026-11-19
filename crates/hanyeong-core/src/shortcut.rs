//! 시스템 단축키 60번(`이전 입력 소스 선택`)이 우리 F-키로 맞춰져 있는지, 바꾸고 되돌릴 때 무엇을
//! 쓸지 판단한다. 강제 종료를 섞은 순서는 `tests/shortcut60_replay.rs` 가 걷는다.

use crate::keys::TargetKey;

/// `com.apple.symbolichotkeys` 의 `AppleSymbolicHotKeys` 에서 `이전 입력 소스 선택`의 번호.
pub const PREVIOUS_SOURCE: &str = "60";
/// 문자가 없는 키(F-키)라는 표시. `parameters` 의 첫 값이다.
pub const NO_CHARACTER: i64 = 65_535;
/// F-키임을 나타내는 플래그. macOS 가 수정키 없는 F-키 단축키를 저장할 때 붙이기도 한다.
pub const SECONDARY_FN: i64 = 0x80_0000;

/// 단축키 항목 하나에서 판단에 쓰는 값.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Entry {
    pub enabled: bool,
    /// `value.type` 이 `standard` 인지.
    pub standard: bool,
    /// `value.parameters`: 문자, 키 코드, 수정키.
    pub parameters: [i64; 3],
}

/// 우리가 쓸 `parameters`.
#[must_use]
pub const fn wanted(key_code: i64) -> [i64; 3] {
    [NO_CHARACTER, key_code, 0]
}

/// 이 항목이 수정키 없이 `key_code` 하나로 켜져 있는가.
#[must_use]
pub fn owns(entry: Option<&Entry>, key_code: i64) -> bool {
    let Some(entry) = entry else {
        return false;
    };
    let [character, code, modifiers] = entry.parameters;
    entry.enabled
        && entry.standard
        && character == NO_CHARACTER
        && code == key_code
        && (modifiers == 0 || modifiers == SECONDARY_FN)
}

/// 우리가 60번에 써 넣었을 수 있는 F-키들. 원래 값과 함께 한 값으로 남긴다.
///
/// 60번의 값이 우리가 쓴 것인지는 이번 실행의 `--target` 이 아니라 이 기록으로 판단한다. 그래야
/// 다른 F-키로 띄웠던 실행이 남긴 값도 알아본다. 비어 있으면 남긴 것이 없다는 뜻이다.
///
/// 고를 수 있는 F-키(`TargetKey::ALL`)마다 비트 하나다. 그 밖의 키는 담지 않는다.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Written(u8);

impl Written {
    /// 남긴 것이 없다.
    #[must_use]
    pub const fn none() -> Self {
        Self(0)
    }

    /// `key_code` 하나만 써 넣었다.
    #[must_use]
    pub fn only(key_code: i64) -> Self {
        Self::none().with(key_code)
    }

    /// 표시와 원래 값을 따로 남기던 옛 형식의 기록. 어느 F-키를 썼는지 적지 않았으므로 고를 수 있는
    /// F-키 모두를 우리 것으로 본다.
    #[must_use]
    pub const fn every_target() -> Self {
        Self(u8::MAX)
    }

    fn bits() -> impl Iterator<Item = (u8, i64)> {
        (0_u32..)
            .zip(TargetKey::ALL)
            .map(|(index, key)| (1_u8.wrapping_shl(index), key.key_code()))
    }

    /// `key_code` 를 더한 기록. 고를 수 있는 F-키가 아니면 그대로다.
    #[must_use]
    pub fn with(self, key_code: i64) -> Self {
        let bit = Self::bits()
            .find(|(_, code)| *code == key_code)
            .map_or(0, |(bit, _)| bit);
        Self(self.0 | bit)
    }

    /// 담긴 F-키들의 키 코드.
    pub fn keys(self) -> impl Iterator<Item = i64> {
        Self::bits()
            .filter(move |(bit, _)| self.0 & bit != 0)
            .map(|(_, code)| code)
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// 이 항목이 우리가 써 넣었을 수 있는 값인가.
    #[must_use]
    pub fn holds(self, entry: Option<&Entry>) -> bool {
        self.keys().any(|key_code| owns(entry, key_code))
    }
}

/// `claim` 이 할 일.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Claim {
    /// 이미 이번 F-키다. 아무것도 쓰지 않는다.
    Already,
    /// 기록을 `written` 으로 남긴 다음 60번을 이번 F-키로 쓴다. `fresh` 면 지금의 60번 값을 원래
    /// 값으로 남기고, 아니면 남아 있던 원래 값을 그대로 둔다.
    Write { written: Written, fresh: bool },
}

/// 60번을 `key_code` 로 맞추려면 무엇을 써야 하는가. `record` 는 남아 있는 기록이다.
///
/// 60번이 우리가 써 넣은 값이 아니면 남아 있던 기록을 믿지 않고 새로 남긴다: 강제 종료 뒤에
/// 사용자가 60번을 다시 골랐으면 그 값이 되돌릴 값이다. 60번이 아직 우리 값이면 원래 값은 두고
/// 이번 F-키만 더한다: 그 값을 원래 값으로 남기면 되돌릴 곳을 잃는다.
#[must_use]
pub fn claim(current: Option<&Entry>, record: Written, key_code: i64) -> Claim {
    if owns(current, key_code) {
        Claim::Already
    } else if record.holds(current) {
        Claim::Write {
            written: record.with(key_code),
            fresh: false,
        }
    } else {
        Claim::Write {
            written: Written::only(key_code),
            fresh: true,
        }
    }
}

/// `release` 가 할 일.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Release {
    /// 우리가 바꾼 적이 없다. 아무것도 쓰지 않는다.
    Untouched,
    /// 60번이 우리가 써 넣은 값이다. 원래 값을 쓰고, 기록을 지운다.
    Restore,
    /// 그 뒤에 누군가 바꿨다. 기록만 지운다.
    Forget,
}

/// 끝날 때 60번을 어떻게 할 것인가.
#[must_use]
pub fn release(current: Option<&Entry>, record: Written) -> Release {
    if record.is_empty() {
        Release::Untouched
    } else if record.holds(current) {
        Release::Restore
    } else {
        Release::Forget
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const F19: i64 = 80;

    fn entry(parameters: [i64; 3]) -> Entry {
        Entry {
            enabled: true,
            standard: true,
            parameters,
        }
    }

    #[test]
    fn the_entry_we_write_is_ours_with_or_without_the_function_key_flag() {
        assert!(owns(Some(&entry(wanted(F19))), F19));
        assert!(owns(Some(&entry([NO_CHARACTER, F19, SECONDARY_FN])), F19));
    }

    // 이 기기에서 gksdud 가 바꾸기 전의 값: ⌃ + Space (32, 49, 262144).
    #[test]
    fn control_space_and_real_modifiers_are_not_ours() {
        assert!(!owns(Some(&entry([32, 49, 262_144])), F19));
        // ⌘ + F19 는 한영 키 하나로 눌리지 않는다.
        assert!(!owns(Some(&entry([NO_CHARACTER, F19, 1_048_576])), F19));
        assert!(!owns(Some(&entry(wanted(79))), F19));
    }

    #[test]
    fn a_disabled_missing_or_non_standard_entry_is_not_ours() {
        assert!(!owns(None, F19));
        assert!(!owns(
            Some(&Entry {
                enabled: false,
                ..entry(wanted(F19))
            }),
            F19
        ));
        assert!(!owns(
            Some(&Entry {
                standard: false,
                ..entry(wanted(F19))
            }),
            F19
        ));
    }

    #[test]
    fn a_record_holds_the_keys_it_was_given_and_only_keys_we_can_write() {
        let written = Written::only(F19).with(79).with(F19);
        assert_eq!(written.keys().collect::<Vec<_>>(), [79, F19]);
        assert!(written.holds(Some(&entry(wanted(79)))));
        assert!(!written.holds(Some(&entry(wanted(64)))));
        assert!(!Written::none().holds(Some(&entry(wanted(F19)))));
        // 고를 수 있는 F-키가 아니면 담지 않는다(Space, 49).
        assert!(Written::only(49).is_empty());
        // 옛 형식의 기록은 고를 수 있는 F-키를 빠짐없이 담는다.
        let every = Written::every_target();
        assert_eq!(every.keys().count(), TargetKey::ALL.len());
        assert!(
            TargetKey::ALL
                .iter()
                .all(|key| every.holds(Some(&entry(wanted(key.key_code())))))
        );
        assert!(!every.holds(Some(&entry([32, 49, 262_144]))));
    }
}

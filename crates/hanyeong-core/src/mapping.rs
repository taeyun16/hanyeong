//! 키보드 하나의 `UserKeyMapping` 목록을 어떻게 고칠지 정한다.
//!
//! 목록에는 다른 도구나 사용자가 넣은 매핑도 들어 있다. 우리는 한영 키 하나의 매핑만 더하고,
//! 걷어낼 때는 우리가 쓴 그 매핑만 걷어낸다.

/// 한 키보드에서 다루는 매핑의 상한. 넘으면 손대지 않는다.
pub const MAX_MAPPINGS: usize = 64;

/// 키 하나를 다른 키로 바꾸는 매핑. 값은 HID usage 다.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Pair {
    pub src: u64,
    pub dst: u64,
}

#[derive(PartialEq, Eq, Debug)]
pub enum Change {
    /// 이미 원하는 상태다. 쓰지 않는다.
    Keep,
    /// 이 목록으로 바꿔 쓴다. `displaced` 는 같은 키에 걸려 있던 남의 매핑의 대상이다.
    Write {
        list: Vec<Pair>,
        displaced: Option<u64>,
    },
    /// 목록이 상한을 넘어 손대지 않는다.
    TooMany,
}

/// `want` 를 목록에 넣는다. 같은 키의 다른 매핑은 밀어내고 그 대상을 돌려준다.
#[must_use]
pub fn apply(current: &[Pair], want: Pair) -> Change {
    if current.len() > MAX_MAPPINGS {
        return Change::TooMany;
    }
    if current.contains(&want) {
        return Change::Keep;
    }
    let mut list = Vec::with_capacity(current.len().saturating_add(1));
    let mut displaced = None;
    for pair in current {
        if pair.src == want.src {
            displaced = Some(pair.dst);
        } else {
            list.push(*pair);
        }
    }
    list.push(want);
    Change::Write { list, displaced }
}

/// 우리가 넣은 `ours` 를 걷어내고, 밀어냈던 매핑이 있으면 되돌린다.
///
/// 목록에 `ours` 가 그대로 없으면 그 뒤에 누군가 바꾼 것이므로 손대지 않는다.
#[must_use]
pub fn remove(current: &[Pair], ours: Pair, displaced: Option<u64>) -> Change {
    if current.len() > MAX_MAPPINGS {
        return Change::TooMany;
    }
    if !current.contains(&ours) {
        return Change::Keep;
    }
    let mut list = Vec::with_capacity(current.len());
    for pair in current {
        if *pair != ours {
            list.push(*pair);
        }
    }
    if let Some(dst) = displaced {
        list.push(Pair { src: ours.src, dst });
    }
    Change::Write {
        list,
        displaced: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RIGHT_COMMAND: u64 = 0x7_0000_00E7;
    const F19: u64 = 0x7_0000_006E;
    const F18: u64 = 0x7_0000_006D;
    const CAPS: u64 = 0x7_0000_0039;
    const ESCAPE: u64 = 0x7_0000_0029;
    const WANT: Pair = Pair {
        src: RIGHT_COMMAND,
        dst: F19,
    };
    const OTHERS: Pair = Pair {
        src: CAPS,
        dst: ESCAPE,
    };

    #[test]
    fn applying_keeps_other_mappings_and_adds_ours() {
        assert_eq!(
            apply(&[OTHERS], WANT),
            Change::Write {
                list: vec![OTHERS, WANT],
                displaced: None
            }
        );
        assert_eq!(
            apply(&[], WANT),
            Change::Write {
                list: vec![WANT],
                displaced: None
            }
        );
    }

    #[test]
    fn applying_twice_writes_nothing_the_second_time() {
        assert_eq!(apply(&[OTHERS, WANT], WANT), Change::Keep);
    }

    #[test]
    fn a_mapping_already_on_our_key_is_displaced_and_comes_back_on_removal() {
        let theirs = Pair {
            src: RIGHT_COMMAND,
            dst: F18,
        };
        let Change::Write { list, displaced } = apply(&[theirs, OTHERS], WANT) else {
            panic!("expected a write");
        };
        assert_eq!(list, vec![OTHERS, WANT]);
        assert_eq!(displaced, Some(F18));
        assert_eq!(
            remove(&list, WANT, displaced),
            Change::Write {
                list: vec![OTHERS, theirs],
                displaced: None
            }
        );
    }

    #[test]
    fn removing_takes_only_our_mapping() {
        assert_eq!(
            remove(&[OTHERS, WANT], WANT, None),
            Change::Write {
                list: vec![OTHERS],
                displaced: None
            }
        );
    }

    #[test]
    fn removing_leaves_a_list_someone_changed_since() {
        let changed = Pair {
            src: RIGHT_COMMAND,
            dst: F18,
        };
        assert_eq!(remove(&[OTHERS, changed], WANT, None), Change::Keep);
        assert_eq!(remove(&[], WANT, Some(F18)), Change::Keep);
    }

    #[test]
    fn a_list_over_the_limit_is_left_alone() {
        let many: Vec<Pair> = (0..=MAX_MAPPINGS as u64)
            .map(|index| Pair {
                src: index,
                dst: index + 1,
            })
            .collect();
        assert_eq!(apply(&many, WANT), Change::TooMany);
        assert_eq!(remove(&many, WANT, None), Change::TooMany);
        assert!(matches!(
            apply(&many[..MAX_MAPPINGS], WANT),
            Change::Write { .. }
        ));
    }
}

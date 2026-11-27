//! 보안 입력(Secure Event Input)이 켜지고 꺼지는 것을 언제 알릴지 정한다.
//!
//! 암호 입력란 같은 곳에서 어떤 앱이 보안 입력을 켜면, 그동안 이벤트 탭에는 키가 오지 않는다. 한영 키는
//! hanyeong 을 거치지 않고 시스템 단축키가 직접 받아 뗄 때 전환하고, 기기가 바쁘면 누름을 잃는다
//! (2026-11-27 실기기: Slack 이 켜 둔 동안 사용자가 40번 눌렀고 hanyeong 의 전환은 0번이었다). 우리가
//! 풀 수는 없으므로, 켜진 것과 그 주인을 알린다.

/// 보안 입력의 지금 상태.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Secure {
    Off,
    /// 켜져 있다. `pid` 는 켠 프로세스이고, 알 수 없으면 0 이다.
    On {
        pid: i64,
    },
}

/// 남길 말.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Notice {
    /// 켜졌거나, 켠 프로세스가 바뀌었다.
    On { pid: i64 },
    /// 풀렸다.
    Off,
}

/// 마지막으로 본 상태를 기억해, 달라졌을 때만 알린다.
#[derive(Debug)]
pub struct SecureWatch {
    last: Secure,
}

impl Default for SecureWatch {
    fn default() -> Self {
        Self::new()
    }
}

impl SecureWatch {
    #[must_use]
    pub const fn new() -> Self {
        Self { last: Secure::Off }
    }

    /// 지금 상태를 넣는다. 처음 본 상태가 켜짐이면 그것도 알린다.
    pub fn observe(&mut self, now: Secure) -> Option<Notice> {
        let before = self.last;
        self.last = now;
        match (before, now) {
            (Secure::Off, Secure::Off) => None,
            (Secure::On { .. }, Secure::Off) => Some(Notice::Off),
            (Secure::On { pid: was }, Secure::On { pid }) if was == pid => None,
            (_, Secure::On { pid }) => Some(Notice::On { pid }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2026-11-27 실기기에서 본 주인들: Slack 2217, 그 뒤 Claude 27092.
    const SLACK: i64 = 2217;
    const CLAUDE: i64 = 27092;

    #[test]
    fn nothing_is_said_while_it_stays_off() {
        let mut watch = SecureWatch::new();
        assert_eq!(watch.observe(Secure::Off), None);
        assert_eq!(watch.observe(Secure::Off), None);
    }

    #[test]
    fn already_on_at_start_is_said_once() {
        let mut watch = SecureWatch::new();
        assert_eq!(
            watch.observe(Secure::On { pid: SLACK }),
            Some(Notice::On { pid: SLACK })
        );
        assert_eq!(watch.observe(Secure::On { pid: SLACK }), None);
    }

    #[test]
    fn a_new_owner_is_said_and_the_release_is_said() {
        let mut watch = SecureWatch::new();
        watch.observe(Secure::On { pid: SLACK });
        assert_eq!(
            watch.observe(Secure::On { pid: CLAUDE }),
            Some(Notice::On { pid: CLAUDE })
        );
        assert_eq!(watch.observe(Secure::Off), Some(Notice::Off));
        assert_eq!(watch.observe(Secure::Off), None);
        assert_eq!(
            watch.observe(Secure::On { pid: SLACK }),
            Some(Notice::On { pid: SLACK })
        );
    }

    #[test]
    fn an_unknown_owner_is_still_on() {
        let mut watch = SecureWatch::new();
        assert_eq!(
            watch.observe(Secure::On { pid: 0 }),
            Some(Notice::On { pid: 0 })
        );
        assert_eq!(watch.observe(Secure::On { pid: 0 }), None);
    }
}

//! 키 이벤트 하나를 받아 통과시킬지, 막을지, 전환할지 정한다.
//!
//! macOS 의 `이전 입력 소스 선택` 단축키는 키를 뗄 때 전환한다. 빠르게 치면 한 글자가 이전
//! 입력 소스로 들어간다. 그래서 대상 F-키가 눌리는 순간 그 누름을 막고, 같은 F-키의 누름·뗌
//! 한 쌍을 곧바로 다시 보내 그 자리에서 전환되게 한다. 그 한 쌍은 우리 표시를 달고 돌아오므로
//! 통과시킨다.

/// 전환이 이 횟수만큼 `STORM_WINDOW_NS` 안에 몰리면 사람이 누른 것이 아니다.
pub const STORM_LIMIT: usize = 6;
/// 폭주를 판단하는 구간. 사람은 60ms 안에 한영 키를 여섯 번 누르지 못한다.
pub const STORM_WINDOW_NS: u64 = 60_000_000;
/// 폭주를 본 뒤 대상 키에 손대지 않는 시간.
pub const STORM_MUTE_NS: u64 = 2_000_000_000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Down,
    Up,
}

/// 이벤트 탭이 본 키 이벤트 하나.
#[derive(Clone, Copy, Debug)]
pub struct Key {
    pub kind: Kind,
    /// 가상 키 코드.
    pub code: i64,
    /// 누르고 있어서 반복된 누름인지.
    pub repeat: bool,
    /// 우리가 다시 보낸 이벤트인지(표시 값이 일치).
    pub ours: bool,
    /// 이 이벤트가 탭에 도착한 시각(단조 시계). 폭주 판단에만 쓴다.
    pub now_ns: u64,
    /// 이 이벤트가 만들어진 시각. 합성 이벤트에서는 단위가 다르거나 0 일 수 있어, 도착 시각과
    /// 함께일 때만 폭주의 근거로 쓴다.
    pub created_ns: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Decision {
    /// 그대로 다음으로 넘긴다.
    Pass,
    /// 없앤다.
    Swallow,
    /// 없애고, 대상 F-키의 누름·뗌 한 쌍을 보낸다.
    Switch,
}

/// 받은 이벤트가 우리가 다시 보낸 것인가.
///
/// 사용자 데이터의 표시만으로는 모자라다: 다른 프로그램이 합성해 보낸 키의 복사본을 다시 보내면 그
/// 프로그램의 표시와 프로세스 번호가 남았다(2026-11-16 실기기, 한 번의 누름에 5번 전환하고 폭주로
/// 막혔다). 그래서 우리 이벤트 소스로 새 이벤트를 만들어 보내고, 보낸 프로세스 번호로도 알아본다.
/// 하드웨어 키는 번호가 0 이다.
#[must_use]
pub const fn is_ours(user_data: i64, marker: i64, source_pid: i64, own_pid: i64) -> bool {
    user_data == marker || (own_pid != 0 && source_pid == own_pid)
}

/// 대상 F-키 하나의 누름 상태.
#[derive(Debug)]
pub struct PressGate {
    target: i64,
    held: bool,
    /// 최근 전환의 (도착 시각, 만든 시각). 고리 모양으로 덮어쓴다.
    recent: [(u64, u64); STORM_LIMIT - 1],
    next: usize,
    switches: u64,
    muted_until: u64,
    storms: u64,
}

impl PressGate {
    #[must_use]
    pub const fn new(target: i64) -> Self {
        Self {
            target,
            held: false,
            recent: [(0, 0); STORM_LIMIT - 1],
            next: 0,
            switches: 0,
            muted_until: 0,
            storms: 0,
        }
    }

    #[must_use]
    pub const fn target(&self) -> i64 {
        self.target
    }

    /// 지금까지 낸 전환 수.
    #[must_use]
    pub const fn switches(&self) -> u64 {
        self.switches
    }

    /// 지금까지 본 폭주 수.
    #[must_use]
    pub const fn storms(&self) -> u64 {
        self.storms
    }

    /// 이벤트 탭이 꺼졌다 켜졌을 때. 그 사이의 뗌을 놓쳤을 수 있다.
    pub const fn reset(&mut self) {
        self.held = false;
    }

    pub fn decide(&mut self, key: Key) -> Decision {
        if key.ours || key.code != self.target {
            return Decision::Pass;
        }
        if key.now_ns < self.muted_until {
            self.held = false;
            return Decision::Pass;
        }
        match key.kind {
            Kind::Down if key.repeat || self.held => Decision::Swallow,
            Kind::Down => {
                if self.storm(&key) {
                    self.storms = self.storms.saturating_add(1);
                    self.muted_until = key.now_ns.saturating_add(STORM_MUTE_NS);
                    return Decision::Pass;
                }
                self.held = true;
                self.switches = self.switches.saturating_add(1);
                Decision::Switch
            }
            Kind::Up if self.held => {
                self.held = false;
                Decision::Swallow
            }
            Kind::Up => Decision::Pass,
        }
    }

    /// 이번 전환까지 넣어 `STORM_LIMIT` 번이 `STORM_WINDOW_NS` 안에 들어오면 참.
    ///
    /// 같은 수법을 쓰는 다른 도구가 함께 돌면, 서로가 보낸 한 쌍을 서로 새 누름으로 보고 끝없이
    /// 주고받는다. 그때 대상 키를 그냥 통과시키면 고리가 끊긴다.
    ///
    /// 도착 시각과 만든 시각이 둘 다 구간 안이어야 한다. 도착 시각만 보면, 기기가 바빠 밀렸다가
    /// 한꺼번에 도착한 사람의 누름이 폭주로 보인다(그 누름들은 만든 시각이 떨어져 있다).
    /// 만든 시각만 보면, 그 값이 0 이거나 단위가 작은 합성 이벤트가 폭주로 보인다. 주고받기에서는
    /// 본뜬 이벤트라 만든 시각이 그대로이고 도착도 몰려 있어 둘 다 구간 안이다.
    fn storm(&mut self, key: &Key) -> bool {
        let Some(slot) = self.recent.get_mut(self.next) else {
            return false;
        };
        // 이 칸에는 `STORM_LIMIT - 1` 번 전의 전환 시각이 들어 있다.
        let (arrived, created) = *slot;
        *slot = (key.now_ns, key.created_ns);
        let following = self.next.saturating_add(1);
        self.next = if following >= self.recent.len() {
            0
        } else {
            following
        };
        self.switches >= self.recent.len() as u64
            && key.now_ns.saturating_sub(arrived) < STORM_WINDOW_NS
            && key.created_ns.abs_diff(created) < STORM_WINDOW_NS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const F19: i64 = 80;
    const MS: u64 = 1_000_000;

    fn key(kind: Kind, code: i64, at_ms: u64) -> Key {
        Key {
            kind,
            code,
            repeat: false,
            ours: false,
            now_ns: at_ms * MS,
            created_ns: at_ms * MS,
        }
    }

    #[test]
    fn a_press_switches_once_and_its_repeats_and_release_are_swallowed() {
        let mut gate = PressGate::new(F19);
        assert_eq!(gate.decide(key(Kind::Down, F19, 1000)), Decision::Switch);
        let repeat = Key {
            repeat: true,
            ..key(Kind::Down, F19, 1500)
        };
        assert_eq!(gate.decide(repeat), Decision::Swallow);
        assert_eq!(gate.decide(repeat), Decision::Swallow);
        assert_eq!(gate.decide(key(Kind::Up, F19, 1600)), Decision::Swallow);
        assert_eq!(gate.switches(), 1);
        // 뗀 뒤의 다음 누름은 다시 전환한다.
        assert_eq!(gate.decide(key(Kind::Down, F19, 2000)), Decision::Switch);
        assert_eq!(gate.switches(), 2);
    }

    #[test]
    fn our_own_pulse_and_other_keys_pass_untouched() {
        let mut gate = PressGate::new(F19);
        assert_eq!(gate.decide(key(Kind::Down, F19, 1000)), Decision::Switch);
        let ours_down = Key {
            ours: true,
            ..key(Kind::Down, F19, 1000)
        };
        let ours_up = Key {
            ours: true,
            ..key(Kind::Up, F19, 1000)
        };
        assert_eq!(gate.decide(ours_down), Decision::Pass);
        assert_eq!(gate.decide(ours_up), Decision::Pass);
        // 우리 한 쌍의 뗌이 실제 키의 눌림 상태를 풀면 안 된다.
        assert_eq!(gate.decide(key(Kind::Up, F19, 1100)), Decision::Swallow);
        assert_eq!(gate.decide(key(Kind::Down, 0, 1200)), Decision::Pass);
        assert_eq!(gate.decide(key(Kind::Up, 0, 1250)), Decision::Pass);
        assert_eq!(gate.switches(), 1);
    }

    #[test]
    fn a_second_press_without_a_release_does_not_switch_again() {
        let mut gate = PressGate::new(F19);
        assert_eq!(gate.decide(key(Kind::Down, F19, 1000)), Decision::Switch);
        assert_eq!(gate.decide(key(Kind::Down, F19, 1300)), Decision::Swallow);
        assert_eq!(gate.switches(), 1);
    }

    #[test]
    fn a_release_that_was_never_pressed_passes_and_reset_forgets_a_held_key() {
        let mut gate = PressGate::new(F19);
        assert_eq!(gate.decide(key(Kind::Up, F19, 1000)), Decision::Pass);
        assert_eq!(gate.decide(key(Kind::Down, F19, 2000)), Decision::Switch);
        gate.reset();
        // 탭이 꺼진 동안 뗌이 지나갔다. 다음 누름은 새 누름이다.
        assert_eq!(gate.decide(key(Kind::Down, F19, 3000)), Decision::Switch);
    }

    #[test]
    fn fast_human_presses_never_count_as_a_storm() {
        let mut gate = PressGate::new(F19);
        // 초당 25번: 사람이 낼 수 있는 것보다 빠르다.
        for press in 0..200_u64 {
            let at = 1000 + press * 40;
            assert_eq!(gate.decide(key(Kind::Down, F19, at)), Decision::Switch);
            assert_eq!(gate.decide(key(Kind::Up, F19, at + 10)), Decision::Swallow);
        }
        assert_eq!(gate.storms(), 0);
    }

    #[test]
    fn a_ping_pong_with_another_tool_is_cut_after_the_limit_and_recovers() {
        let mut gate = PressGate::new(F19);
        let mut decisions = Vec::new();
        // 다른 도구가 우리 한 쌍을 받아 1ms 마다 새 누름·뗌으로 되돌려 보낸다.
        for round in 0..20_u64 {
            let at = 1000 + round;
            decisions.push(gate.decide(key(Kind::Down, F19, at)));
            gate.decide(key(Kind::Up, F19, at));
        }
        let switched = decisions
            .iter()
            .filter(|decision| **decision == Decision::Switch)
            .count();
        assert_eq!(switched, STORM_LIMIT - 1);
        assert_eq!(gate.storms(), 1);
        assert!(
            decisions[STORM_LIMIT - 1..]
                .iter()
                .all(|decision| *decision == Decision::Pass)
        );
        // 잠잠해진 뒤에는 다시 전환한다.
        let later = 1020 + STORM_MUTE_NS / MS + 1;
        assert_eq!(gate.decide(key(Kind::Down, F19, later)), Decision::Switch);
    }

    // 2026-11-16 실기기: 부하 평균 50~84 에서 250~400ms 간격의 누름이 밀렸다가 한꺼번에 도착해
    // `폭주 차단 1번`이 나왔다. 만들어진 시각은 멀리 떨어져 있었다.
    #[test]
    fn presses_that_were_delayed_and_arrive_together_are_not_a_storm() {
        let mut gate = PressGate::new(F19);
        for press in 0..12_u64 {
            let down = Key {
                now_ns: (5000 + press) * MS,
                ..key(Kind::Down, F19, 1000 + press * 250)
            };
            assert_eq!(gate.decide(down), Decision::Switch);
            let up = Key {
                kind: Kind::Up,
                ..down
            };
            assert_eq!(gate.decide(up), Decision::Swallow);
        }
        assert_eq!(gate.storms(), 0);
    }

    // 합성 이벤트의 만든 시각은 단위가 작거나(틱) 0 이다. 도착이 400ms 간격이면 폭주가 아니다.
    #[test]
    fn synthetic_presses_with_unusable_creation_times_are_not_a_storm() {
        for created_step in [0_u64, 9_600_000] {
            let mut gate = PressGate::new(F19);
            for press in 0..20_u64 {
                let down = Key {
                    created_ns: press * created_step,
                    ..key(Kind::Down, F19, 1000 + press * 400)
                };
                assert_eq!(gate.decide(down), Decision::Switch);
                let up = Key {
                    kind: Kind::Up,
                    ..down
                };
                assert_eq!(gate.decide(up), Decision::Swallow);
            }
            assert_eq!(gate.storms(), 0);
        }
    }

    // 주고받기에서는 상대가 우리 한 쌍을 본떠 보내므로 만든 시각이 그대로다. 그래도 끊어야 한다.
    #[test]
    fn a_ping_pong_of_copies_that_keep_one_creation_time_is_still_cut() {
        let mut gate = PressGate::new(F19);
        let mut switched = 0;
        for round in 0..20_u64 {
            let down = Key {
                created_ns: 1000 * MS,
                ..key(Kind::Down, F19, 1000 + round)
            };
            if gate.decide(down) == Decision::Switch {
                switched += 1;
            }
            gate.decide(Key {
                kind: Kind::Up,
                ..down
            });
        }
        assert_eq!(switched, STORM_LIMIT - 1);
        assert_eq!(gate.storms(), 1);
    }

    const MARKER: i64 = 0x5EED;
    const OWN: i64 = 4242;
    const INJECTOR: i64 = 777;

    #[test]
    fn our_pulse_is_ours_even_when_its_marker_was_lost() {
        // 다른 프로그램이 표시를 달아 보낸 키를 본뜬 누름: 표시는 그 프로그램의 것이지만 보낸 것은 우리다.
        assert!(is_ours(91, MARKER, OWN, OWN));
        assert!(is_ours(MARKER, MARKER, 0, OWN));
    }

    #[test]
    fn hardware_keys_and_other_programs_keys_are_not_ours() {
        assert!(!is_ours(0, MARKER, 0, OWN));
        assert!(!is_ours(91, MARKER, INJECTOR, OWN));
        // 번호를 모르면 번호로는 판단하지 않는다.
        assert!(!is_ours(0, MARKER, 0, 0));
    }

    // 실기기에서 본 주고받기: 우리 누름을 남의 것으로 보면 한 번의 누름이 여러 번의 전환이 된다.
    #[test]
    fn recognising_our_pulse_keeps_one_press_one_switch() {
        let mut gate = PressGate::new(F19);
        let injected = key(Kind::Down, F19, 1000);
        assert_eq!(gate.decide(injected), Decision::Switch);
        for _ in 0..6 {
            let pulse = Key {
                ours: is_ours(91, MARKER, OWN, OWN),
                ..key(Kind::Down, F19, 1000)
            };
            assert_eq!(gate.decide(pulse), Decision::Pass);
            assert_eq!(
                gate.decide(Key {
                    kind: Kind::Up,
                    ..pulse
                }),
                Decision::Pass
            );
        }
        assert_eq!(gate.switches(), 1);
        assert_eq!(gate.storms(), 0);
    }
}

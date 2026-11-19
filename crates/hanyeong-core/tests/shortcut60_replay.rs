//! `formal/models/Shortcut60.tla` 의 길을 실제 판단 함수(`shortcut::claim`, `shortcut::release`)로
//! 한 걸음씩 다시 걷는다.
//!
//! 걸음의 이름은 모델의 동작 이름과 같고, 걸음마다 모델의 규칙을 확인한다. 설정을 쓰는 순서(기록
//! 다음에 60번, 60번 다음에 지우기)는 `crates/hanyeong-mac/src/hotkey.rs` 의 것을 옮겨 적었다. 그
//! 파일이 바뀌면 `formal/check.py` 가 다시 맞춰 보라고 멈춘다.
//!
//! `finding_` 으로 시작하는 테스트는 2026-11-17 에 모델이 찾아 고친 길이다(`formal/README.md`).
//! `formal/check.py --replay` 는 고친 곳을 하나씩 되돌려 이 테스트들이 실패하는지 확인한다.

use hanyeong_core::shortcut::{self, Claim, Entry, NO_CHARACTER, Release, Written};

const F19: i64 = 80;
const F18: i64 = 79;

const fn entry(parameters: [i64; 3]) -> Entry {
    Entry {
        enabled: true,
        standard: true,
        parameters,
    }
}

/// 이 기기의 원래 값: ⌃ + Space.
const CTRL_SPACE: Entry = entry([32, 49, 262_144]);
/// 사용자가 나중에 고른 값: ⌥ + Space.
const CUSTOM: Entry = entry([32, 49, 524_288]);

const fn ours(key_code: i64) -> Entry {
    entry([NO_CHARACTER, key_code, 0])
}

/// 모델의 변수들. 60번, 우리 설정 도메인의 기록(써 넣은 키들과 원래 값), 그리고 유령 변수.
struct Machine {
    sys: Option<Entry>,
    written: Written,
    original: Option<Entry>,
    /// 남이 마지막으로 정한 값. hanyeong 이 없었다면 60번에 있을 값이다.
    truth: Option<Entry>,
    /// 지금 60번의 값을 우리가 써 넣었다.
    by_us: bool,
}

impl Machine {
    const fn new(value: Entry) -> Self {
        Self {
            sys: Some(value),
            written: Written::none(),
            original: None,
            truth: Some(value),
            by_us: false,
        }
    }

    /// 옛 형식(표시와 원래 값)을 남긴 빌드가 60번을 `key_code` 로 써 둔 채 죽은 상태.
    fn left_by_the_old_format(original: Entry, key_code: i64) -> Self {
        Self {
            sys: Some(ours(key_code)),
            written: Written::every_target(),
            original: Some(original),
            truth: Some(original),
            by_us: true,
        }
    }

    /// 모델의 규칙들.
    fn check(&self) {
        if !self.by_us {
            assert_eq!(
                self.sys, self.truth,
                "OursOrTheirs: 60번에 우리 값도 남이 정한 값도 아닌 것이 있다"
            );
        }
        if self.by_us {
            assert!(
                !self.written.is_empty(),
                "KeptWhileOurs: 우리 값이 60번에 있는데 되돌릴 값이 없다"
            );
            assert_eq!(
                self.original, self.truth,
                "KeptIsLatest: 남긴 값이 덮어쓰기 직전의 값이 아니다"
            );
        }
    }

    fn claim_read(&self, key_code: i64) -> Claim {
        shortcut::claim(self.sys.as_ref(), self.written, key_code)
    }

    /// 기록을 한 번의 쓰기로 남긴다.
    fn keep_record(&mut self, plan: Claim, seen: Option<Entry>) {
        if let Claim::Write { written, fresh } = plan {
            self.written = written;
            if fresh {
                self.original = seen;
            }
        }
        self.check();
    }

    fn claim_write(&mut self, key_code: i64) {
        self.sys = Some(ours(key_code));
        self.by_us = true;
        self.check();
    }

    /// `hotkey::claim` 한 번을 끝까지.
    fn claim(&mut self, key_code: i64) -> Claim {
        let seen = self.sys;
        let plan = self.claim_read(key_code);
        if plan != Claim::Already {
            self.keep_record(plan, seen);
            self.claim_write(key_code);
        }
        plan
    }

    /// `hotkey::release` 한 번을 끝까지.
    fn release(&mut self) -> Release {
        let plan = shortcut::release(self.sys.as_ref(), self.written);
        match plan {
            Release::Untouched => {}
            Release::Restore => {
                self.sys = self.original;
                self.by_us = false;
                self.check();
                self.forget_record();
            }
            Release::Forget => self.forget_record(),
        }
        plan
    }

    fn forget_record(&mut self) {
        self.written = Written::none();
        self.original = None;
        self.check();
    }

    /// 프로세스가 사라진다. 설정에 쓴 것만 남는다.
    fn crash(&self) {
        self.check();
    }

    fn other_writes(&mut self, value: Entry) {
        self.sys = Some(value);
        self.truth = Some(value);
        self.by_us = false;
        self.check();
    }
}

#[test]
fn a_clean_run_puts_the_original_back() {
    let mut machine = Machine::new(CTRL_SPACE);
    assert_eq!(
        machine.claim(F19),
        Claim::Write {
            written: Written::only(F19),
            fresh: true
        }
    );
    assert_eq!(machine.sys, Some(ours(F19)));
    assert_eq!(machine.release(), Release::Restore);
    assert_eq!(machine.sys, Some(CTRL_SPACE));
    assert!(machine.written.is_empty());
}

#[test]
fn a_restart_after_a_crash_still_puts_the_original_back() {
    let mut machine = Machine::new(CTRL_SPACE);
    machine.claim(F19);
    machine.crash();
    // 60번이 아직 우리 값이다. 그것을 원래 값으로 남기면 안 된다.
    assert_eq!(machine.claim(F19), Claim::Already);
    assert_eq!(machine.release(), Release::Restore);
    assert_eq!(machine.sys, Some(CTRL_SPACE));
}

#[test]
fn restore_after_a_crash_puts_the_original_back() {
    let mut machine = Machine::new(CTRL_SPACE);
    machine.claim(F19);
    machine.crash();
    assert_eq!(machine.release(), Release::Restore);
    assert_eq!(machine.sys, Some(CTRL_SPACE));
    // 되돌릴 것이 없으면 손대지 않는다.
    assert_eq!(machine.release(), Release::Untouched);
}

#[test]
fn a_value_someone_set_while_we_ran_is_left_alone() {
    let mut machine = Machine::new(CTRL_SPACE);
    machine.claim(F19);
    machine.other_writes(CUSTOM);
    assert_eq!(machine.release(), Release::Forget);
    assert_eq!(machine.sys, Some(CUSTOM));
    assert!(machine.written.is_empty());
}

// 발견 1. 모델의 길: Start, ClaimRead, 기록을 남김, Crash, OtherWrites, Start, ClaimRead, 기록을
// 남김, ClaimWrite. 고치기 전에는 남아 있던 표시를 믿고 낡은 원래 값을 그대로 두었다.
#[test]
fn finding_1_a_choice_made_after_a_crash_is_what_gets_restored() {
    let mut machine = Machine::new(CTRL_SPACE);
    let plan = machine.claim_read(F19);
    machine.keep_record(plan, Some(CTRL_SPACE));
    machine.crash();
    machine.other_writes(CUSTOM);
    machine.claim(F19);
    assert_eq!(machine.release(), Release::Restore);
    assert_eq!(machine.sys, Some(CUSTOM));
}

// 발견 2. 모델의 길: Start(f19), ..., ClaimWrite, Crash, StartRestore(f18), ReleaseRead. 고치기 전에는
// 60번이 이번 --target 과 달라 "남이 바꿨다"고 보고 원래 값을 버렸다.
#[test]
fn finding_2_a_key_written_by_an_earlier_run_is_still_ours() {
    let mut machine = Machine::new(CTRL_SPACE);
    machine.claim(F19);
    machine.crash();
    // 다른 F-키로 다시 띄우다가, 기록만 고치고 60번을 쓰기 전에 또 죽는다. 60번은 아직 f19 다.
    let plan = machine.claim_read(F18);
    machine.keep_record(plan, Some(ours(F19)));
    machine.crash();
    assert_eq!(machine.release(), Release::Restore);
    assert_eq!(machine.sys, Some(CTRL_SPACE));
}

#[test]
fn switching_to_another_key_after_a_crash_keeps_the_original() {
    let mut machine = Machine::new(CTRL_SPACE);
    machine.claim(F19);
    machine.crash();
    machine.claim(F18);
    assert_eq!(machine.sys, Some(ours(F18)));
    machine.crash();
    assert_eq!(machine.release(), Release::Restore);
    assert_eq!(machine.sys, Some(CTRL_SPACE));
}

// 발견 3. 고치기 전의 길: 표시를 쓰고, 원래 값을 쓰기 전에 Crash. 이제 둘은 한 번의 쓰기라 그 사이가
// 없다(`crates/hanyeong-mac/src/kept.rs`). 기록을 남긴 직후에 죽어도 잃는 것이 없는지 본다.
#[test]
fn finding_3_a_crash_right_after_keeping_the_record_loses_nothing() {
    let mut machine = Machine::new(CTRL_SPACE);
    let plan = machine.claim_read(F19);
    machine.keep_record(plan, Some(CTRL_SPACE));
    machine.crash();
    assert_eq!(machine.original, Some(CTRL_SPACE));
    machine.claim(F19);
    assert_eq!(machine.release(), Release::Restore);
    assert_eq!(machine.sys, Some(CTRL_SPACE));
}

// 옛 형식을 남긴 빌드가 죽은 뒤에 새 빌드가 뜬다. 옛 기록은 어느 F-키를 썼는지 적지 않았다.
#[test]
fn a_record_in_the_old_format_is_still_restored() {
    let mut machine = Machine::left_by_the_old_format(CTRL_SPACE, F19);
    assert_eq!(machine.claim(F19), Claim::Already);
    assert_eq!(machine.release(), Release::Restore);
    assert_eq!(machine.sys, Some(CTRL_SPACE));

    let mut machine = Machine::left_by_the_old_format(CTRL_SPACE, F19);
    machine.other_writes(CUSTOM);
    // 60번이 우리 값이 아니다. 옛 기록을 믿지 않고 지금 값을 새로 남긴다.
    assert_eq!(
        machine.claim(F18),
        Claim::Write {
            written: Written::only(F18),
            fresh: true
        }
    );
    assert_eq!(machine.release(), Release::Restore);
    assert_eq!(machine.sys, Some(CUSTOM));
}

//! `run` 과 `restore`: 키 매핑과 단축키를 걸고, 탭을 붙이고, 끝날 때 되돌린다.

use std::time::Duration;

use hanyeong_core::args::Keys;
use hanyeong_core::lag::{self, Lag};
use hanyeong_core::mapping::{self, Change, Pair};
use hanyeong_core::press::PressGate;
use hanyeong_core::recheck::{IDLE_MS, Recheck};
use hanyeong_core::secure::{Notice, SecureWatch};
use hanyeong_mac::hid::{Keyboard, Keyboards, MAX_KEYBOARDS};
use hanyeong_mac::hotkey::{self, Released};
use hanyeong_mac::secure;
use hanyeong_mac::session::{self, Events};
use hanyeong_mac::tap::Tap;
use hanyeong_mac::tray::Tray;
use hanyeong_mac::trust;

use crate::rivals::{rival, running};

struct App {
    want: Pair,
    recheck: Recheck,
    /// 키보드마다, 우리 매핑이 밀어낸 남의 매핑의 대상.
    displaced: Vec<(u64, u64)>,
    /// 쓰기를 거부한 키보드. 같은 말을 되풀이하지 않으려고 기억한다.
    refused: Vec<u64>,
}

impl App {
    fn note(list: &mut Vec<u64>, id: u64) -> bool {
        if list.contains(&id) || list.len() >= MAX_KEYBOARDS {
            return false;
        }
        list.push(id);
        true
    }

    fn apply_to(&mut self, keyboard: &Keyboard<'_>) {
        let Some(id) = keyboard.id() else {
            return;
        };
        let Some(current) = keyboard.mappings() else {
            if Self::note(&mut self.refused, id) {
                err!("hanyeong: 키보드 {id:#x} 의 키 매핑을 읽을 수 없어 건너뜁니다");
            }
            return;
        };
        match mapping::apply(&current, self.want) {
            Change::Keep => {}
            Change::TooMany => {
                if Self::note(&mut self.refused, id) {
                    err!("hanyeong: 키보드 {id:#x} 의 키 매핑이 너무 많아 건너뜁니다");
                }
            }
            Change::Write { list, displaced } => {
                if !keyboard.set_mappings(&list) {
                    if Self::note(&mut self.refused, id) {
                        err!("hanyeong: 키보드 {id:#x} 가 키 매핑을 받지 않습니다");
                    }
                    return;
                }
                let mut name = String::new();
                keyboard.push_name(&mut name);
                out!("hanyeong: 한영 키를 걸었습니다: {name} ({id:#x})");
                if let Some(dst) = displaced
                    && self.displaced.len() < MAX_KEYBOARDS
                {
                    self.displaced.retain(|(known, _)| *known != id);
                    self.displaced.push((id, dst));
                }
            }
        }
    }

    fn apply_all(&mut self) {
        match Keyboards::open() {
            Some(keyboards) => keyboards.each(|keyboard| self.apply_to(keyboard)),
            None => err!("hanyeong: 키보드 목록을 읽지 못했습니다"),
        }
    }

    /// 우리가 쓴 매핑만 걷어낸다. 걷어낸 키보드 수를 돌려준다.
    fn remove_all(&self) -> usize {
        let mut removed = 0_usize;
        let Some(keyboards) = Keyboards::open() else {
            err!("hanyeong: 키보드 목록을 읽지 못해 키 매핑을 되돌리지 못했습니다");
            return 0;
        };
        keyboards.each(|keyboard| {
            let (Some(id), Some(current)) = (keyboard.id(), keyboard.mappings()) else {
                return;
            };
            let displaced = self
                .displaced
                .iter()
                .find(|(known, _)| *known == id)
                .map(|(_, dst)| *dst);
            if let Change::Write { list, .. } = mapping::remove(&current, self.want, displaced) {
                if keyboard.set_mappings(&list) {
                    removed = removed.saturating_add(1);
                } else {
                    err!("hanyeong: 키보드 {id:#x} 의 키 매핑을 되돌리지 못했습니다");
                }
            }
        });
        removed
    }
}

/// 보안 입력을 켠 프로세스를 `이름(번호)` 로 적는다.
pub fn owner(pid: i64) -> String {
    let mut text = String::new();
    if pid <= 0 {
        text.push_str("알 수 없는 프로세스");
    } else if secure::push_process_name(pid, &mut text) {
        text.push_str(&format!("({pid})"));
    } else {
        text.push_str(&format!("프로세스 {pid}"));
    }
    text
}

/// 탭이 붙어 도는 동안의 앱. 확인할 때마다 탭이 모은 대기 분포를 가져와, 늦은 누름이 있었으면 남긴다.
struct Running<'a> {
    app: &'a mut App,
    tap: &'a Tap,
    lag: Lag,
    secure: SecureWatch,
}

impl Running<'_> {
    /// 보안 입력이 켜지거나 풀렸으면 남긴다. 켜진 동안에는 한영 키가 탭에 오지 않는다.
    fn check_secure(&mut self) {
        match self.secure.observe(secure::state()) {
            Some(Notice::On { pid }) => err!(
                "hanyeong: 보안 입력이 켜져 있습니다: {}. 그동안 한영 키는 hanyeong 에 닿지 않아 뗄 때 \
                 시스템이 전환하고, 기기가 바쁘면 누름을 잃습니다. 그 앱의 암호 입력란을 벗어나거나 앱을 \
                 다시 띄우면 풀립니다",
                owner(pid)
            ),
            Some(Notice::Off) => out!("hanyeong: 보안 입력이 풀렸습니다"),
            None => {}
        }
    }

    fn collect(&mut self) {
        let recent = self.tap.take_lag();
        if recent.slow() > 0 {
            out!(
                "hanyeong: 지난 확인 뒤로 한영 키가 늦게 닿았습니다: {} ms 이상 {}번, 가장 늦은 것 {:.1} ms",
                lag::SLOW_MS,
                recent.slow(),
                ms(recent.max_ns)
            );
        }
        self.lag.add(&recent);
    }
}

impl Events for Running<'_> {
    fn recheck(&mut self) -> u32 {
        self.collect();
        self.check_secure();
        self.app.apply_all();
        self.app.recheck.checked()
    }

    fn device_changed(&mut self) -> u32 {
        self.app.recheck.device_changed()
    }
}

/// 로그에 보이는 ms 값. 2^53 ns(약 104일)까지는 정확하다.
fn ms(ns: u64) -> f64 {
    ns as f64 / 1_000_000.0
}

/// 실행 전체의 대기 분포를 한 줄로 남긴다.
fn report_lag(total: &Lag) {
    let mut spread = String::new();
    let mut lower = 0_u64;
    for (index, count) in total.counts.iter().enumerate() {
        if !spread.is_empty() {
            spread.push_str(" · ");
        }
        match lag::BOUNDS_MS.get(index) {
            Some(upper) => {
                spread.push_str(&format!("{lower}~{upper} ms {count}"));
                lower = *upper;
            }
            None => spread.push_str(&format!("{lower} ms~ {count}")),
        }
    }
    out!(
        "hanyeong: 한영 키가 hanyeong 에 닿기까지: {}번, {} ms 이상 {}번, 가장 늦은 것 {:.1} ms, 잴 수 없음 {}번 ({spread})",
        total.measured(),
        lag::SLOW_MS,
        total.slow(),
        ms(total.max_ns),
        total.unknown
    );
}

fn new_app(keys: Keys) -> App {
    App {
        want: Pair {
            src: keys.source.usage(),
            dst: keys.target.usage(),
        },
        recheck: Recheck::new(),
        displaced: Vec::new(),
        refused: Vec::new(),
    }
}

/// 걸어 둔 것을 되돌린다. 전부 되돌렸으면 참.
fn undo(app: &App) -> bool {
    let removed = app.remove_all();
    out!("hanyeong: 키보드 {removed} 대의 키 매핑을 되돌렸습니다");
    match hotkey::release() {
        Ok(Released::Restored) => out!("hanyeong: 입력 소스 단축키를 원래대로 되돌렸습니다"),
        Ok(Released::Forgotten) => {
            out!("hanyeong: 입력 소스 단축키는 그 뒤에 바뀌어 있어 그대로 둡니다");
        }
        Ok(Released::Untouched) => {}
        Err(error) => {
            err!("hanyeong: {error}");
            return false;
        }
    }
    true
}

/// 권한이 없을 때 끝내기 전에 쉬는 시간. 로그인 항목은 실패로 끝나면 다시 띄워지므로, 이만큼의
/// 간격으로 새 프로세스가 권한을 다시 확인하게 된다.
const TRUST_RETRY: Duration = Duration::from_secs(5);

/// 접근성 권한이 있는지 본다. 없으면 한 번만 설정 화면을 열어 안내하고 거짓을 돌려준다.
///
/// 프로세스 안에서 기다리지 않는다: 2026-11-16 에 설치본이 10분 동안 기다리기만 하고 권한을 받지
/// 못했다(안내 창이 뜨지 않았거나, 켠 것을 돌고 있던 프로세스가 알아채지 못했다). 새로 뜬
/// 프로세스의 판단은 확실하다.
fn check_trust() -> bool {
    if trust::trusted() {
        if trust::prompted() {
            trust::set_prompted(false);
        }
        return true;
    }
    if !trust::prompted() {
        trust::request();
        trust::open_settings();
        trust::set_prompted(true);
        err!(
            "hanyeong: 접근성 권한이 없습니다. 방금 연 손쉬운 사용 화면에서 hanyeong(터미널에서 띄웠다면 \
             그 터미널)을 켜 주세요. 목록에 없으면 + 로 ~/Applications/hanyeong.app 을 더합니다"
        );
    }
    std::thread::sleep(TRUST_RETRY);
    false
}

pub fn run(keys: Keys) -> bool {
    // 권한부터 본다. 로그인 항목이 권한을 기다리는 동안 터미널에서 띄운 것이 돌고 있어도 안내는
    // 나가야 한다.
    if !check_trust() {
        return false;
    }
    if let Some(name) = rival() {
        err!(
            "hanyeong: {name} 이(가) 돌고 있습니다. 함께 돌면 서로의 키를 주고받으므로 먼저 종료하세요"
        );
        return false;
    }
    // 탭을 먼저 붙인다. 매핑부터 걸면 그 사이의 누름이 뗄 때 전환된다.
    let Some(tap) = Tap::install(PressGate::new(keys.target.key_code())) else {
        err!("hanyeong: 이벤트 탭을 만들지 못했습니다");
        return false;
    };
    if !tap.realtime() {
        err!(
            "hanyeong: 탭 스레드를 실시간 우선순위로 올리지 못했습니다. 기기가 바쁘면 전환이 늦을 수 있습니다"
        );
    }
    let mut app = new_app(keys);
    if let Err(error) = hotkey::claim(keys.target.key_code()) {
        err!("hanyeong: {error}");
        undo(&app);
        return false;
    }
    app.apply_all();
    out!(
        "hanyeong: 준비됐습니다 ({} → {})",
        keys.source.name(),
        keys.target.name()
    );
    let tray = Tray::install(&tap);
    if tray.is_none() {
        err!("hanyeong: 메뉴바 아이콘을 올리지 못했습니다. 아이콘 없이 계속합니다");
    }
    let mut running = Running {
        app: &mut app,
        tap: &tap,
        lag: Lag::default(),
        secure: SecureWatch::new(),
    };
    running.check_secure();
    let ran = session::run(&mut running, IDLE_MS, tray.as_ref());
    let mut total = running.lag;
    drop(tray);
    let stats = tap.stop();
    total.add(&stats.lag);
    out!(
        "hanyeong: 전환 {}번, 폭주 차단 {}번, 탭 꺼짐 {}번",
        stats.switches,
        stats.storms,
        stats.disabled
    );
    report_lag(&total);
    let undone = undo(&app);
    if let Err(error) = &ran {
        err!("hanyeong: {error}");
    }
    ran.is_ok() && undone
}

pub fn restore(keys: Keys) -> bool {
    if running("hanyeong") > 0 {
        err!("hanyeong: 실행 중인 hanyeong 이 있습니다. 그것을 종료하면 스스로 되돌립니다");
        return false;
    }
    undo(&new_app(keys))
}

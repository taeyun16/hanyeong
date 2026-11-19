//! `run` 과 `restore`: 키 매핑과 단축키를 걸고, 탭을 붙이고, 끝날 때 되돌린다.

use std::time::Duration;

use hanyeong_core::args::Keys;
use hanyeong_core::mapping::{self, Change, Pair};
use hanyeong_core::press::PressGate;
use hanyeong_core::recheck::{IDLE_MS, Recheck};
use hanyeong_mac::hid::{Keyboard, Keyboards, MAX_KEYBOARDS};
use hanyeong_mac::hotkey::{self, Released};
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
                eprintln!("hanyeong: 키보드 {id:#x} 의 키 매핑을 읽을 수 없어 건너뜁니다");
            }
            return;
        };
        match mapping::apply(&current, self.want) {
            Change::Keep => {}
            Change::TooMany => {
                if Self::note(&mut self.refused, id) {
                    eprintln!("hanyeong: 키보드 {id:#x} 의 키 매핑이 너무 많아 건너뜁니다");
                }
            }
            Change::Write { list, displaced } => {
                if !keyboard.set_mappings(&list) {
                    if Self::note(&mut self.refused, id) {
                        eprintln!("hanyeong: 키보드 {id:#x} 가 키 매핑을 받지 않습니다");
                    }
                    return;
                }
                let mut name = String::new();
                keyboard.push_name(&mut name);
                println!("hanyeong: 한영 키를 걸었습니다: {name} ({id:#x})");
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
            None => eprintln!("hanyeong: 키보드 목록을 읽지 못했습니다"),
        }
    }

    /// 우리가 쓴 매핑만 걷어낸다. 걷어낸 키보드 수를 돌려준다.
    fn remove_all(&self) -> usize {
        let mut removed = 0_usize;
        let Some(keyboards) = Keyboards::open() else {
            eprintln!("hanyeong: 키보드 목록을 읽지 못해 키 매핑을 되돌리지 못했습니다");
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
                    eprintln!("hanyeong: 키보드 {id:#x} 의 키 매핑을 되돌리지 못했습니다");
                }
            }
        });
        removed
    }
}

impl Events for App {
    fn recheck(&mut self) -> u32 {
        self.apply_all();
        self.recheck.checked()
    }

    fn device_changed(&mut self) -> u32 {
        self.recheck.device_changed()
    }
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
    println!("hanyeong: 키보드 {removed} 대의 키 매핑을 되돌렸습니다");
    match hotkey::release() {
        Ok(Released::Restored) => println!("hanyeong: 입력 소스 단축키를 원래대로 되돌렸습니다"),
        Ok(Released::Forgotten) => {
            println!("hanyeong: 입력 소스 단축키는 그 뒤에 바뀌어 있어 그대로 둡니다");
        }
        Ok(Released::Untouched) => {}
        Err(error) => {
            eprintln!("hanyeong: {error}");
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
        eprintln!(
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
        eprintln!(
            "hanyeong: {name} 이(가) 돌고 있습니다. 함께 돌면 서로의 키를 주고받으므로 먼저 종료하세요"
        );
        return false;
    }
    // 탭을 먼저 붙인다. 매핑부터 걸면 그 사이의 누름이 뗄 때 전환된다.
    let Some(tap) = Tap::install(PressGate::new(keys.target.key_code())) else {
        eprintln!("hanyeong: 이벤트 탭을 만들지 못했습니다");
        return false;
    };
    let mut app = new_app(keys);
    if let Err(error) = hotkey::claim(keys.target.key_code()) {
        eprintln!("hanyeong: {error}");
        undo(&app);
        return false;
    }
    app.apply_all();
    println!(
        "hanyeong: 준비됐습니다 ({} → {})",
        keys.source.name(),
        keys.target.name()
    );
    let tray = Tray::install(&tap);
    if tray.is_none() {
        eprintln!("hanyeong: 메뉴바 아이콘을 올리지 못했습니다. 아이콘 없이 계속합니다");
    }
    let ran = session::run(&mut app, IDLE_MS, tray.as_ref());
    drop(tray);
    let stats = tap.stats();
    drop(tap);
    println!(
        "hanyeong: 전환 {}번, 폭주 차단 {}번, 탭 꺼짐 {}번",
        stats.switches, stats.storms, stats.disabled
    );
    let undone = undo(&app);
    if let Err(error) = &ran {
        eprintln!("hanyeong: {error}");
    }
    ran.is_ok() && undone
}

pub fn restore(keys: Keys) -> bool {
    if running("hanyeong") > 0 {
        eprintln!("hanyeong: 실행 중인 hanyeong 이 있습니다. 그것을 종료하면 스스로 되돌립니다");
        return false;
    }
    undo(&new_app(keys))
}

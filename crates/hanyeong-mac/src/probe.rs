//! 실기기 측정. 지금 돌고 있는 도구(hanyeong 이든 gksdud 든, 아무것도 없든)를 밖에서 잰다.
//!
//! 여기서 보내는 키 이벤트는 사용자의 입력 소스를 실제로 바꾼다. 짝수 번 누르면 원래대로 돌아온다.

use core::cell::Cell;
use core::ffi::c_void;
use core::ptr;

use crate::cf::{self, Owned};
use crate::ffi::{self, Ref};
use crate::tap::{marker, now_ns};

/// 입력 소스가 바뀌면 시스템이 돌리는 알림의 이름(`kTISNotifySelectedKeyboardInputSourceChanged`).
const SOURCE_CHANGED: &str = "com.apple.Carbon.TISNotifySelectedKeyboardInputSourceChanged";
/// 이 시간 안에 결과가 없으면 그 누름은 놓친 것으로 적는다.
const TIMEOUT_NS: u64 = 1_000_000_000;
/// 통과 지연을 잴 때 보내는, 어느 앱도 뜻을 두지 않는 키 코드. 세션 끝의 우리 탭이 없앤다.
const INERT_KEY: u16 = 0xFF;
const MS: u64 = 1_000_000;

/// 런 루프를 `until_ns` 까지, 또는 `done` 이 참이 될 때까지 돌린다.
fn spin(until_ns: u64, done: impl Fn() -> bool) {
    loop {
        let now = now_ns();
        if done() || now >= until_ns {
            return;
        }
        let seconds = (until_ns.saturating_sub(now) as f64 / 1.0e9).min(0.05);
        // SAFETY: 기본 모드에서 지금 스레드의 런 루프를 잠깐 돌린다.
        unsafe { ffi::CFRunLoopRunInMode(ffi::DEFAULT_MODE, seconds, 1) };
    }
}

fn post(key: u16, down: bool, flags: u64, user_data: i64) -> bool {
    // SAFETY: 이벤트 소스 없이 키 이벤트를 만들고, 반환값은 Create 규칙으로 우리가 소유한다.
    let Some(event) = (unsafe {
        Owned::adopt(ffi::CGEventCreateKeyboardEvent(ptr::null(), key, down).cast_const())
    }) else {
        return false;
    };
    let event = event.get().cast_mut();
    // SAFETY: `event` 는 방금 만든 우리 소유의 이벤트다.
    unsafe {
        ffi::CGEventSetFlags(event, flags);
        ffi::CGEventSetIntegerValueField(event, ffi::FIELD_USER_DATA, user_data);
        ffi::CGEventPost(ffi::HID_TAP, event);
    }
    true
}

extern "C" fn source_changed(
    _center: Ref,
    observer: *mut c_void,
    _name: Ref,
    _object: *const c_void,
    _user_info: Ref,
) {
    // SAFETY: `observer` 는 `switch_latency` 의 스택에 있는 `Cell` 이고, 그 함수가 끝나기 전에
    // 관찰자를 지운다. 알림은 그 함수가 돌리는 런 루프에서만 온다.
    if let Some(arrived) = unsafe { observer.cast::<Cell<u64>>().as_ref() }
        && arrived.get() == 0
    {
        arrived.set(now_ns());
    }
}

/// 대상 F-키를 `count` 번 눌러, 누른 순간부터 입력 소스가 바뀌었다는 알림까지의 시간(ns)을 잰다.
///
/// 누른 뒤 `hold_ms` 가 지나면 뗀다. 한영 도구가 없으면 시스템은 뗄 때 전환하므로 그만큼 늦게 온다.
/// 놓친 누름은 `None` 이다. 키를 보내지 못하면(접근성 권한) 거짓을 돌려준다.
#[must_use]
pub fn switch_latency(
    key_code: i64,
    count: u32,
    hold_ms: u32,
    gap_ms: u32,
    out: &mut Vec<Option<u64>>,
) -> bool {
    let (Ok(key), Some(name)) = (u16::try_from(key_code), cf::string(SOURCE_CHANGED)) else {
        return false;
    };
    let arrived = Cell::new(0_u64);
    let observer = (&raw const arrived).cast::<c_void>();
    // SAFETY: 분산 알림 센터는 프로세스가 사는 동안 살아 있고, `observer` 는 아래에서 지울 때까지 산다.
    let center = unsafe { ffi::CFNotificationCenterGetDistributedCenter() };
    // SAFETY: 콜백의 시그니처가 맞고 이름은 살아 있는 CFString 이다.
    unsafe {
        ffi::CFNotificationCenterAddObserver(
            center,
            observer,
            source_changed,
            name.get(),
            ptr::null(),
            ffi::DELIVER_IMMEDIATELY,
        );
    }
    let mut sent = true;
    for _ in 0..count {
        // 앞선 전환의 늦은 알림을 흘려보낸다.
        spin(now_ns().saturating_add(20 * MS), || false);
        arrived.set(0);
        let start = now_ns();
        sent = post(key, true, ffi::FLAG_SECONDARY_FN, 0);
        if !sent {
            break;
        }
        let seen = || arrived.get() != 0;
        spin(
            start.saturating_add(u64::from(hold_ms).saturating_mul(MS)),
            seen,
        );
        sent = post(key, false, ffi::FLAG_SECONDARY_FN, 0);
        spin(start.saturating_add(TIMEOUT_NS), seen);
        out.push(seen().then(|| arrived.get().saturating_sub(start)));
        spin(
            start.saturating_add(u64::from(gap_ms).saturating_mul(MS)),
            || false,
        );
    }
    // SAFETY: 위에서 등록한 관찰자를 지운다. 이 뒤로는 `arrived` 를 가리키는 콜백이 없다.
    unsafe { ffi::CFNotificationCenterRemoveEveryObserver(center, observer) };
    sent
}

/// 세션에서 기다리는 키.
#[derive(Clone, Copy)]
enum Watch {
    /// 우리가 표시를 단 뜻 없는 키. 모든 탭을 지난 끝에서 보고, 앱에 닿지 않게 없앤다.
    Marked(i64),
    /// 이 키 코드의 누름 가운데 다른 프로세스가 보낸 것(한영 도구가 다시 보낸 것). 시스템 단축키는
    /// 끝에 닿기 전에 시스템이 가져가므로 머리에서 본다(F18 은 끝에 닿고 F19 는 닿지 않았다).
    /// 시스템이 전환하도록 그대로 넘긴다.
    Pulse(i64),
}

struct Tail {
    watch: Watch,
    pid: i64,
    arrived: Cell<u64>,
}

extern "C" fn tail_callback(
    _proxy: *mut c_void,
    kind: u32,
    event: *mut c_void,
    info: *mut c_void,
) -> *mut c_void {
    // SAFETY: `info` 는 `tail_latency` 의 스택에 있는 `Tail` 이고, 그 함수가 끝나기 전에
    // 탭을 무효로 만든다.
    let Some(tail) = (unsafe { info.cast::<Tail>().as_ref() }) else {
        return event;
    };
    if kind != ffi::KEY_DOWN && kind != ffi::KEY_UP {
        return event;
    }
    // SAFETY: `event` 는 콜백 동안 살아 있는 키 이벤트다.
    let (code, user_data, pid) = unsafe {
        (
            ffi::CGEventGetIntegerValueField(event, ffi::FIELD_KEYCODE),
            ffi::CGEventGetIntegerValueField(event, ffi::FIELD_USER_DATA),
            ffi::CGEventGetIntegerValueField(event, ffi::FIELD_SOURCE_PID),
        )
    };
    let (matched, swallow) = match tail.watch {
        Watch::Marked(marker) => (user_data == marker, true),
        Watch::Pulse(wanted) => (code == wanted && pid != 0 && pid != tail.pid, false),
    };
    if !matched {
        return event;
    }
    if kind == ffi::KEY_DOWN && tail.arrived.get() == 0 {
        tail.arrived.set(now_ns());
    }
    if swallow { ptr::null_mut() } else { event }
}

/// 세션에 탭을 붙인 채로 `body` 를 돌린다. 탭을 만들지 못하면(접근성 권한) 거짓.
fn with_tail(tail: &Tail, body: impl FnOnce() -> bool) -> bool {
    let place = match tail.watch {
        Watch::Marked(_) => ffi::TAIL_APPEND,
        // 나중에 붙인 머리 탭이 먼저 불리므로 한영 도구보다 앞이다.
        Watch::Pulse(_) => ffi::HEAD_INSERT,
    };
    // SAFETY: 콜백의 시그니처가 맞고, `tail` 은 아래에서 탭을 무효로 만들 때까지 산다.
    let port = unsafe {
        Owned::adopt(ffi::CGEventTapCreate(
            ffi::SESSION_TAP,
            place,
            ffi::TAP_DEFAULT,
            ffi::KEY_MASK,
            tail_callback,
            ptr::from_ref(tail).cast_mut().cast(),
        ))
    };
    // SAFETY: `port` 는 살아 있는 mach 포트다.
    let source = port.as_ref().and_then(|port| unsafe {
        Owned::adopt(ffi::CFMachPortCreateRunLoopSource(
            ptr::null(),
            port.get(),
            0,
        ))
    });
    let (Some(port), Some(source)) = (port, source) else {
        return false;
    };
    // SAFETY: 소스와 포트는 살아 있고, 지금 스레드의 런 루프에 붙인다.
    unsafe {
        ffi::CFRunLoopAddSource(ffi::CFRunLoopGetCurrent(), source.get(), ffi::COMMON_MODES);
        ffi::CGEventTapEnable(port.get(), true);
    }
    let sent = body();
    // 마지막 뗌이 탭을 지나갈 시간을 준 뒤에 걷는다.
    spin(now_ns().saturating_add(50 * MS), || false);
    // SAFETY: 포트를 무효로 만들면 콜백이 더 불리지 않는다. `tail` 은 그 뒤에 버려진다.
    unsafe {
        ffi::CGEventTapEnable(port.get(), false);
        ffi::CFMachPortInvalidate(port.get());
    }
    sent
}

/// 키를 넣고 탭에 닿기까지를 `count` 번 잰다. `timing` 은 (횟수, 누름과 뗌 사이 ms, 누름 사이 ms).
fn tail_latency(
    key: u16,
    flags: u64,
    watch: Watch,
    timing: (u32, u32, u32),
    out: &mut Vec<Option<u64>>,
) -> bool {
    let (count, hold_ms, gap_ms) = timing;
    let user_data = match watch {
        Watch::Marked(marker) => marker,
        Watch::Pulse(_) => 0,
    };
    let tail = Tail {
        watch,
        pid: i64::from(std::process::id()),
        arrived: Cell::new(0),
    };
    with_tail(&tail, || {
        for _ in 0..count {
            tail.arrived.set(0);
            let start = now_ns();
            if !post(key, true, flags, user_data) {
                return false;
            }
            let seen = || tail.arrived.get() != 0;
            spin(
                start.saturating_add(u64::from(hold_ms).saturating_mul(MS)),
                seen,
            );
            if !post(key, false, flags, user_data) {
                return false;
            }
            spin(start.saturating_add(TIMEOUT_NS), seen);
            out.push(seen().then(|| tail.arrived.get().saturating_sub(start)));
            spin(
                start.saturating_add(u64::from(gap_ms).saturating_mul(MS)),
                || false,
            );
        }
        true
    })
}

/// 뜻 없는 키 하나를 HID 단계에 넣어, 세션의 이벤트 탭들을 다 지나 끝에 닿기까지의 시간(ns)을 잰다.
///
/// 한영 도구의 탭은 세션 머리에 있으므로, 이 시간에는 그 도구의 콜백을 다녀오는 시간이 들어 있다.
/// 탭을 만들지 못하면(접근성 권한) 거짓을 돌려준다.
#[must_use]
pub fn passthrough_latency(count: u32, gap_ms: u32, out: &mut Vec<Option<u64>>) -> bool {
    tail_latency(
        INERT_KEY,
        0,
        Watch::Marked(marker()),
        (count, 0, gap_ms),
        out,
    )
}

/// 대상 F-키를 눌러, 한영 도구가 다시 보낸 누름이 세션 머리를 지나기까지의 시간(ns)을 잰다.
///
/// 도구가 다시 보낸 누름은 보낸 프로세스 번호로 가린다. 그래서 받은 이벤트의 복사본을 다시 보내는
/// 도구(gksdud 2.1.1, 2026-11-16 이전의 hanyeong)는 잴 수 없다: 그 복사본에는 이 측정의 번호가
/// 남고, 그 도구는 자기 누름을 알아보지 못해 스스로 주고받는다. 도구가 없으면 모두 `None` 이다.
/// 입력 소스가 실제로 바뀐다(누를 때마다 한 번).
#[must_use]
pub fn react_latency(
    key_code: i64,
    count: u32,
    hold_ms: u32,
    gap_ms: u32,
    out: &mut Vec<Option<u64>>,
) -> bool {
    let Ok(key) = u16::try_from(key_code) else {
        return false;
    };
    tail_latency(
        key,
        ffi::FLAG_SECONDARY_FN,
        Watch::Pulse(key_code),
        (count, hold_ms, gap_ms),
        out,
    )
}

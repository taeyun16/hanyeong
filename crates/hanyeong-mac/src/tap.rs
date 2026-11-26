//! 세션 이벤트 탭. 대상 F-키의 누름을 잡아 그 자리에서 누름·뗌 한 쌍으로 바꿔 보낸다.
//!
//! 콜백은 사용자가 치는 모든 키가 지나간다. 대상 키가 아니면 키 코드 하나만 읽고 돌려준다.
//!
//! 탭은 전용 스레드에서 돈다. 메인 스레드는 30초마다 키 매핑을 다시 확인하고 메뉴바를 그리는데,
//! 기기가 바쁘고 메모리가 스왑에 나가 있으면 그 일이 길어진다. 같은 스레드였다면 그동안 누른 한영
//! 키가 기다렸다. 이 스레드는 실시간 우선순위로 돌고(안 되면 사용자 입력용 QoS), 탭이 사는 동안
//! App Nap 을 끈다.
//!
//! 실시간인 까닭(2026-11-26 실측, 부하 평균 109~145): 실시간이 아닌 스레드는 깨어남의 약 1% 가
//! 50 ms 이상, 최대 192 ms 늦었고, 실시간 스레드는 최대 0.6 ms 였다. 사용자 입력용 QoS 는 메뉴바 앱에서
//! 37 까지만 올라, CPU 를 쓰는 46~47 의 스레드들 아래에 남는다.

use core::ffi::c_void;
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};

use hanyeong_core::lag::{self, Lag, Timebase};
use hanyeong_core::press::{self, Decision, Key, Kind, PressGate};

use crate::cf::{self, Owned};
use crate::ffi;
use crate::objc;

/// 다시 보내는 한 쌍에 남기는 플래그: F-키임을 나타내는 둘과 Caps Lock 상태.
/// 눌려 있던 수정키는 지운다. 남기면 단축키와 일치하지 않는다.
const KEPT_FLAGS: u64 = ffi::FLAG_SECONDARY_FN | ffi::FLAG_NUMERIC_PAD | ffi::FLAG_ALPHA_SHIFT;

/// 실시간 스레드가 한 번 깨어나 쓸 것으로 알리는 CPU 시간. 콜백은 이보다 훨씬 짧다. 이것을 자주 넘기면
/// 커널이 실시간을 거둔다.
const RT_COMPUTATION_NS: u64 = 500_000;
/// 깨어난 뒤 일을 마쳐야 하는 시간.
const RT_CONSTRAINT_NS: u64 = 1_000_000;

/// App Nap 을 끄는 까닭으로 시스템에 남기는 말.
const ACTIVITY_REASON: &str = "한영 키 입력 처리";

/// 메인 스레드(메뉴)와 탭 스레드가 함께 보는 것.
struct Shared {
    /// 사용자가 메뉴에서 멈췄다. 탭을 꺼 두고, 시스템이 껐을 때처럼 다시 켜지 않는다.
    paused: AtomicBool,
    /// 멈추거나 다시 켤 때마다 하나 는다. 콜백이 달라진 것을 보면 누름 상태를 잊는다.
    epoch: AtomicU64,
    /// 메인 스레드가 마지막으로 가져간 뒤의 대기 분포. 콜백은 기다리지 않고 넣을 수 있을 때만 넣는다.
    lag: Mutex<Lag>,
}

/// 탭 스레드에서만 쓰는 것.
struct State {
    gate: PressGate,
    /// 우리가 보낸 이벤트에 다는 값. 실행마다 다르다.
    marker: i64,
    /// 다시 보내는 누름을 만드는 이벤트 소스. 사용자 데이터가 `marker` 로 정해져 있다.
    source: ffi::Ref,
    /// 이 프로세스의 번호. 우리가 보낸 이벤트에는 이 번호가 붙는다.
    pid: i64,
    /// 탭 자신. 시스템이 탭을 껐을 때 다시 켜는 데 쓴다.
    port: ffi::Ref,
    disabled: u64,
    shared: Arc<Shared>,
    /// 콜백이 마지막으로 본 `Shared::epoch`.
    epoch: u64,
    /// 키의 만든 시각을 읽는 데 쓰는 비율.
    timebase: Timebase,
    /// 아직 `Shared::lag` 에 넣지 못한 대기.
    pending: Lag,
}

impl State {
    /// 전환한 누름이 기다린 시간을 남긴다. 메인 스레드가 분포를 가져가는 중이면 다음 누름 때 넣는다.
    fn note_lag(&mut self, now_ns: u64, created: u64) {
        self.pending
            .record(lag::waited_ns(now_ns, created, self.timebase));
        if let Ok(mut shared) = self.shared.lag.try_lock() {
            shared.add(&self.pending);
            self.pending = Lag::default();
        }
    }
}

/// 탭 스레드가 메인 스레드에 넘기는 두 객체. 둘 다 우리가 retain 한 참조다.
struct Remote {
    run_loop: ffi::Ref,
    port: ffi::Ref,
    realtime: bool,
}

// SAFETY: 런 루프와 mach 포트는 CoreFoundation 객체라 참조를 어느 스레드에서 놓아도 되고, 메인
// 스레드는 이것으로 스레드 안전한 `CFRunLoopStop`·`CGEventTapEnable` 만 부른다.
unsafe impl Send for Remote {}

/// 설치된 탭. 버려지면 탭 스레드를 멈추고 기다린다. 탭은 그 스레드가 걷어낸다.
pub struct Tap {
    shared: Arc<Shared>,
    run_loop: Owned,
    port: Owned,
    thread: Option<JoinHandle<TapStats>>,
    realtime: bool,
}

/// 탭이 살아 있는 동안 한 일.
#[derive(Clone, Copy, Debug, Default)]
pub struct TapStats {
    pub switches: u64,
    pub storms: u64,
    /// 시스템이 탭을 껐던 횟수(콜백이 느렸다는 뜻이다).
    pub disabled: u64,
    /// 마지막으로 `Tap::take_lag` 한 뒤의 대기 분포.
    pub lag: Lag,
}

pub fn now_ns() -> u64 {
    // SAFETY: 인자가 상수인 시계 조회다.
    unsafe { ffi::clock_gettime_nsec_np(ffi::CLOCK_UPTIME_RAW) }
}

/// 0 이 아닌 무작위 표시 값.
pub fn marker() -> i64 {
    // SAFETY: 인자 없는 난수 조회다.
    let (high, low) = unsafe { (ffi::arc4random(), ffi::arc4random()) };
    ((i64::from(high) << 31) ^ i64::from(low)) | 1
}

extern "C" fn callback(
    _proxy: *mut c_void,
    kind: u32,
    event: *mut c_void,
    info: *mut c_void,
) -> *mut c_void {
    // SAFETY: `info` 는 `serve` 가 넘긴 `State` 이고, 탭이 걷힐 때까지 살아 있다.
    // 콜백은 탭을 붙인 탭 스레드에서만 불리므로 겹치는 참조가 없다.
    let Some(state) = (unsafe { info.cast::<State>().as_mut() }) else {
        return event;
    };
    let kind = match kind {
        ffi::KEY_DOWN => Kind::Down,
        ffi::KEY_UP => Kind::Up,
        ffi::TAP_DISABLED_BY_TIMEOUT | ffi::TAP_DISABLED_BY_USER_INPUT => {
            state.gate.reset();
            if !state.shared.paused.load(Ordering::Relaxed) {
                state.disabled = state.disabled.saturating_add(1);
                // SAFETY: `port` 는 이 탭의 살아 있는 mach 포트다.
                unsafe { ffi::CGEventTapEnable(state.port, true) };
            }
            return event;
        }
        _ => return event,
    };
    // SAFETY: `event` 는 콜백 동안 살아 있는 키 이벤트다.
    let code = unsafe { ffi::CGEventGetIntegerValueField(event, ffi::FIELD_KEYCODE) };
    if code != state.gate.target() {
        return event;
    }
    // 메뉴에서 멈춘 직후, 탭이 꺼지기 전에 들어온 이벤트일 수 있다.
    if state.shared.paused.load(Ordering::Relaxed) {
        return event;
    }
    let epoch = state.shared.epoch.load(Ordering::Acquire);
    if epoch != state.epoch {
        // 멈춘 동안의 뗌을 보지 못했다. 다음 누름은 새 누름이다.
        state.epoch = epoch;
        state.gate.reset();
    }
    // SAFETY: 위와 같은 이벤트다.
    let (repeat, user_data, source_pid, created_ns) = unsafe {
        (
            ffi::CGEventGetIntegerValueField(event, ffi::FIELD_AUTOREPEAT) != 0,
            ffi::CGEventGetIntegerValueField(event, ffi::FIELD_USER_DATA),
            ffi::CGEventGetIntegerValueField(event, ffi::FIELD_SOURCE_PID),
            ffi::CGEventGetTimestamp(event),
        )
    };
    let now_ns = now_ns();
    let decision = state.gate.decide(Key {
        kind,
        code,
        repeat,
        ours: press::is_ours(user_data, state.marker, source_pid, state.pid),
        now_ns,
        created_ns,
    });
    match decision {
        Decision::Pass => event,
        Decision::Swallow => ptr::null_mut(),
        Decision::Switch => {
            pulse(event, state.source, code);
            state.note_lag(now_ns, created_ns);
            ptr::null_mut()
        }
    }
}

/// 우리 소스로 대상 F-키의 누름·뗌 한 쌍을 새로 만들어 HID 단계에 보낸다. 받은 누름에서는 F-키
/// 플래그와 Caps Lock 상태만 가져온다.
///
/// 받은 이벤트를 복사해 보내면 안 된다: 복사본에는 그것을 처음 만든 프로그램의 사용자 데이터와 번호가
/// 남아 우리 것으로 알아볼 수 없다(`press::is_ours`).
fn pulse(event: *mut c_void, source: ffi::Ref, code: i64) {
    let Ok(key) = u16::try_from(code) else {
        return;
    };
    // SAFETY: `event` 는 콜백 동안 살아 있는 키 이벤트다.
    let flags = unsafe { ffi::CGEventGetFlags(event) } & KEPT_FLAGS;
    for down in [true, false] {
        // SAFETY: `source` 는 탭이 살아 있는 동안 유효하고, 반환값은 Create 규칙으로 우리가 소유한다.
        let Some(made) = (unsafe {
            Owned::adopt(ffi::CGEventCreateKeyboardEvent(source, key, down).cast_const())
        }) else {
            return;
        };
        let made = made.get().cast_mut();
        // SAFETY: `made` 는 방금 만든 우리 소유의 이벤트다.
        unsafe {
            ffi::CGEventSetFlags(made, flags);
            ffi::CGEventPost(ffi::HID_TAP, made);
        }
    }
}

/// 탭 스레드의 몸통. 탭을 만들어 `ready` 로 알리고, 멈추라고 할 때까지 런 루프를 돌린 뒤 탭을 걷는다.
fn serve(
    gate: PressGate,
    shared: Arc<Shared>,
    ready: &mpsc::SyncSender<Option<Remote>>,
) -> TapStats {
    // SAFETY: 지금 스레드의 QoS 를 바꾸는 호출이다. 실패해도 기본 QoS 로 계속 돈다.
    unsafe { ffi::pthread_set_qos_class_self_np(ffi::QOS_USER_INTERACTIVE, 0) };
    let timebase = timebase();
    let realtime = make_realtime(timebase);
    let reason = cf::string(ACTIVITY_REASON);
    // SAFETY: `reason` 은 살아 있는 CFString(= NSString)이다. 토큰은 아래에서 끝낸다.
    let activity = reason.as_ref().map_or(ptr::null_mut(), |reason| unsafe {
        objc::begin_activity(reason.get())
    });
    let stats = run_tap(gate, shared, (timebase, realtime), ready);
    // SAFETY: 위에서 받은 토큰이고 한 번만 끝낸다.
    unsafe { objc::end_activity(activity) };
    stats
}

fn timebase() -> Timebase {
    let mut timebase = ffi::MachTimebase::default();
    // SAFETY: `timebase` 는 살아 있는 구조체이고 함수가 채운다. 실패하면 0 이 남아 틱으로 읽지 않는다.
    unsafe { ffi::mach_timebase_info(&raw mut timebase) };
    Timebase {
        numer: timebase.numer,
        denom: timebase.denom,
    }
}

/// 지금 스레드를 실시간 스케줄링으로 바꾼다. 바꿨으면 참.
fn make_realtime(timebase: Timebase) -> bool {
    let (Some(computation), Some(constraint)) = (
        timebase.ticks(RT_COMPUTATION_NS),
        timebase.ticks(RT_CONSTRAINT_NS),
    ) else {
        return false;
    };
    let policy = ffi::TimeConstraint {
        // 주기가 없는 일(키가 올 때만 깨어난다)이다.
        period: 0,
        computation,
        constraint,
        preemptible: 1,
    };
    // SAFETY: `mach_thread_self` 가 준 포트 이름으로 지금 스레드의 정책을 바꾸고, 그 이름을 놓는다.
    // `policy` 는 호출 동안 살아 있고 개수는 그 구조체의 u32 칸 수다.
    unsafe {
        let thread = ffi::mach_thread_self();
        let result = ffi::thread_policy_set(
            thread,
            ffi::THREAD_TIME_CONSTRAINT_POLICY,
            &raw const policy,
            ffi::THREAD_TIME_CONSTRAINT_POLICY_COUNT,
        );
        ffi::mach_port_deallocate(ffi::MACH_TASK_SELF, thread);
        result == 0
    }
}

fn run_tap(
    gate: PressGate,
    shared: Arc<Shared>,
    (timebase, realtime): (Timebase, bool),
    ready: &mpsc::SyncSender<Option<Remote>>,
) -> TapStats {
    let marker = marker();
    // SAFETY: 상수 인자로 만들고, 반환값은 Create 규칙으로 우리가 소유한다.
    let Some(pulse_source) =
        (unsafe { Owned::adopt(ffi::CGEventSourceCreate(ffi::SOURCE_STATE_HID)) })
    else {
        let _ = ready.send(None);
        return TapStats::default();
    };
    // SAFETY: `pulse_source` 는 방금 만든 살아 있는 이벤트 소스다.
    unsafe { ffi::CGEventSourceSetUserData(pulse_source.get(), marker) };
    let epoch = shared.epoch.load(Ordering::Acquire);
    let state = Box::into_raw(Box::new(State {
        gate,
        marker,
        source: pulse_source.get(),
        pid: i64::from(std::process::id()),
        port: ptr::null(),
        disabled: 0,
        shared,
        epoch,
        timebase,
        pending: Lag::default(),
    }));
    // SAFETY: `callback` 의 시그니처는 CGEventTapCallBack 과 같고, `state` 는 탭보다 오래 산다.
    let port = unsafe {
        Owned::adopt(ffi::CGEventTapCreate(
            ffi::SESSION_TAP,
            ffi::HEAD_INSERT,
            ffi::TAP_DEFAULT,
            ffi::KEY_MASK,
            callback,
            state.cast(),
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
        // SAFETY: `state` 는 위에서 만든 Box 이고 아무도 쓰지 않는다.
        drop(unsafe { Box::from_raw(state) });
        let _ = ready.send(None);
        return TapStats::default();
    };
    // SAFETY: `state` 는 살아 있고 아직 콜백이 불리기 전이다. 소스와 모드는 살아 있는 객체다.
    // 메인 스레드에 넘기는 두 참조는 retain 해서 넘긴다.
    let remote = unsafe {
        (*state).port = port.get();
        let run_loop = ffi::CFRunLoopGetCurrent();
        ffi::CFRunLoopAddSource(run_loop, source.get(), ffi::COMMON_MODES);
        ffi::CGEventTapEnable(port.get(), true);
        Remote {
            run_loop: ffi::CFRetain(run_loop),
            port: ffi::CFRetain(port.get()),
            realtime,
        }
    };
    if ready.send(Some(remote)).is_ok() {
        // SAFETY: 메인 스레드가 `CFRunLoopStop` 을 부를 때까지 이 스레드의 런 루프를 돌린다.
        // 그 전에 멈추라고 했어도 런 루프는 곧바로 돌아온다.
        unsafe { ffi::CFRunLoopRun() };
    }
    // SAFETY: 포트를 무효로 만들면 콜백이 더 불리지 않는다. 그 뒤에 `state` 를 놓는다.
    // 이벤트 소스(`pulse_source`)는 이 함수가 끝날 때 탭보다 나중에 버려진다.
    unsafe {
        ffi::CGEventTapEnable(port.get(), false);
        ffi::CFMachPortInvalidate(port.get());
        let state = Box::from_raw(state);
        let mut lag = state
            .shared
            .lag
            .lock()
            .map(|mut lag| core::mem::take(&mut *lag))
            .unwrap_or_default();
        lag.add(&state.pending);
        TapStats {
            switches: state.gate.switches(),
            storms: state.gate.storms(),
            disabled: state.disabled,
            lag,
        }
    }
}

impl Tap {
    /// 탭 스레드를 띄워 탭을 붙인다. 접근성 권한이 없으면 `None`.
    #[must_use]
    pub fn install(gate: PressGate) -> Option<Self> {
        let shared = Arc::new(Shared {
            paused: AtomicBool::new(false),
            epoch: AtomicU64::new(0),
            lag: Mutex::new(Lag::default()),
        });
        let (ready, answer) = mpsc::sync_channel(1);
        let theirs = Arc::clone(&shared);
        let thread = thread::Builder::new()
            .name("hanyeong-tap".into())
            .spawn(move || serve(gate, theirs, &ready))
            .ok()?;
        let Ok(Some(remote)) = answer.recv() else {
            let _ = thread.join();
            return None;
        };
        // SAFETY: 탭 스레드가 retain 해서 넘긴 참조라 우리가 하나씩 소유한다.
        let (run_loop, port) =
            unsafe { (Owned::adopt(remote.run_loop), Owned::adopt(remote.port)) };
        let (Some(run_loop), Some(port)) = (run_loop, port) else {
            return None;
        };
        Some(Self {
            shared,
            run_loop,
            port,
            thread: Some(thread),
            realtime: remote.realtime,
        })
    }

    /// 탭을 멈추거나 다시 켠다. 멈춘 동안 대상 F-키는 시스템이 직접 받아, 뗄 때 전환된다.
    pub fn set_paused(&self, paused: bool) {
        self.shared.paused.store(paused, Ordering::Relaxed);
        self.shared.epoch.fetch_add(1, Ordering::Release);
        // SAFETY: `port` 는 우리가 retain 한 탭의 mach 포트다. 무효가 됐으면 아무 일도 없다.
        unsafe { ffi::CGEventTapEnable(self.port.get(), !paused) };
    }

    /// 탭 스레드가 실시간 우선순위로 도는지.
    #[must_use]
    pub const fn realtime(&self) -> bool {
        self.realtime
    }

    /// 마지막으로 부른 뒤에 전환한 누름들의 대기 분포를 가져간다.
    #[must_use]
    pub fn take_lag(&self) -> Lag {
        self.shared
            .lag
            .lock()
            .map(|mut lag| core::mem::take(&mut *lag))
            .unwrap_or_default()
    }

    /// 탭 스레드를 멈추고, 탭이 살아 있는 동안 한 일을 돌려준다. 대기 분포는 마지막 `take_lag` 뒤의 것이다.
    #[must_use]
    pub fn stop(mut self) -> TapStats {
        self.finish()
    }

    fn finish(&mut self) -> TapStats {
        let Some(thread) = self.thread.take() else {
            return TapStats::default();
        };
        // SAFETY: `run_loop` 는 우리가 retain 한 탭 스레드의 런 루프이고, 멈추는 것은 어느
        // 스레드에서 불러도 된다.
        unsafe { ffi::CFRunLoopStop(self.run_loop.get()) };
        thread.join().unwrap_or_default()
    }
}

impl Drop for Tap {
    fn drop(&mut self) {
        self.finish();
    }
}

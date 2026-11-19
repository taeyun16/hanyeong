//! 런 루프: 키 매핑을 다시 확인할 타이머, 키보드가 붙었다는 알림, 종료 신호.

use core::ffi::c_void;
use core::fmt;
use core::ptr;

use crate::cf::Owned;
use crate::ffi::{self, Ref};
use crate::tray::{self, Tray};

/// 런 루프가 도는 동안 불리는 것들. 모두 런 루프 스레드에서, 겹치지 않게 불린다.
pub trait Events {
    /// 정해 둔 시각이 됐다. 다음 확인까지의 시간(ms)을 돌려준다.
    fn recheck(&mut self) -> u32;
    /// 키보드가 붙었다. 다음 확인까지의 시간(ms)을 돌려준다.
    fn device_changed(&mut self) -> u32;
}

#[derive(Debug)]
pub enum SessionError {
    Timer,
    Watch,
}

impl fmt::Display for SessionError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Timer => out.write_str("타이머를 만들지 못했습니다"),
            Self::Watch => out.write_str("키보드 연결 알림을 등록하지 못했습니다"),
        }
    }
}

struct Context<'a> {
    events: &'a mut dyn Events,
    timer: Ref,
}

impl Context<'_> {
    fn schedule(&self, delay_ms: u32) {
        let seconds = f64::from(delay_ms) / 1_000.0;
        // SAFETY: `timer` 는 `run` 이 만든 살아 있는 타이머다.
        unsafe {
            // 정확할 필요가 없다. 여유를 주면 시스템이 다른 깨어남과 묶는다.
            ffi::CFRunLoopTimerSetTolerance(self.timer, seconds / 10.0);
            ffi::CFRunLoopTimerSetNextFireDate(
                self.timer,
                ffi::CFAbsoluteTimeGetCurrent() + seconds,
            );
        }
    }
}

extern "C" fn timer_fired(_timer: *mut c_void, info: *mut c_void) {
    // SAFETY: `info` 는 `run` 의 스택에 있는 `Context` 이고 런 루프가 도는 동안 살아 있다.
    // 런 루프의 콜백은 한 번에 하나만 돈다.
    if let Some(context) = unsafe { info.cast::<Context<'_>>().as_mut() } {
        let delay = context.events.recheck();
        context.schedule(delay);
    }
}

/// 알림 반복자를 끝까지 읽는다. 다 읽어야 다음 알림이 온다.
fn drain(iterator: u32) {
    loop {
        // SAFETY: `iterator` 는 살아 있는 알림 반복자이고, 받은 객체는 곧바로 놓는다.
        let service = unsafe { ffi::IOIteratorNext(iterator) };
        if service == 0 {
            return;
        }
        // SAFETY: 방금 받은, 우리가 참조 하나를 가진 객체다.
        unsafe { ffi::IOObjectRelease(service) };
    }
}

extern "C" fn device_matched(refcon: *mut c_void, iterator: u32) {
    drain(iterator);
    // SAFETY: `timer_fired` 와 같다.
    if let Some(context) = unsafe { refcon.cast::<Context<'_>>().as_mut() } {
        let delay = context.events.device_changed();
        context.schedule(delay);
    }
}

extern "C" fn stop(_context: *mut c_void) {
    // SAFETY: 메인 런 루프는 프로세스가 사는 동안 살아 있다.
    unsafe { ffi::CFRunLoopStop(ffi::CFRunLoopGetMain()) };
    tray::request_stop();
}

/// SIGINT·SIGTERM·SIGHUP 이 런 루프를 멈추게 한다. 신호 처리기 안에서는 아무것도 하지 않고,
/// 메인 큐에서 멈춘다.
fn stop_on_signals() {
    for number in [ffi::SIGINT, ffi::SIGTERM, ffi::SIGHUP] {
        // SAFETY: 기본 동작(즉시 종료)을 끄고, 같은 신호를 메인 큐의 dispatch 소스로 받는다.
        // 소스는 프로세스가 끝날 때까지 살아 있어야 하므로 놓지 않는다.
        unsafe {
            ffi::signal(number, ffi::SIG_IGN);
            let source = ffi::dispatch_source_create(
                &raw const ffi::DISPATCH_SOURCE_TYPE_SIGNAL,
                number.cast_unsigned() as usize,
                0,
                &raw const ffi::DISPATCH_MAIN_QUEUE,
            );
            if !source.is_null() {
                ffi::dispatch_source_set_event_handler_f(source, stop);
                ffi::dispatch_resume(source);
            }
        }
    }
}

/// 멈추라고 할 때까지 이벤트를 돌린다.
fn pump(tray: Option<&Tray<'_>>) {
    match tray {
        // 아이콘이 있으면 AppKit 이 이벤트를 받아야 메뉴가 열린다.
        Some(tray) => tray.run(),
        // SAFETY: 지금 스레드의 런 루프를 멈출 때까지 돌린다.
        None => unsafe { ffi::CFRunLoopRun() },
    }
}

/// 종료 신호가 오거나 메뉴에서 종료를 고를 때까지 지금 스레드의 런 루프를 돌린다.
/// 메인 스레드에서 부른다.
///
/// # Errors
/// 타이머나 키보드 알림을 만들지 못했을 때.
pub fn run(
    events: &mut dyn Events,
    first_ms: u32,
    tray: Option<&Tray<'_>>,
) -> Result<(), SessionError> {
    let mut context = Context {
        events,
        timer: ptr::null(),
    };
    let context = &raw mut context;
    let info = context.cast::<c_void>();
    let mut timer_context = ffi::TimerContext {
        version: 0,
        info,
        retain: None,
        release: None,
        copy_description: None,
    };
    // 간격은 쓰지 않는다. 불릴 때마다 다음 시각을 다시 정한다. 0 이면 한 번 뒤에 무효가 된다.
    // SAFETY: 콜백의 시그니처가 맞고, `info` 는 이 함수가 끝날 때까지 살아 있다.
    // 반환값은 Create 규칙으로 우리가 소유한다.
    let timer = unsafe {
        Owned::adopt(ffi::CFRunLoopTimerCreate(
            ptr::null(),
            ffi::CFAbsoluteTimeGetCurrent() + f64::from(first_ms) / 1_000.0,
            1.0e9,
            0,
            0,
            timer_fired,
            &raw mut timer_context,
        ))
    }
    .ok_or(SessionError::Timer)?;
    // SAFETY: `context` 는 이 함수의 지역 변수를 가리키고, 아직 콜백이 불리기 전이다.
    unsafe { (*context).timer = timer.get() };

    // SAFETY: 0 은 기본 메인 포트다.
    let port = unsafe { ffi::IONotificationPortCreate(0) };
    if port.is_null() {
        return Err(SessionError::Watch);
    }
    let mut iterator = 0_u32;
    // SAFETY: 이름은 NUL 로 끝나는 상수이고, 매칭 사전의 참조는 등록 함수가 가져간다.
    // `info` 는 알림 포트를 없앨 때까지 살아 있다.
    let registered = unsafe {
        ffi::IOServiceAddMatchingNotification(
            port,
            c"IOServiceFirstMatch".as_ptr(),
            ffi::IOServiceMatching(c"IOHIDEventService".as_ptr()),
            device_matched,
            info,
            &raw mut iterator,
        )
    } == 0;
    if registered {
        // 이미 붙어 있는 것들이 먼저 나온다. 읽어야 알림이 켜진다.
        drain(iterator);
        stop_on_signals();
        // SAFETY: 타이머와 알림 소스는 살아 있고, 지금 스레드의 런 루프에 붙인다.
        unsafe {
            let current = ffi::CFRunLoopGetCurrent();
            ffi::CFRunLoopAddTimer(current, timer.get(), ffi::COMMON_MODES);
            ffi::CFRunLoopAddSource(
                current,
                ffi::IONotificationPortGetRunLoopSource(port),
                ffi::COMMON_MODES,
            );
        }
        pump(tray);
    }
    // SAFETY: 콜백이 `info` 를 더 쓰지 못하게 타이머와 알림을 먼저 없앤다.
    unsafe {
        ffi::CFRunLoopTimerInvalidate(timer.get());
        if iterator != 0 {
            ffi::IOObjectRelease(iterator);
        }
        ffi::IONotificationPortDestroy(port);
    }
    registered.then_some(()).ok_or(SessionError::Watch)
}

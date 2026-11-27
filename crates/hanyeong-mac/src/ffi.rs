//! macOS 프레임워크의 C 선언. 시그니처는 SDK 헤더(CoreFoundation, CoreGraphics, IOKit, HIServices, HIToolbox)에서 옮겼다.

use core::ffi::{c_char, c_int, c_void};

/// CoreFoundation 객체의 포인터. 소유 여부는 받은 함수의 이름(Create/Copy 는 소유)을 따른다.
pub type Ref = *const c_void;
pub type Index = isize;
pub type Boolean = u8;

/// 주소만 쓰는 구조체 상수.
#[repr(C)]
pub struct Opaque {
    _private: [u8; 0],
}

/// `mach_timebase_info_data_t`.
#[repr(C)]
#[derive(Default)]
pub struct MachTimebase {
    pub numer: u32,
    pub denom: u32,
}

/// `thread_time_constraint_policy_data_t`. 시간은 mach 틱이다.
#[repr(C)]
pub struct TimeConstraint {
    pub period: u32,
    pub computation: u32,
    pub constraint: u32,
    pub preemptible: c_int,
}

pub const THREAD_TIME_CONSTRAINT_POLICY: u32 = 2;
pub const THREAD_TIME_CONSTRAINT_POLICY_COUNT: u32 = 4;

#[repr(C)]
pub struct TimerContext {
    pub version: Index,
    pub info: *mut c_void,
    pub retain: Option<extern "C" fn(*const c_void) -> *const c_void>,
    pub release: Option<extern "C" fn(*const c_void)>,
    pub copy_description: Option<extern "C" fn(*const c_void) -> Ref>,
}

pub const UTF8: u32 = 0x0800_0100;
pub const NUMBER_SINT64: Index = 4;

pub const SESSION_TAP: u32 = 1;
pub const HID_TAP: u32 = 0;
pub const HEAD_INSERT: u32 = 0;
pub const TAIL_APPEND: u32 = 1;
pub const TAP_DEFAULT: u32 = 0;

pub const KEY_DOWN: u32 = 10;
pub const KEY_UP: u32 = 11;
pub const TAP_DISABLED_BY_TIMEOUT: u32 = 0xFFFF_FFFE;
pub const TAP_DISABLED_BY_USER_INPUT: u32 = 0xFFFF_FFFF;
pub const KEY_MASK: u64 = (1 << KEY_DOWN) | (1 << KEY_UP);

pub const FIELD_AUTOREPEAT: u32 = 8;
pub const FIELD_KEYCODE: u32 = 9;
pub const FIELD_SOURCE_PID: u32 = 41;
pub const FIELD_USER_DATA: u32 = 42;
/// `kCGEventSourceStateHIDSystemState`: 하드웨어 키보드와 같은 상태를 보는 이벤트 소스.
pub const SOURCE_STATE_HID: i32 = 1;

pub const FLAG_ALPHA_SHIFT: u64 = 0x1_0000;
pub const FLAG_NUMERIC_PAD: u64 = 0x20_0000;
pub const FLAG_SECONDARY_FN: u64 = 0x80_0000;

pub const CLOCK_UPTIME_RAW: c_int = 8;
/// `QOS_CLASS_USER_INTERACTIVE`: 사용자 입력에 바로 반응해야 하는 일.
pub const QOS_USER_INTERACTIVE: u32 = 0x21;
pub const SIGHUP: c_int = 1;
pub const SIGINT: c_int = 2;
pub const SIGTERM: c_int = 15;
pub const SIG_IGN: usize = 1;

pub const DELIVER_IMMEDIATELY: Index = 4;

pub type TapCallback = extern "C" fn(
    proxy: *mut c_void,
    kind: u32,
    event: *mut c_void,
    info: *mut c_void,
) -> *mut c_void;
pub type TimerCallback = extern "C" fn(timer: *mut c_void, info: *mut c_void);
pub type MatchingCallback = extern "C" fn(refcon: *mut c_void, iterator: u32);
pub type NotificationCallback = extern "C" fn(
    center: Ref,
    observer: *mut c_void,
    name: Ref,
    object: *const c_void,
    user_info: Ref,
);

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    #[link_name = "kCFTypeArrayCallBacks"]
    pub static ARRAY_CALLBACKS: Opaque;
    #[link_name = "kCFTypeDictionaryKeyCallBacks"]
    pub static DICTIONARY_KEY_CALLBACKS: Opaque;
    #[link_name = "kCFTypeDictionaryValueCallBacks"]
    pub static DICTIONARY_VALUE_CALLBACKS: Opaque;
    #[link_name = "kCFBooleanTrue"]
    pub static BOOLEAN_TRUE: Ref;
    #[link_name = "kCFRunLoopCommonModes"]
    pub static COMMON_MODES: Ref;
    #[link_name = "kCFRunLoopDefaultMode"]
    pub static DEFAULT_MODE: Ref;

    pub fn CFRetain(object: Ref) -> Ref;
    pub fn CFRelease(object: Ref);
    pub fn CFEqual(left: Ref, right: Ref) -> Boolean;
    pub fn CFGetTypeID(object: Ref) -> usize;
    pub fn CFStringGetTypeID() -> usize;
    pub fn CFNumberGetTypeID() -> usize;
    pub fn CFBooleanGetTypeID() -> usize;
    pub fn CFArrayGetTypeID() -> usize;
    pub fn CFDictionaryGetTypeID() -> usize;

    pub fn CFStringCreateWithBytes(
        allocator: Ref,
        bytes: *const u8,
        length: Index,
        encoding: u32,
        external: Boolean,
    ) -> Ref;
    pub fn CFStringGetCString(
        string: Ref,
        buffer: *mut c_char,
        size: Index,
        encoding: u32,
    ) -> Boolean;
    pub fn CFNumberCreate(allocator: Ref, kind: Index, value: *const c_void) -> Ref;
    pub fn CFNumberGetValue(number: Ref, kind: Index, value: *mut c_void) -> Boolean;
    pub fn CFBooleanGetValue(boolean: Ref) -> Boolean;

    pub fn CFArrayCreate(
        allocator: Ref,
        values: *const Ref,
        count: Index,
        callbacks: *const Opaque,
    ) -> Ref;
    pub fn CFArrayGetCount(array: Ref) -> Index;
    pub fn CFArrayGetValueAtIndex(array: Ref, index: Index) -> Ref;

    pub fn CFDictionaryCreate(
        allocator: Ref,
        keys: *const Ref,
        values: *const Ref,
        count: Index,
        key_callbacks: *const Opaque,
        value_callbacks: *const Opaque,
    ) -> Ref;
    pub fn CFDictionaryCreateMutable(
        allocator: Ref,
        capacity: Index,
        key_callbacks: *const Opaque,
        value_callbacks: *const Opaque,
    ) -> Ref;
    pub fn CFDictionaryCreateMutableCopy(allocator: Ref, capacity: Index, dictionary: Ref) -> Ref;
    pub fn CFDictionaryGetValue(dictionary: Ref, key: Ref) -> Ref;
    pub fn CFDictionarySetValue(dictionary: Ref, key: Ref, value: Ref);
    pub fn CFDictionaryRemoveValue(dictionary: Ref, key: Ref);

    pub fn CFPreferencesCopyAppValue(key: Ref, application: Ref) -> Ref;
    pub fn CFPreferencesSetAppValue(key: Ref, value: Ref, application: Ref);
    pub fn CFPreferencesAppSynchronize(application: Ref) -> Boolean;

    pub fn CFAbsoluteTimeGetCurrent() -> f64;
    pub fn CFRunLoopGetCurrent() -> Ref;
    pub fn CFRunLoopGetMain() -> Ref;
    pub fn CFRunLoopRun();
    pub fn CFRunLoopRunInMode(mode: Ref, seconds: f64, return_after_source: Boolean) -> i32;
    pub fn CFRunLoopStop(run_loop: Ref);
    pub fn CFRunLoopAddSource(run_loop: Ref, source: Ref, mode: Ref);
    pub fn CFRunLoopAddTimer(run_loop: Ref, timer: Ref, mode: Ref);
    pub fn CFRunLoopTimerCreate(
        allocator: Ref,
        fire_date: f64,
        interval: f64,
        flags: usize,
        order: Index,
        callout: TimerCallback,
        context: *mut TimerContext,
    ) -> Ref;
    pub fn CFRunLoopTimerSetNextFireDate(timer: Ref, fire_date: f64);
    pub fn CFRunLoopTimerSetTolerance(timer: Ref, tolerance: f64);
    pub fn CFRunLoopTimerInvalidate(timer: Ref);
    pub fn CFMachPortCreateRunLoopSource(allocator: Ref, port: Ref, order: Index) -> Ref;
    pub fn CFMachPortInvalidate(port: Ref);

    pub fn CFNotificationCenterGetDistributedCenter() -> Ref;
    pub fn CFNotificationCenterAddObserver(
        center: Ref,
        observer: *const c_void,
        callback: NotificationCallback,
        name: Ref,
        object: *const c_void,
        suspension: Index,
    );
    pub fn CFNotificationCenterRemoveEveryObserver(center: Ref, observer: *const c_void);
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    pub fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events: u64,
        callback: TapCallback,
        info: *mut c_void,
    ) -> Ref;
    pub fn CGEventTapEnable(tap: Ref, enable: bool);
    pub fn CGEventGetIntegerValueField(event: *mut c_void, field: u32) -> i64;
    pub fn CGEventSetIntegerValueField(event: *mut c_void, field: u32, value: i64);
    pub fn CGEventGetFlags(event: *mut c_void) -> u64;
    /// 이벤트가 만들어진 시각. 합성 이벤트에서는 단위를 믿을 수 없다.
    pub fn CGEventGetTimestamp(event: *mut c_void) -> u64;
    pub fn CGEventSetFlags(event: *mut c_void, flags: u64);
    pub fn CGEventCreateKeyboardEvent(source: Ref, key: u16, down: bool) -> *mut c_void;
    /// Create 규칙으로 호출자가 소유한다.
    pub fn CGEventSourceCreate(state: i32) -> Ref;
    /// 이 소스로 만든 이벤트에 붙는 사용자 데이터.
    pub fn CGEventSourceSetUserData(source: Ref, data: i64);
    pub fn CGEventPost(tap: u32, event: *mut c_void);
    /// Copy 규칙으로 호출자가 소유한다. 로그인 세션이 없으면 null.
    pub fn CGSessionCopyCurrentDictionary() -> Ref;
}

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    /// 어느 프로세스든 보안 입력을 켜 두었으면 참.
    pub fn IsSecureEventInputEnabled() -> Boolean;
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    #[link_name = "kAXTrustedCheckOptionPrompt"]
    pub static TRUSTED_CHECK_PROMPT: Ref;

    pub fn AXIsProcessTrusted() -> Boolean;
    pub fn AXIsProcessTrustedWithOptions(options: Ref) -> Boolean;
}

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    pub fn IOHIDEventSystemClientCreateSimpleClient(allocator: Ref) -> Ref;
    pub fn IOHIDEventSystemClientCopyServices(client: Ref) -> Ref;
    pub fn IOHIDServiceClientConformsTo(service: Ref, page: u32, usage: u32) -> u32;
    pub fn IOHIDServiceClientCopyProperty(service: Ref, key: Ref) -> Ref;
    pub fn IOHIDServiceClientSetProperty(service: Ref, key: Ref, value: Ref) -> Boolean;
    pub fn IOHIDServiceClientGetRegistryID(service: Ref) -> Ref;

    pub fn IONotificationPortCreate(main_port: u32) -> *mut c_void;
    pub fn IONotificationPortDestroy(port: *mut c_void);
    pub fn IONotificationPortGetRunLoopSource(port: *mut c_void) -> Ref;
    /// `matching` 의 참조 하나를 가져간다.
    pub fn IOServiceMatching(name: *const c_char) -> Ref;
    pub fn IOServiceAddMatchingNotification(
        port: *mut c_void,
        kind: *const c_char,
        matching: Ref,
        callback: MatchingCallback,
        refcon: *mut c_void,
        iterator: *mut u32,
    ) -> c_int;
    pub fn IOIteratorNext(iterator: u32) -> u32;
    pub fn IOObjectRelease(object: u32) -> c_int;
}

// libSystem. 따로 링크하지 않아도 들어온다.
unsafe extern "C" {
    #[link_name = "_dispatch_source_type_signal"]
    pub static DISPATCH_SOURCE_TYPE_SIGNAL: Opaque;
    #[link_name = "_dispatch_main_q"]
    pub static DISPATCH_MAIN_QUEUE: Opaque;

    pub fn dispatch_source_create(
        kind: *const Opaque,
        handle: usize,
        mask: usize,
        queue: *const Opaque,
    ) -> *mut c_void;
    pub fn dispatch_source_set_event_handler_f(
        source: *mut c_void,
        handler: extern "C" fn(*mut c_void),
    );
    pub fn dispatch_resume(object: *mut c_void);
    pub fn signal(signal: c_int, handler: usize) -> usize;
    pub fn clock_gettime_nsec_np(clock: c_int) -> u64;
    pub fn arc4random() -> u32;
    pub fn mach_timebase_info(info: *mut MachTimebase) -> c_int;
    #[link_name = "mach_task_self_"]
    pub static MACH_TASK_SELF: u32;
    pub fn mach_thread_self() -> u32;
    pub fn mach_port_deallocate(task: u32, name: u32) -> c_int;
    pub fn thread_policy_set(
        thread: u32,
        flavor: u32,
        info: *const TimeConstraint,
        count: u32,
    ) -> c_int;
    pub fn pthread_set_qos_class_self_np(class: u32, relative_priority: c_int) -> c_int;
    /// 프로세스 이름을 `buffer` 에 쓰고 그 길이를 돌려준다. 없으면 0.
    pub fn proc_name(pid: c_int, buffer: *mut c_void, size: u32) -> c_int;
}

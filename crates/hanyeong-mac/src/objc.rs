//! Objective-C 런타임을 부르는 최소한의 도우미. AppKit 의 메뉴바 아이콘과 App Nap 끄기에 쓴다.

use core::ffi::{CStr, c_char, c_void};
use core::mem;

pub type Id = *mut c_void;
pub type Sel = *const c_void;
/// 메뉴 항목이 부르는 메서드의 모양: `- (void)name:(id)sender`.
pub type Action = extern "C" fn(this: Id, selector: Sel, sender: Id);

#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {}

#[link(name = "objc")]
unsafe extern "C" {
    fn objc_getClass(name: *const c_char) -> Id;
    fn sel_registerName(name: *const c_char) -> Sel;
    fn objc_msgSend();
    fn objc_allocateClassPair(superclass: Id, name: *const c_char, extra: usize) -> Id;
    fn class_addMethod(class: Id, name: Sel, method: Action, types: *const c_char) -> bool;
    fn objc_registerClassPair(class: Id);
}

pub fn class(name: &CStr) -> Id {
    // SAFETY: 이름은 NUL 로 끝난다. 없는 클래스면 null 이 오고, null 에 보낸 메시지는 null 을 돌려준다.
    unsafe { objc_getClass(name.as_ptr()) }
}

pub fn sel(name: &CStr) -> Sel {
    // SAFETY: 이름은 NUL 로 끝난다.
    unsafe { sel_registerName(name.as_ptr()) }
}

/// `NSObject` 를 물려받아 `actions` 의 메서드를 가진 클래스를 만든다.
pub fn subclass(name: &CStr, actions: &[(Sel, Action)]) -> Id {
    // SAFETY: 이름들은 NUL 로 끝나고, 메서드의 모양은 타입 문자열 "v@:@" 와 같다.
    // 같은 이름이 이미 있으면 null 이 오고 그 뒤의 호출은 아무 일도 하지 않는다.
    unsafe {
        let made = objc_allocateClassPair(class(c"NSObject"), name.as_ptr(), 0);
        if !made.is_null() {
            for (selector, action) in actions {
                class_addMethod(made, *selector, *action, c"v@:@".as_ptr());
            }
            objc_registerClassPair(made);
        }
        made
    }
}

// objc_msgSend 는 받는 메서드의 시그니처로 바꿔 불러야 한다. 아래 함수마다 하나의 모양이다.

/// 인자 없는 메시지.
///
/// # Safety
/// `receiver` 는 null 이거나 살아 있는 객체이고, 메서드는 인자가 없고 객체(또는 void)를 돌려줘야 한다.
pub unsafe fn send(receiver: Id, selector: Sel) -> Id {
    // SAFETY: 호출자가 메서드의 모양을 보장한다.
    unsafe {
        let function: extern "C" fn(Id, Sel) -> Id =
            mem::transmute(objc_msgSend as unsafe extern "C" fn());
        function(receiver, selector)
    }
}

/// 객체 하나를 받는 메시지.
///
/// # Safety
/// `send` 와 같고, 메서드는 포인터 크기의 인자 하나를 받아야 한다.
pub unsafe fn send_id(receiver: Id, selector: Sel, argument: *const c_void) -> Id {
    // SAFETY: 호출자가 메서드의 모양을 보장한다.
    unsafe {
        let function: extern "C" fn(Id, Sel, *const c_void) -> Id =
            mem::transmute(objc_msgSend as unsafe extern "C" fn());
        function(receiver, selector, argument)
    }
}

/// 정수 하나를 받는 메시지.
///
/// # Safety
/// `send` 와 같고, 메서드는 `NSInteger` 나 `BOOL` 하나를 받아야 한다.
pub unsafe fn send_int(receiver: Id, selector: Sel, argument: isize) -> Id {
    // SAFETY: 호출자가 메서드의 모양을 보장한다.
    unsafe {
        let function: extern "C" fn(Id, Sel, isize) -> Id =
            mem::transmute(objc_msgSend as unsafe extern "C" fn());
        function(receiver, selector, argument)
    }
}

/// 실수 하나를 받는 메시지.
///
/// # Safety
/// `send` 와 같고, 메서드는 `CGFloat` 하나를 받아야 한다.
pub unsafe fn send_float(receiver: Id, selector: Sel, argument: f64) -> Id {
    // SAFETY: 호출자가 메서드의 모양을 보장한다.
    unsafe {
        let function: extern "C" fn(Id, Sel, f64) -> Id =
            mem::transmute(objc_msgSend as unsafe extern "C" fn());
        function(receiver, selector, argument)
    }
}

/// `-[NSMenuItem initWithTitle:action:keyEquivalent:]`.
///
/// # Safety
/// `receiver` 는 방금 `alloc` 한 `NSMenuItem` 이고 두 문자열은 살아 있는 `NSString` 이어야 한다.
pub unsafe fn init_menu_item(
    receiver: Id,
    title: *const c_void,
    action: Sel,
    key: *const c_void,
) -> Id {
    // SAFETY: 호출자가 메서드의 모양을 보장한다.
    unsafe {
        let function: extern "C" fn(Id, Sel, *const c_void, Sel, *const c_void) -> Id =
            mem::transmute(objc_msgSend as unsafe extern "C" fn());
        function(
            receiver,
            sel(c"initWithTitle:action:keyEquivalent:"),
            title,
            action,
            key,
        )
    }
}

/// 아무 뜻 없는 앱 정의 이벤트를 만들어 큐 맨 앞에 넣는다. `-[NSApplication stop:]` 은 다음 이벤트가
/// 처리된 뒤에야 `run` 을 끝내므로, 이벤트가 없을 때 깨우는 데 쓴다.
///
/// # Safety
/// `app` 은 `NSApplication` 이어야 하고 메인 스레드에서 불러야 한다.
pub unsafe fn post_wakeup(app: Id) {
    const APPLICATION_DEFINED: usize = 15;
    // SAFETY: 시그니처는 +[NSEvent otherEventWithType:location:modifierFlags:timestamp:windowNumber:
    // context:subtype:data1:data2:] 와 같다. `NSPoint` 는 실수 둘이고 두 실수 인자와 같게 전달된다.
    unsafe {
        let make: extern "C" fn(
            Id,
            Sel,
            usize,
            f64,
            f64,
            usize,
            f64,
            isize,
            Id,
            i16,
            isize,
            isize,
        ) -> Id = mem::transmute(objc_msgSend as unsafe extern "C" fn());
        let event = make(
            class(c"NSEvent"),
            sel(c"otherEventWithType:location:modifierFlags:timestamp:windowNumber:context:subtype:data1:data2:"),
            APPLICATION_DEFINED,
            0.0,
            0.0,
            0,
            0.0,
            0,
            core::ptr::null_mut(),
            0,
            0,
            0,
        );
        let post: extern "C" fn(Id, Sel, Id, bool) =
            mem::transmute(objc_msgSend as unsafe extern "C" fn());
        post(app, sel(c"postEvent:atStart:"), event, true);
    }
}

/// `NSActivityUserInitiatedAllowingIdleSystemSleep | NSActivityLatencyCritical`: App Nap 과 타이머 묶기를
/// 끄고 입출력을 늦추지 않게 하되, 시스템이나 화면이 잠드는 것은 막지 않는다.
const QUICK_ACTIVITY: u64 = 0x00EF_FFFF | 0xFF_0000_0000;

/// 프로세스가 App Nap 에 들지 않게 하는 활동을 시작한다. 돌려준 토큰은 우리가 retain 한 것이고,
/// `end_activity` 에 넘겨야 끝난다. 실패하면 null.
///
/// # Safety
/// `reason` 은 살아 있는 `NSString` 이어야 한다.
pub unsafe fn begin_activity(reason: *const c_void) -> Id {
    // SAFETY: -[NSProcessInfo beginActivityWithOptions:reason:] 는 `NSActivityOptions`(u64) 하나와
    // NSString 하나를 받아 autorelease 된 객체를 돌려준다. 그것을 retain 해 우리 것으로 만든다.
    unsafe {
        let info = send(class(c"NSProcessInfo"), sel(c"processInfo"));
        let begin: extern "C" fn(Id, Sel, u64, *const c_void) -> Id =
            mem::transmute(objc_msgSend as unsafe extern "C" fn());
        let token = begin(
            info,
            sel(c"beginActivityWithOptions:reason:"),
            QUICK_ACTIVITY,
            reason,
        );
        send(token, sel(c"retain"))
    }
}

/// `begin_activity` 로 시작한 활동을 끝내고 토큰을 놓는다. null 이면 아무 일도 하지 않는다.
///
/// # Safety
/// `token` 은 null 이거나 `begin_activity` 가 돌려준, 아직 끝내지 않은 토큰이어야 한다.
pub unsafe fn end_activity(token: Id) {
    if token.is_null() {
        return;
    }
    // SAFETY: -[NSProcessInfo endActivity:] 는 토큰 하나를 받는다. 토큰은 우리가 retain 한 것이다.
    unsafe {
        let info = send(class(c"NSProcessInfo"), sel(c"processInfo"));
        send_id(info, sel(c"endActivity:"), token);
        send(token, sel(c"release"));
    }
}

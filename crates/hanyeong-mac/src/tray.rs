//! 메뉴바 아이콘. 켜져 있는지 보여 주고, 메뉴로 일시정지와 종료를 한다.
//!
//! 툴팁은 달지 않는다. gksdud 의 툴팁이 입력 소스가 바뀔 때마다 다시 떠서 이 도구를 만들게 됐다.
//! 한/영 상태도 보여 주지 않는다: macOS 의 입력 메뉴가 이미 보여 주고, 보여 주려면 입력 소스
//! 변화를 계속 지켜봐야 한다.

use core::ffi::c_void;
use core::marker::PhantomData;
use core::ptr;
use core::sync::atomic::{AtomicPtr, Ordering};

use crate::cf::{self, Owned};
use crate::objc::{self, Id, Sel};
use crate::tap::Tap;

const ACCESSORY_POLICY: isize = 1;
const VARIABLE_LENGTH: f64 = -1.0;
const TITLE: &str = "한";
const PAUSE: &str = "일시정지";
const RESUME: &str = "다시 켜기";
const QUIT: &str = "hanyeong 종료";

struct State {
    app: Id,
    bar: Id,
    item: Id,
    button: Id,
    toggle: Id,
    tap: *const Tap,
    paused: bool,
}

/// 메뉴의 메서드가 찾아오는 곳. 아이콘이 없으면 null 이다.
static STATE: AtomicPtr<State> = AtomicPtr::new(ptr::null_mut());

/// 설치된 아이콘. 버려지면 메뉴바에서 걷어낸다. 탭보다 먼저 버려져야 한다(빌림이 강제한다).
pub struct Tray<'a> {
    state: *mut State,
    _tap: PhantomData<&'a Tap>,
}

fn text(value: &str) -> Option<Owned> {
    // CFString 과 NSString 은 같은 객체다.
    cf::string(value)
}

/// # Safety
/// 메인 스레드에서, `STATE` 가 가리키는 아이콘이 살아 있는 동안만 부른다.
unsafe fn state<'a>() -> Option<&'a mut State> {
    // SAFETY: null 이 아니면 `Tray::install` 이 만든 살아 있는 `State` 다. 메뉴의 메서드와
    // 신호 처리는 모두 메인 스레드에서 차례로 돌므로 겹치는 참조가 없다.
    unsafe { STATE.load(Ordering::Relaxed).as_mut() }
}

extern "C" fn toggle(_this: Id, _selector: Sel, _sender: Id) {
    // SAFETY: AppKit 은 메뉴의 메서드를 메인 스레드에서 부른다.
    let Some(state) = (unsafe { state() }) else {
        return;
    };
    state.paused = !state.paused;
    let Some(title) = text(if state.paused { RESUME } else { PAUSE }) else {
        return;
    };
    // SAFETY: 탭은 아이콘보다 오래 산다. 버튼과 메뉴 항목은 살아 있는 AppKit 객체이고,
    // 두 메서드는 각각 BOOL 하나와 NSString 하나를 받는다.
    unsafe {
        (*state.tap).set_paused(state.paused);
        objc::send_int(
            state.button,
            objc::sel(c"setAppearsDisabled:"),
            isize::from(state.paused),
        );
        objc::send_id(state.toggle, objc::sel(c"setTitle:"), title.get());
    }
}

extern "C" fn quit(_this: Id, _selector: Sel, _sender: Id) {
    request_stop();
}

/// `run` 을 끝낸다. 아이콘이 없으면 아무 일도 하지 않는다. 메인 스레드에서 부른다.
pub fn request_stop() {
    // SAFETY: 메뉴의 메서드와 신호의 dispatch 소스는 메인 스레드에서 돈다.
    if let Some(state) = unsafe { state() } {
        // SAFETY: `app` 은 NSApplication 이고 `stop:` 은 보낸 이(없음) 하나를 받는다.
        unsafe {
            objc::send_id(state.app, objc::sel(c"stop:"), ptr::null());
            objc::post_wakeup(state.app);
        }
    }
}

/// 메뉴 항목 하나를 만들어 메뉴에 넣는다.
///
/// # Safety
/// `menu` 는 NSMenu, `target` 은 `action` 을 가진 객체여야 한다.
unsafe fn add_item(menu: Id, target: Id, title: &str, action: Sel) -> Option<Id> {
    let (title, key) = (text(title)?, text("")?);
    // SAFETY: 호출자의 계약과 각 메서드의 시그니처대로 부른다. 메뉴가 항목을 retain 한다.
    unsafe {
        let item = objc::init_menu_item(
            objc::send(objc::class(c"NSMenuItem"), objc::sel(c"alloc")),
            title.get(),
            action,
            key.get(),
        );
        objc::send_id(item, objc::sel(c"setTarget:"), target);
        objc::send_id(menu, objc::sel(c"addItem:"), item);
        (!item.is_null()).then_some(item)
    }
}

/// 상태 아이콘에 일시정지·종료 메뉴를 단다. 일시정지 항목을 돌려준다.
///
/// # Safety
/// `item` 은 살아 있는 NSStatusItem 이어야 하고 메인 스레드에서 불러야 한다.
unsafe fn add_menu(item: Id) -> Option<Id> {
    let actions = [
        (objc::sel(c"toggle:"), toggle as objc::Action),
        (objc::sel(c"quit:"), quit as objc::Action),
    ];
    let class = objc::subclass(c"HanyeongTrayTarget", &actions);
    // SAFETY: 각 메시지는 AppKit 문서의 시그니처대로다. 대상 객체는 프로세스가 끝날 때까지 산다.
    unsafe {
        let target = objc::send(objc::send(class, objc::sel(c"alloc")), objc::sel(c"init"));
        let menu = objc::send(
            objc::send(objc::class(c"NSMenu"), objc::sel(c"alloc")),
            objc::sel(c"init"),
        );
        let toggle = add_item(menu, target, PAUSE, objc::sel(c"toggle:"));
        let quit = add_item(menu, target, QUIT, objc::sel(c"quit:"));
        objc::send_id(item, objc::sel(c"setMenu:"), menu);
        quit.and(toggle)
    }
}

impl<'a> Tray<'a> {
    /// 메뉴바에 아이콘을 올린다. 메인 스레드에서 부른다.
    #[must_use]
    pub fn install(tap: &'a Tap) -> Option<Self> {
        let title = text(TITLE)?;
        // SAFETY: 각 메시지는 AppKit 문서의 시그니처대로다. 독에 아이콘이 없는 앱으로 만들고,
        // 상태 아이콘은 우리가 retain 해 둔다. 대상 객체는 프로세스가 끝날 때까지 산다.
        let state = unsafe {
            let app = objc::send(
                objc::class(c"NSApplication"),
                objc::sel(c"sharedApplication"),
            );
            objc::send_int(app, objc::sel(c"setActivationPolicy:"), ACCESSORY_POLICY);
            let bar = objc::send(objc::class(c"NSStatusBar"), objc::sel(c"systemStatusBar"));
            let item = objc::send(
                objc::send_float(bar, objc::sel(c"statusItemWithLength:"), VARIABLE_LENGTH),
                objc::sel(c"retain"),
            );
            let button = objc::send(item, objc::sel(c"button"));
            objc::send_id(button, objc::sel(c"setTitle:"), title.get());
            let (Some(toggle), false) = (add_menu(item), button.is_null()) else {
                objc::send_id(bar, objc::sel(c"removeStatusItem:"), item);
                return None;
            };
            Box::into_raw(Box::new(State {
                app,
                bar,
                item,
                button,
                toggle,
                tap: ptr::from_ref(tap),
                paused: false,
            }))
        };
        STATE.store(state, Ordering::Relaxed);
        Some(Self {
            state,
            _tap: PhantomData,
        })
    }

    /// `request_stop` 이 불릴 때까지 AppKit 의 이벤트 루프를 돌린다. 런 루프의 소스들도 함께 돈다.
    pub fn run(&self) {
        // SAFETY: `state` 는 이 아이콘이 살아 있는 동안 유효하고, `run` 은 인자가 없다.
        unsafe { objc::send((*self.state).app, objc::sel(c"run")) };
    }
}

impl Drop for Tray<'_> {
    fn drop(&mut self) {
        STATE.store(ptr::null_mut(), Ordering::Relaxed);
        // SAFETY: `state` 는 `install` 이 만든 Box 이고 이제 아무도 가리키지 않는다.
        // 상태 아이콘은 우리가 retain 한 살아 있는 객체다.
        unsafe {
            let state = Box::from_raw(self.state);
            objc::send_id(
                state.bar,
                objc::sel(c"removeStatusItem:"),
                state.item.cast::<c_void>(),
            );
            objc::send(state.item, objc::sel(c"release"));
        }
    }
}

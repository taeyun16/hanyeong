//! 시스템 단축키 60번(`이전 입력 소스 선택`)을 우리 F-키로 맞추고 되돌린다.
//!
//! 바꾸기 전의 값은 우리 설정 도메인에 남긴다(하위 모듈 `kept`). 되돌릴 때는 지금 값이 우리가 써 넣은
//! 값일 때만 되돌린다. 무엇을 쓸지는 `hanyeong_core::shortcut` 의 `claim`·`release` 가 정하고, 여기서는 그
//! 순서대로 쓰기만 한다. 쓰기의 순서는 `hanyeong-core/tests/shortcut60_replay.rs` 가 옮겨 적어 시험한다.

use core::fmt;
use core::ptr;
use std::process::Command;

use hanyeong_core::shortcut::{self, Claim, Entry, PREVIOUS_SOURCE, Release};

use crate::cf::{self, Owned};
use crate::entry::{build, parse};
use crate::ffi::{self, Ref};

mod kept;
use kept::Kept;

const SYSTEM_DOMAIN: &str = "com.apple.symbolichotkeys";
const SYSTEM_KEY: &str = "AppleSymbolicHotKeys";
/// 바뀐 단축키를 로그아웃 없이 반영한다. `-virtualSession` 이 없으면 마우스 가속 같은 물리 장치
/// 설정까지 다시 적용된다(gksdud 의 docs/tricks.md).
const ACTIVATE: &str =
    "/System/Library/PrivateFrameworks/SystemAdministration.framework/Resources/activateSettings";

#[derive(Debug)]
pub enum HotkeyError {
    /// CoreFoundation 값을 만들지 못했다.
    Encode,
    /// 설정을 저장하지 못했다.
    Save(&'static str),
    /// activateSettings 가 실패했다.
    Activate,
}

impl fmt::Display for HotkeyError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Encode => out.write_str("단축키 값을 만들지 못했습니다"),
            Self::Save(domain) => write!(out, "{domain} 설정을 저장하지 못했습니다"),
            Self::Activate => out.write_str("바뀐 단축키를 반영하지 못했습니다(activateSettings)"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Claimed {
    /// 이미 우리 F-키였다. 아무것도 쓰지 않았다.
    Already,
    /// 원래 값을 남기고 바꿔 썼다.
    Written,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Released {
    /// 우리가 바꾼 적이 없다.
    Untouched,
    /// 원래 값으로 되돌렸다.
    Restored,
    /// 그 뒤에 누군가 바꿨다. 남겨 둔 원래 값만 지웠다.
    Forgotten,
}

struct Names {
    system: Owned,
    key: Owned,
    slot: Owned,
}

impl Names {
    fn new() -> Result<Self, HotkeyError> {
        let text = |text| cf::string(text).ok_or(HotkeyError::Encode);
        Ok(Self {
            system: text(SYSTEM_DOMAIN)?,
            key: text(SYSTEM_KEY)?,
            slot: text(PREVIOUS_SOURCE)?,
        })
    }

    /// 시스템의 단축키 사전 전체.
    fn all(&self) -> Option<Owned> {
        // SAFETY: 키와 도메인은 살아 있는 CFString 이고, 반환값은 Copy 규칙으로 우리가 소유한다.
        unsafe {
            ffi::CFPreferencesAppSynchronize(self.system.get());
            Owned::adopt(ffi::CFPreferencesCopyAppValue(
                self.key.get(),
                self.system.get(),
            ))
        }
    }

    /// 60번 항목을 `entry` 로 바꾼(null 이면 지운) 사전을 저장하고 반영한다.
    fn write(&self, all: Option<&Owned>, entry: Ref) -> Result<(), HotkeyError> {
        let current = all.map_or(ptr::null(), Owned::get);
        // SAFETY: 사전이 있으면 그 사본을, 없으면 CF 객체를 retain 하는 빈 사전을 만든다.
        // 반환값은 Create 규칙으로 우리가 소유한다.
        let copy = unsafe {
            Owned::adopt(if cf::is_dictionary(current) {
                ffi::CFDictionaryCreateMutableCopy(ptr::null(), 0, current)
            } else {
                ffi::CFDictionaryCreateMutable(
                    ptr::null(),
                    0,
                    &raw const ffi::DICTIONARY_KEY_CALLBACKS,
                    &raw const ffi::DICTIONARY_VALUE_CALLBACKS,
                )
            })
        }
        .ok_or(HotkeyError::Encode)?;
        // SAFETY: `copy` 는 방금 만든 변경 가능한 사전이고, 키와 값은 살아 있는 객체다.
        let stored = unsafe {
            if entry.is_null() {
                ffi::CFDictionaryRemoveValue(copy.get(), self.slot.get());
            } else {
                ffi::CFDictionarySetValue(copy.get(), self.slot.get(), entry);
            }
            ffi::CFPreferencesSetAppValue(self.key.get(), copy.get(), self.system.get());
            ffi::CFPreferencesAppSynchronize(self.system.get()) != 0
        };
        if !stored {
            return Err(HotkeyError::Save(SYSTEM_DOMAIN));
        }
        activate()
    }
}

fn activate() -> Result<(), HotkeyError> {
    match Command::new(ACTIVATE)
        .args(["-u", "-virtualSession"])
        .status()
    {
        Ok(status) if status.success() => Ok(()),
        _ => Err(HotkeyError::Activate),
    }
}

/// 지금의 60번 항목.
#[must_use]
pub fn read() -> Option<Entry> {
    let names = Names::new().ok()?;
    let all = names.all()?;
    parse(cf::get(all.get(), PREVIOUS_SOURCE))
}

/// 60번을 `key_code` 로 맞춘다. 이미 그 값이면 아무것도 쓰지 않는다.
///
/// # Errors
/// 값을 만들거나 저장하거나 반영하지 못했을 때.
pub fn claim(key_code: i64) -> Result<Claimed, HotkeyError> {
    let names = Names::new()?;
    let kept = Kept::new().ok_or(HotkeyError::Encode)?;
    let all = names.all();
    let current = all
        .as_ref()
        .map_or(ptr::null(), |all| cf::get(all.get(), PREVIOUS_SOURCE));
    let record = kept.read();
    let (written, fresh) = match shortcut::claim(parse(current).as_ref(), record.written, key_code)
    {
        Claim::Already => return Ok(Claimed::Already),
        Claim::Write { written, fresh } => (written, fresh),
    };
    let entry = build(key_code).ok_or(HotkeyError::Encode)?;
    // 기록을 먼저 남기고 60번을 쓴다. 거꾸로 하면 그 사이에 죽었을 때 되돌릴 값이 없다.
    kept.store(written, if fresh { current } else { record.original() })?;
    names.write(all.as_ref(), entry.get())?;
    Ok(Claimed::Written)
}

/// `claim` 이 바꿨던 60번을 되돌린다. 어느 F-키로 띄웠던 실행이 남긴 것이든 되돌린다.
///
/// # Errors
/// 값을 저장하거나 반영하지 못했을 때.
pub fn release() -> Result<Released, HotkeyError> {
    let names = Names::new()?;
    let kept = Kept::new().ok_or(HotkeyError::Encode)?;
    let record = kept.read();
    let all = if record.written.is_empty() {
        None
    } else {
        names.all()
    };
    let current = all
        .as_ref()
        .map_or(ptr::null(), |all| cf::get(all.get(), PREVIOUS_SOURCE));
    match shortcut::release(parse(current).as_ref(), record.written) {
        Release::Untouched => Ok(Released::Untouched),
        Release::Restore => {
            // 60번을 먼저 되돌리고 기록을 지운다. 거꾸로 하면 그 사이에 죽었을 때 되돌릴 값이 없다.
            names.write(all.as_ref(), record.original())?;
            kept.forget()?;
            Ok(Released::Restored)
        }
        Release::Forget => {
            kept.forget()?;
            Ok(Released::Forgotten)
        }
    }
}

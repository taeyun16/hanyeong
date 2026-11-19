//! CoreFoundation 값을 만들고 읽는 작은 도우미.

use core::ffi::{CStr, c_void};
use core::ptr::{self, NonNull};

use crate::ffi::{self, Ref};

/// 우리가 소유한 CoreFoundation 객체. 버려질 때 놓는다.
pub struct Owned(NonNull<c_void>);

impl Owned {
    /// # Safety
    /// `object` 는 null 이거나, 호출자가 참조 하나를 소유한 살아 있는 CoreFoundation 객체여야 한다.
    pub unsafe fn adopt(object: Ref) -> Option<Self> {
        NonNull::new(object.cast_mut()).map(Self)
    }

    pub const fn get(&self) -> Ref {
        self.0.as_ptr()
    }
}

impl Drop for Owned {
    fn drop(&mut self) {
        // SAFETY: `adopt` 의 계약대로 우리가 소유한 살아 있는 객체다.
        unsafe { ffi::CFRelease(self.0.as_ptr()) }
    }
}

pub fn string(text: &str) -> Option<Owned> {
    let length = isize::try_from(text.len()).ok()?;
    // SAFETY: 포인터와 길이는 살아 있는 `&str` 의 것이고, 반환값은 Create 규칙으로 우리가 소유한다.
    unsafe {
        Owned::adopt(ffi::CFStringCreateWithBytes(
            ptr::null(),
            text.as_ptr(),
            length,
            ffi::UTF8,
            0,
        ))
    }
}

pub fn number(value: i64) -> Option<Owned> {
    // SAFETY: `value` 는 호출 동안 살아 있는 i64 이고 종류를 SInt64 로 알렸다.
    unsafe {
        Owned::adopt(ffi::CFNumberCreate(
            ptr::null(),
            ffi::NUMBER_SINT64,
            (&raw const value).cast(),
        ))
    }
}

fn has_type(value: Ref, wanted: usize) -> bool {
    // SAFETY: null 이 아닌 CoreFoundation 객체에만 묻는다.
    !value.is_null() && unsafe { ffi::CFGetTypeID(value) } == wanted
}

pub fn is_dictionary(value: Ref) -> bool {
    // SAFETY: 인자 없는 조회다.
    has_type(value, unsafe { ffi::CFDictionaryGetTypeID() })
}

pub fn is_array(value: Ref) -> bool {
    // SAFETY: 인자 없는 조회다.
    has_type(value, unsafe { ffi::CFArrayGetTypeID() })
}

/// CFNumber 를 i64 로 읽는다. 숫자가 아니면 `None`.
pub fn int(value: Ref) -> Option<i64> {
    // SAFETY: 인자 없는 조회다.
    if !has_type(value, unsafe { ffi::CFNumberGetTypeID() }) {
        return None;
    }
    let mut out = 0_i64;
    // SAFETY: `value` 는 CFNumber 이고 `out` 은 SInt64 를 받을 자리다.
    let exact =
        unsafe { ffi::CFNumberGetValue(value, ffi::NUMBER_SINT64, (&raw mut out).cast()) } != 0;
    exact.then_some(out)
}

/// CFBoolean 이나 0/1 의 CFNumber 를 읽는다.
pub fn boolean(value: Ref) -> Option<bool> {
    // SAFETY: 인자 없는 조회다.
    if has_type(value, unsafe { ffi::CFBooleanGetTypeID() }) {
        // SAFETY: `value` 는 CFBoolean 이다.
        return Some(unsafe { ffi::CFBooleanGetValue(value) } != 0);
    }
    int(value).map(|number| number != 0)
}

/// CFString 이 `text` 와 같은가.
pub fn string_is(value: Ref, text: &str) -> bool {
    // SAFETY: 인자 없는 조회다.
    if !has_type(value, unsafe { ffi::CFStringGetTypeID() }) {
        return false;
    }
    // SAFETY: 둘 다 살아 있는 CFString 이다.
    string(text).is_some_and(|wanted| unsafe { ffi::CFEqual(value, wanted.get()) } != 0)
}

/// CFString 을 `out` 뒤에 붙인다. 127 바이트를 넘으면 붙이지 않는다.
pub fn push_string(value: Ref, out: &mut String) -> bool {
    // SAFETY: 인자 없는 조회다.
    if !has_type(value, unsafe { ffi::CFStringGetTypeID() }) {
        return false;
    }
    let mut buffer = [0; 128];
    // SAFETY: `value` 는 CFString 이고 버퍼의 포인터와 크기가 맞다.
    let copied = unsafe {
        ffi::CFStringGetCString(value, buffer.as_mut_ptr(), buffer.len() as isize, ffi::UTF8)
    } != 0;
    if !copied {
        return false;
    }
    // SAFETY: 성공한 CFStringGetCString 은 버퍼 안에 NUL 로 끝나는 문자열을 쓴다.
    let text = unsafe { CStr::from_ptr(buffer.as_ptr()) };
    text.to_str().is_ok_and(|text| {
        out.push_str(text);
        true
    })
}

pub fn array(items: &[Ref]) -> Option<Owned> {
    let count = isize::try_from(items.len()).ok()?;
    // SAFETY: `items` 는 살아 있는 CoreFoundation 객체 `count` 개이고, 콜백 상수가 그것들을 retain 한다.
    unsafe {
        Owned::adopt(ffi::CFArrayCreate(
            ptr::null(),
            items.as_ptr(),
            count,
            &raw const ffi::ARRAY_CALLBACKS,
        ))
    }
}

pub fn dictionary(keys: &[Ref], values: &[Ref]) -> Option<Owned> {
    if keys.len() != values.len() {
        return None;
    }
    let count = isize::try_from(keys.len()).ok()?;
    // SAFETY: 두 배열은 길이가 같은 살아 있는 객체들이고, 콜백 상수가 그것들을 retain 한다.
    unsafe {
        Owned::adopt(ffi::CFDictionaryCreate(
            ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            count,
            &raw const ffi::DICTIONARY_KEY_CALLBACKS,
            &raw const ffi::DICTIONARY_VALUE_CALLBACKS,
        ))
    }
}

/// 사전에서 문자열 키로 값을 꺼낸다. 돌려준 값은 사전이 살아 있는 동안만 쓴다.
pub fn get(dictionary: Ref, key: &str) -> Ref {
    if !is_dictionary(dictionary) {
        return ptr::null();
    }
    // SAFETY: `dictionary` 는 CFDictionary 이고 키는 살아 있는 CFString 이다.
    string(key).map_or(ptr::null(), |key| unsafe {
        ffi::CFDictionaryGetValue(dictionary, key.get())
    })
}

/// 배열의 원소들을 차례로 넘긴다. `limit` 개까지만 본다.
pub fn each(array: Ref, limit: usize, mut visit: impl FnMut(Ref)) {
    if !is_array(array) {
        return;
    }
    // SAFETY: `array` 는 CFArray 다.
    let count = unsafe { ffi::CFArrayGetCount(array) };
    for index in 0..count.min(isize::try_from(limit).unwrap_or(isize::MAX)) {
        // SAFETY: `index` 는 0 이상 count 미만이다.
        visit(unsafe { ffi::CFArrayGetValueAtIndex(array, index) });
    }
}

pub fn count(array: Ref) -> usize {
    if !is_array(array) {
        return 0;
    }
    // SAFETY: `array` 는 CFArray 다.
    usize::try_from(unsafe { ffi::CFArrayGetCount(array) }).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_come_back_out_including_korean() {
        let name = string("한영 Keyboard").unwrap();
        let mut out = String::from("> ");
        assert!(push_string(name.get(), &mut out));
        assert_eq!(out, "> 한영 Keyboard");
        assert!(string_is(name.get(), "한영 Keyboard"));
        assert!(!string_is(name.get(), "한영"));
    }

    #[test]
    fn a_value_of_the_wrong_type_is_refused_not_misread() {
        let text = string("80").unwrap();
        let eighty = number(80).unwrap();
        assert_eq!(int(text.get()), None);
        assert_eq!(int(eighty.get()), Some(80));
        assert_eq!(boolean(text.get()), None);
        assert_eq!(boolean(number(0).unwrap().get()), Some(false));
        assert!(!string_is(eighty.get(), "80"));
        let mut out = String::new();
        assert!(!push_string(eighty.get(), &mut out));
        assert!(!push_string(ptr::null(), &mut out));
        assert_eq!(int(ptr::null()), None);
    }

    #[test]
    fn arrays_are_walked_in_order_up_to_the_limit() {
        let numbers = [number(1).unwrap(), number(2).unwrap(), number(3).unwrap()];
        let list = array(&numbers.each_ref().map(Owned::get)).unwrap();
        let mut seen = Vec::new();
        each(list.get(), 2, |item| seen.push(int(item)));
        assert_eq!(seen, [Some(1), Some(2)]);
        assert_eq!(count(list.get()), 3);
        assert_eq!(count(numbers[0].get()), 0);
    }

    #[test]
    fn a_long_string_is_not_truncated_into_the_output() {
        let long = "가".repeat(60);
        let mut out = String::new();
        assert!(!push_string(string(&long).unwrap().get(), &mut out));
        assert!(out.is_empty());
    }
}

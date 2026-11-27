//! 보안 입력(Secure Event Input)을 읽는다. 켜져 있으면 이벤트 탭에 키가 오지 않는다.
//!
//! 켜짐 여부는 `IsSecureEventInputEnabled` 로 본다. 켠 프로세스는 로그인 세션 사전의
//! `kCGSSessionSecureInputPID` 로 본다. `ioreg` 의 같은 값은 바뀐 뒤에도 옛 번호가 남아 있었다
//! (2026-11-27: 세션 사전은 27092, ioreg 는 2217).

use core::ffi::{c_int, c_void};

use hanyeong_core::secure::Secure;

use crate::cf::{self, Owned};
use crate::ffi;

const OWNER_KEY: &str = "kCGSSessionSecureInputPID";
/// `proc_name` 이 쓰는 이름의 최대 길이(`2 * MAXCOMLEN`)와 끝의 0.
const NAME_BUFFER: usize = 33;

/// 지금의 보안 입력 상태.
#[must_use]
pub fn state() -> Secure {
    // SAFETY: 인자 없는 조회다.
    if unsafe { ffi::IsSecureEventInputEnabled() } == 0 {
        return Secure::Off;
    }
    // SAFETY: 인자 없는 조회이고, 반환값은 Copy 규칙으로 우리가 소유한다.
    let session = unsafe { Owned::adopt(ffi::CGSessionCopyCurrentDictionary()) };
    let pid = session
        .as_ref()
        .and_then(|session| cf::int(cf::get(session.get(), OWNER_KEY)))
        .unwrap_or(0);
    Secure::On { pid }
}

/// 프로세스 이름을 `out` 에 더한다. 찾지 못하면 거짓이고 `out` 은 그대로다.
pub fn push_process_name(pid: i64, out: &mut String) -> bool {
    let Ok(pid) = c_int::try_from(pid) else {
        return false;
    };
    if pid <= 0 {
        return false;
    }
    let mut buffer = [0_u8; NAME_BUFFER];
    // SAFETY: `buffer` 는 살아 있는 배열이고 그 크기를 알린다. 함수는 그 안에만 쓴다.
    let written = unsafe {
        ffi::proc_name(
            pid,
            buffer.as_mut_ptr().cast::<c_void>(),
            u32::try_from(buffer.len()).unwrap_or(0),
        )
    };
    let Some(name) = usize::try_from(written)
        .ok()
        .and_then(|length| buffer.get(..length))
        .and_then(|bytes| str::from_utf8(bytes).ok())
        .filter(|name| !name.is_empty())
    else {
        return false;
    };
    out.push_str(name);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn our_own_name_is_found_and_a_missing_process_is_not() {
        let mut name = String::new();
        assert!(push_process_name(i64::from(std::process::id()), &mut name));
        assert!(!name.is_empty());
        let mut none = String::new();
        assert!(!push_process_name(0, &mut none));
        assert!(!push_process_name(-1, &mut none));
        assert!(!push_process_name(i64::MAX, &mut none));
        assert!(none.is_empty());
    }
}

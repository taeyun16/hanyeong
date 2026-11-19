//! 접근성 권한. 이벤트 탭을 만들고 키 이벤트를 보내려면 필요하다.

use crate::cf;
use crate::ffi;

/// 이 프로세스가 접근성 권한을 받았는가.
#[must_use]
pub fn trusted() -> bool {
    // SAFETY: 인자 없는 조회다.
    unsafe { ffi::AXIsProcessTrusted() != 0 }
}

/// 권한이 없으면 시스템이 설정으로 안내하는 창을 띄우게 한다. 지금의 권한 여부를 돌려준다.
pub fn request() -> bool {
    // SAFETY: 프레임워크가 내보내는 상수 둘을 읽는다.
    let (key, yes) = unsafe { (ffi::TRUSTED_CHECK_PROMPT, ffi::BOOLEAN_TRUE) };
    // SAFETY: `options` 는 살아 있는 사전이다.
    cf::dictionary(&[key], &[yes])
        .is_some_and(|options| unsafe { ffi::AXIsProcessTrustedWithOptions(options.get()) != 0 })
}

const OWN_DOMAIN: &str = "dev.hanyeong";
const PROMPTED: &str = "trust.prompted";
const SETTINGS_URL: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";

/// 시스템 설정의 손쉬운 사용 화면을 연다.
pub fn open_settings() -> bool {
    std::process::Command::new("/usr/bin/open")
        .arg(SETTINGS_URL)
        .status()
        .is_ok_and(|status| status.success())
}

/// 권한을 달라고 이미 안내했는가. 로그인 항목으로 되풀이해 뜰 때 창을 거듭 열지 않으려고 남긴다.
#[must_use]
pub fn prompted() -> bool {
    let (Some(key), Some(domain)) = (cf::string(PROMPTED), cf::string(OWN_DOMAIN)) else {
        return false;
    };
    // SAFETY: 키와 도메인은 살아 있는 CFString 이고, 반환값은 Copy 규칙으로 우리가 소유한다.
    unsafe { cf::Owned::adopt(ffi::CFPreferencesCopyAppValue(key.get(), domain.get())) }
        .is_some_and(|value| cf::boolean(value.get()) == Some(true))
}

/// 안내했다는 표시를 남기거나 지운다.
pub fn set_prompted(prompted: bool) -> bool {
    let (Some(key), Some(domain)) = (cf::string(PROMPTED), cf::string(OWN_DOMAIN)) else {
        return false;
    };
    // SAFETY: 키와 도메인은 살아 있는 CFString 이고, 값은 상수 CFBoolean 이거나 null(지움)이다.
    unsafe {
        let value = if prompted {
            ffi::BOOLEAN_TRUE
        } else {
            core::ptr::null()
        };
        ffi::CFPreferencesSetAppValue(key.get(), value, domain.get());
        ffi::CFPreferencesAppSynchronize(domain.get()) != 0
    }
}

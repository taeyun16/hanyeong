//! 60번을 바꾸기 전의 값과 우리가 써 넣은 F-키들을 우리 설정 도메인에 남긴다.
//!
//! 둘을 한 값으로, 한 번의 쓰기로 남긴다. 표시와 원래 값을 따로 쓰던 때에는 그 사이에 죽으면 표시만
//! 남아, 다음 실행이 끝날 때 60번 항목을 지웠다(2026-11-17 에 고친 발견 3).

use core::ptr;

use hanyeong_core::keys::TargetKey;
use hanyeong_core::shortcut::Written;

use crate::cf::{self, Owned};
use crate::ffi::{self, Ref};
use crate::hotkey::HotkeyError;

pub const DOMAIN: &str = "dev.hanyeong";
const RECORD: &str = "shortcut60.record";
const WRITTEN: &str = "written";
const ORIGINAL: &str = "original";
/// 2026-11-17 까지의 형식: 표시와 원래 값을 따로 남겼다. 읽기만 하고, 새 기록을 쓰거나 지울 때 함께 지운다.
const LEGACY_SAVED: &str = "shortcut60.saved";
const LEGACY_ORIGINAL: &str = "shortcut60.original";

/// 남아 있는 기록.
pub struct Record {
    /// 우리가 60번에 써 넣었을 수 있는 F-키들. 비어 있으면 남긴 것이 없다.
    pub written: Written,
    /// `original` 이 가리키는 값을 살려 둔다.
    _holder: Option<Owned>,
    original: Ref,
}

impl Record {
    /// 원래 값. 60번 항목이 없었으면 null 이다. 이 기록이 살아 있는 동안만 쓴다.
    #[must_use]
    pub const fn original(&self) -> Ref {
        self.original
    }
}

pub struct Kept {
    domain: Owned,
    record: Owned,
    legacy_saved: Owned,
    legacy_original: Owned,
}

impl Kept {
    pub fn new() -> Option<Self> {
        Some(Self {
            domain: cf::string(DOMAIN)?,
            record: cf::string(RECORD)?,
            legacy_saved: cf::string(LEGACY_SAVED)?,
            legacy_original: cf::string(LEGACY_ORIGINAL)?,
        })
    }

    fn value(&self, key: &Owned) -> Option<Owned> {
        // SAFETY: 키와 도메인은 살아 있는 CFString 이고, 반환값은 Copy 규칙으로 우리가 소유한다.
        unsafe { Owned::adopt(ffi::CFPreferencesCopyAppValue(key.get(), self.domain.get())) }
    }

    /// 남아 있는 기록을 읽는다. 새 형식이 있으면 그것을, 없으면 옛 형식을 본다.
    pub fn read(&self) -> Record {
        if let Some(value) = self.value(&self.record)
            && cf::is_dictionary(value.get())
        {
            let mut written = Written::none();
            cf::each(
                cf::get(value.get(), WRITTEN),
                TargetKey::ALL.len(),
                |item| {
                    if let Some(key_code) = cf::int(item) {
                        written = written.with(key_code);
                    }
                },
            );
            return Record {
                written,
                original: cf::get(value.get(), ORIGINAL),
                _holder: Some(value),
            };
        }
        let saved = self
            .value(&self.legacy_saved)
            .is_some_and(|value| cf::boolean(value.get()) == Some(true));
        if saved {
            let value = self.value(&self.legacy_original);
            return Record {
                written: Written::every_target(),
                original: value.as_ref().map_or(ptr::null(), Owned::get),
                _holder: value,
            };
        }
        Record {
            written: Written::none(),
            original: ptr::null(),
            _holder: None,
        }
    }

    /// 기록을 한 번의 쓰기로 남긴다. `original` 이 null 이면 60번 항목이 없었다는 뜻이다.
    ///
    /// # Errors
    /// 값을 만들거나 저장하지 못했을 때.
    pub fn store(&self, written: Written, original: Ref) -> Result<(), HotkeyError> {
        let mut numbers = Vec::with_capacity(TargetKey::ALL.len());
        for key_code in written.keys() {
            numbers.push(cf::number(key_code).ok_or(HotkeyError::Encode)?);
        }
        let items: Vec<Ref> = numbers.iter().map(Owned::get).collect();
        let list = cf::array(&items).ok_or(HotkeyError::Encode)?;
        let names = [cf::string(WRITTEN), cf::string(ORIGINAL)];
        let [Some(written_name), Some(original_name)] = names else {
            return Err(HotkeyError::Encode);
        };
        let value = if original.is_null() {
            cf::dictionary(&[written_name.get()], &[list.get()])
        } else {
            cf::dictionary(
                &[written_name.get(), original_name.get()],
                &[list.get(), original],
            )
        }
        .ok_or(HotkeyError::Encode)?;
        // SAFETY: 키·도메인은 살아 있는 CFString 이고, 값은 방금 만든 살아 있는 사전이다.
        unsafe {
            ffi::CFPreferencesSetAppValue(self.record.get(), value.get(), self.domain.get());
        }
        // 옛 형식의 값은 새 기록을 쓴 뒤에 지운다. 그 사이에 죽어도 새 기록이 앞선다.
        self.drop_legacy();
        self.synchronize()
    }

    fn synchronize(&self) -> Result<(), HotkeyError> {
        // SAFETY: 도메인은 살아 있는 CFString 이다.
        let stored = unsafe { ffi::CFPreferencesAppSynchronize(self.domain.get()) != 0 };
        stored.then_some(()).ok_or(HotkeyError::Save(DOMAIN))
    }

    /// 기록을 지운다.
    ///
    /// # Errors
    /// 저장하지 못했을 때.
    pub fn forget(&self) -> Result<(), HotkeyError> {
        // 옛 형식의 값을 먼저 지운다. 새 기록을 먼저 지우고 죽으면 옛 값이 기록으로 되살아난다.
        self.drop_legacy();
        // SAFETY: 키·도메인은 살아 있는 CFString 이고, null 은 그 키를 지운다.
        unsafe {
            ffi::CFPreferencesSetAppValue(self.record.get(), ptr::null(), self.domain.get());
        }
        self.synchronize()
    }

    /// 옛 형식의 두 값을 지운다. 표시를 먼저 지운다: 표시가 없으면 남은 값은 읽히지 않는다.
    fn drop_legacy(&self) {
        // SAFETY: 키·도메인은 살아 있는 CFString 이고, null 은 그 키를 지운다.
        unsafe {
            ffi::CFPreferencesSetAppValue(self.legacy_saved.get(), ptr::null(), self.domain.get());
            ffi::CFPreferencesSetAppValue(
                self.legacy_original.get(),
                ptr::null(),
                self.domain.get(),
            );
        }
    }
}

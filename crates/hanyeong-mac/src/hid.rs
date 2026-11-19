//! 키보드마다의 `UserKeyMapping` 을 읽고 쓴다. `hidutil property --set` 이 하는 일과 같다.

use core::marker::PhantomData;
use core::ptr;

use hanyeong_core::mapping::{MAX_MAPPINGS, Pair};

use crate::cf::{self, Owned};
use crate::ffi::{self, Ref};

/// 한 번에 다루는 키보드 수의 상한.
pub const MAX_KEYBOARDS: usize = 64;
/// HID 서비스 목록에서 훑는 항목 수의 상한. 키보드가 아닌 서비스가 대부분이다.
const MAX_SERVICES: usize = 4_096;

const PAGE_GENERIC_DESKTOP: u32 = 1;
const USAGE_KEYBOARD: u32 = 6;
const USER_KEY_MAPPING: &str = "UserKeyMapping";
const MAPPING_SRC: &str = "HIDKeyboardModifierMappingSrc";
const MAPPING_DST: &str = "HIDKeyboardModifierMappingDst";

/// 지금 붙어 있는 HID 서비스의 목록. 서비스는 그것을 만든 클라이언트보다 오래 살 수 없다.
pub struct Keyboards {
    _client: Owned,
    services: Owned,
}

/// 목록 안의 키보드 하나.
pub struct Keyboard<'a> {
    service: Ref,
    _owner: PhantomData<&'a Keyboards>,
}

impl Keyboards {
    /// 목록을 새로 읽는다. 클라이언트는 만들 때의 서비스만 알기 때문에 매번 새로 만든다.
    #[must_use]
    pub fn open() -> Option<Self> {
        // SAFETY: 기본 할당자로 만들고, 반환값은 Create 규칙으로 우리가 소유한다.
        let client =
            unsafe { Owned::adopt(ffi::IOHIDEventSystemClientCreateSimpleClient(ptr::null())) }?;
        // SAFETY: `client` 는 살아 있는 클라이언트이고, 반환값은 Copy 규칙으로 우리가 소유한다.
        let services =
            unsafe { Owned::adopt(ffi::IOHIDEventSystemClientCopyServices(client.get())) }?;
        Some(Self {
            _client: client,
            services,
        })
    }

    /// 키보드로 등록된 서비스를 차례로 넘긴다.
    pub fn each(&self, mut visit: impl FnMut(&Keyboard<'_>)) {
        let mut seen = 0_usize;
        cf::each(self.services.get(), MAX_SERVICES, |service| {
            // SAFETY: `service` 는 살아 있는 목록의 서비스다.
            let keyboard = unsafe {
                ffi::IOHIDServiceClientConformsTo(service, PAGE_GENERIC_DESKTOP, USAGE_KEYBOARD)
            } != 0;
            if keyboard && seen < MAX_KEYBOARDS {
                seen = seen.saturating_add(1);
                visit(&Keyboard {
                    service,
                    _owner: PhantomData,
                });
            }
        });
    }
}

impl Keyboard<'_> {
    fn property(&self, key: &str) -> Option<Owned> {
        let key = cf::string(key)?;
        // SAFETY: 서비스와 키는 살아 있고, 반환값은 Copy 규칙으로 우리가 소유한다.
        unsafe { Owned::adopt(ffi::IOHIDServiceClientCopyProperty(self.service, key.get())) }
    }

    /// 레지스트리 ID. 다시 붙으면 바뀐다.
    #[must_use]
    pub fn id(&self) -> Option<u64> {
        // SAFETY: 서비스는 살아 있고, Get 규칙이라 반환값을 소유하지 않는다.
        let id = unsafe { ffi::IOHIDServiceClientGetRegistryID(self.service) };
        cf::int(id).map(i64::cast_unsigned)
    }

    /// 제품 이름을 `out` 뒤에 붙인다.
    pub fn push_name(&self, out: &mut String) -> bool {
        self.property("Product")
            .is_some_and(|name| cf::push_string(name.get(), out))
    }

    /// 지금의 매핑 목록. 읽을 수 없는 모양이면 `None`.
    #[must_use]
    pub fn mappings(&self) -> Option<Vec<Pair>> {
        let Some(value) = self.property(USER_KEY_MAPPING) else {
            return Some(Vec::new());
        };
        if !cf::is_array(value.get()) {
            return None;
        }
        // 상한보다 하나 더 읽어야 상한을 넘었는지 판단할 수 있다.
        let limit = MAX_MAPPINGS.saturating_add(1);
        let mut list = Vec::with_capacity(cf::count(value.get()).min(limit));
        let mut readable = true;
        cf::each(value.get(), limit, |item| {
            let src = cf::int(cf::get(item, MAPPING_SRC));
            let dst = cf::int(cf::get(item, MAPPING_DST));
            match (src, dst) {
                (Some(src), Some(dst)) => list.push(Pair {
                    src: src.cast_unsigned(),
                    dst: dst.cast_unsigned(),
                }),
                _ => readable = false,
            }
        });
        readable.then_some(list)
    }

    /// 매핑 목록을 통째로 바꿔 쓴다.
    #[must_use]
    pub fn set_mappings(&self, list: &[Pair]) -> bool {
        let mut entries = Vec::with_capacity(list.len());
        for pair in list {
            let Some(entry) = entry(*pair) else {
                return false;
            };
            entries.push(entry);
        }
        let items: Vec<Ref> = entries.iter().map(Owned::get).collect();
        let (Some(array), Some(key)) = (cf::array(&items), cf::string(USER_KEY_MAPPING)) else {
            return false;
        };
        // SAFETY: 서비스·키·배열 모두 살아 있는 객체다.
        unsafe { ffi::IOHIDServiceClientSetProperty(self.service, key.get(), array.get()) != 0 }
    }
}

fn entry(pair: Pair) -> Option<Owned> {
    let keys = [cf::string(MAPPING_SRC)?, cf::string(MAPPING_DST)?];
    let values = [
        cf::number(pair.src.cast_signed())?,
        cf::number(pair.dst.cast_signed())?,
    ];
    cf::dictionary(
        &keys.each_ref().map(Owned::get),
        &values.each_ref().map(Owned::get),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // hidutil 이 쓰는 것과 같은 모양이어야 한다: 두 키에 64비트 usage 가 그대로 들어간다.
    #[test]
    fn a_mapping_entry_carries_both_usages_under_the_hid_keys() {
        let built = entry(Pair {
            src: 0x7_0000_00E7,
            dst: 0x7_0000_006E,
        })
        .unwrap();
        assert_eq!(
            cf::int(cf::get(built.get(), MAPPING_SRC)),
            Some(30_064_771_303)
        );
        assert_eq!(
            cf::int(cf::get(built.get(), MAPPING_DST)),
            Some(30_064_771_182)
        );
        assert_eq!(MAPPING_SRC, "HIDKeyboardModifierMappingSrc");
        assert_eq!(MAPPING_DST, "HIDKeyboardModifierMappingDst");
    }
}

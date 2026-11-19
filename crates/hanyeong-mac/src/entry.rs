//! 단축키 항목 하나(`{enabled, value: {type, parameters}}`)를 읽고 만든다.

use hanyeong_core::shortcut::{self, Entry};

use crate::cf::{self, Owned};
use crate::ffi::{self, Ref};

/// 항목 하나에서 판단에 쓰는 값을 읽는다.
pub fn parse(entry: Ref) -> Option<Entry> {
    if !cf::is_dictionary(entry) {
        return None;
    }
    let value = cf::get(entry, "value");
    let list = cf::get(value, "parameters");
    let mut parameters = [0_i64; 3];
    let mut read = 0_usize;
    if cf::count(list) != parameters.len() {
        return None;
    }
    cf::each(list, parameters.len(), |item| {
        if let (Some(slot), Some(number)) = (parameters.get_mut(read), cf::int(item)) {
            *slot = number;
            read = read.saturating_add(1);
        }
    });
    (read == parameters.len()).then(|| Entry {
        enabled: cf::boolean(cf::get(entry, "enabled")) == Some(true),
        standard: cf::string_is(cf::get(value, "type"), "standard"),
        parameters,
    })
}

/// 우리가 쓸 항목을 만든다.
pub fn build(key_code: i64) -> Option<Owned> {
    let [character, code, modifiers] = shortcut::wanted(key_code);
    let numbers = [
        cf::number(character)?,
        cf::number(code)?,
        cf::number(modifiers)?,
    ];
    let parameters = cf::array(&numbers.each_ref().map(Owned::get))?;
    let keys = [cf::string("parameters")?, cf::string("type")?];
    let kind = cf::string("standard")?;
    let value = cf::dictionary(
        &keys.each_ref().map(Owned::get),
        &[parameters.get(), kind.get()],
    )?;
    let keys = [cf::string("enabled")?, cf::string("value")?];
    // SAFETY: 프레임워크가 내보내는 상수 CFBoolean 을 읽는다.
    let enabled = unsafe { ffi::BOOLEAN_TRUE };
    cf::dictionary(&keys.each_ref().map(Owned::get), &[enabled, value.get()])
}

#[cfg(test)]
mod tests {
    use core::ptr;

    use super::*;

    const F19: i64 = 80;

    fn entry(enabled: bool, kind: &str, parameters: &[i64]) -> Owned {
        let numbers: Vec<Owned> = parameters
            .iter()
            .map(|value| cf::number(*value).unwrap())
            .collect();
        let items: Vec<Ref> = numbers.iter().map(Owned::get).collect();
        let list = cf::array(&items).unwrap();
        let keys = [
            cf::string("parameters").unwrap(),
            cf::string("type").unwrap(),
        ];
        let kind = cf::string(kind).unwrap();
        let value =
            cf::dictionary(&keys.each_ref().map(Owned::get), &[list.get(), kind.get()]).unwrap();
        let keys = [cf::string("enabled").unwrap(), cf::string("value").unwrap()];
        // macOS 는 enabled 를 숫자 0/1 로 저장하기도 한다.
        let enabled = cf::number(i64::from(enabled)).unwrap();
        cf::dictionary(
            &keys.each_ref().map(Owned::get),
            &[enabled.get(), value.get()],
        )
        .unwrap()
    }

    #[test]
    fn the_entry_we_build_reads_back_as_ours() {
        let built = build(F19).unwrap();
        let parsed = parse(built.get()).unwrap();
        assert_eq!(parsed.parameters, [65_535, F19, 0]);
        assert!(shortcut::owns(Some(&parsed), F19));
        assert!(!shortcut::owns(Some(&parsed), 79));
    }

    // 이 기기에서 gksdud 가 남겨 둔 원래 값: ⌃ + Space.
    #[test]
    fn control_space_reads_as_someone_elses() {
        let parsed = parse(entry(true, "standard", &[32, 49, 262_144]).get()).unwrap();
        assert_eq!(parsed.parameters, [32, 49, 262_144]);
        assert!(parsed.enabled && parsed.standard);
        assert!(!shortcut::owns(Some(&parsed), F19));
    }

    #[test]
    fn disabled_and_non_standard_entries_keep_those_facts() {
        let disabled = parse(entry(false, "standard", &[65_535, F19, 0]).get()).unwrap();
        assert!(!disabled.enabled);
        let modifier = parse(entry(true, "modifier", &[65_535, F19, 0]).get()).unwrap();
        assert!(!modifier.standard);
    }

    #[test]
    fn an_entry_of_another_shape_is_not_read() {
        assert!(parse(entry(true, "standard", &[65_535, F19]).get()).is_none());
        assert!(parse(cf::string("60").unwrap().get()).is_none());
        assert!(parse(ptr::null()).is_none());
    }
}

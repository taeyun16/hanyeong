//! `status` 와 `probe`: 읽기만 하거나, 재서 보여 준다.

use hanyeong_core::args::{Keys, Probe, ProbeMode};
use hanyeong_core::mapping::Pair;
use hanyeong_core::shortcut;
use hanyeong_core::stats;
use hanyeong_mac::hid::Keyboards;
use hanyeong_mac::{hotkey, probe, trust};

use crate::rivals;

pub fn status(keys: Keys) -> bool {
    let want = Pair {
        src: keys.source.usage(),
        dst: keys.target.usage(),
    };
    println!("한영 키: {} → {}", keys.source.name(), keys.target.name());
    println!(
        "접근성 권한: {}",
        if trust::trusted() { "있음" } else { "없음" }
    );
    for name in ["hanyeong", "gksdud"] {
        println!("{name} 실행 중: {}", rivals::running(name));
    }
    let entry = hotkey::read();
    match &entry {
        Some(entry) => println!(
            "입력 소스 단축키(60): {:?}, 켜짐 {}, {}",
            entry.parameters,
            entry.enabled,
            if shortcut::owns(Some(entry), keys.target.key_code()) {
                "대상 F-키로 맞춰져 있음"
            } else {
                "대상 F-키가 아님"
            }
        ),
        None => println!("입력 소스 단축키(60): 읽을 수 없거나 없음"),
    }
    let Some(keyboards) = Keyboards::open() else {
        eprintln!("hanyeong: 키보드 목록을 읽지 못했습니다");
        return false;
    };
    let (mut total, mut mapped) = (0_usize, 0_usize);
    keyboards.each(|keyboard| {
        let mut name = String::new();
        keyboard.push_name(&mut name);
        let state = match keyboard.mappings() {
            Some(list) if list.contains(&want) => {
                mapped = mapped.saturating_add(1);
                "걸려 있음"
            }
            Some(_) => "없음",
            None => "읽을 수 없음",
        };
        total = total.saturating_add(1);
        println!(
            "키보드 {:#x} {name}: 한영 키 매핑 {state}",
            keyboard.id().unwrap_or(0)
        );
    });
    println!("키보드 {total} 대 가운데 {mapped} 대에 걸려 있음");
    true
}

/// ns 를 소수 셋째 자리까지의 ms 로 적는다.
fn ms(ns: u64) -> String {
    format!("{}.{:03}", ns / 1_000_000, ns % 1_000_000 / 1_000)
}

pub fn probe(probe: Probe) -> bool {
    let mut samples = Vec::with_capacity(probe.count as usize);
    let (label, sent) = match probe.mode {
        ProbeMode::Switch => (
            "switch",
            probe::switch_latency(
                probe.target.key_code(),
                probe.count,
                probe.hold_ms,
                probe.gap_ms,
                &mut samples,
            ),
        ),
        ProbeMode::Passthrough => (
            "passthrough",
            probe::passthrough_latency(probe.count, probe.gap_ms, &mut samples),
        ),
        ProbeMode::React => (
            "react",
            probe::react_latency(
                probe.target.key_code(),
                probe.count,
                probe.hold_ms,
                probe.gap_ms,
                &mut samples,
            ),
        ),
    };
    if !sent {
        eprintln!(
            "hanyeong: 키 이벤트를 보내거나 받을 수 없습니다. 접근성 권한이 필요합니다(지금: {})",
            if trust::trusted() { "있음" } else { "없음" }
        );
        return false;
    }
    let Some(summary) = stats::summarize(&samples) else {
        eprintln!(
            "hanyeong: {} 번 모두 시간 안에 결과가 없었습니다",
            samples.len()
        );
        return false;
    };
    println!(
        "probe {label}: {}번 가운데 {}번 측정, {}번 놓침, hold {}ms, gap {}ms",
        samples.len(),
        summary.count,
        summary.missed,
        probe.hold_ms,
        probe.gap_ms
    );
    println!(
        "  최소 {} ms, 중앙값 {} ms, p90 {} ms, 최대 {} ms",
        ms(summary.min),
        ms(summary.median),
        ms(summary.p90),
        ms(summary.max)
    );
    let mut line = String::from("  값(ms):");
    for sample in &samples {
        line.push(' ');
        match sample {
            Some(ns) => line.push_str(&ms(*ns)),
            None => line.push('-'),
        }
    }
    println!("{line}");
    true
}

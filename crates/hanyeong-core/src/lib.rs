//! 한영 전환의 판단 로직.
//!
//! 여기에는 시스템 호출이 없다. 키 이벤트 하나를 받아 무엇을 할지 정하고(`press`),
//! 키보드의 키 매핑 목록을 어떻게 고칠지 정하고(`mapping`), 시스템 단축키가 우리 것인지
//! 판단한다(`shortcut`). 설치할 파일의 내용도 여기서 만든다(`install`). macOS 를 부르는 일은 `hanyeong-mac` 이 한다.
#![forbid(unsafe_code)]

pub mod args;
pub mod install;
pub mod keys;
pub mod mapping;
pub mod press;
pub mod recheck;
pub mod shortcut;
pub mod stats;

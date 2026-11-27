//! macOS 를 부르는 일.
//!
//! 무엇을 할지는 `hanyeong-core` 가 정하고, 여기서는 그대로 실행만 한다. 이 저장소에서 unsafe 가
//! 허용되는 곳은 이 크레이트뿐이고, 블록마다 `// SAFETY:` 가 붙는다.
#![cfg(target_os = "macos")]
#![deny(clippy::undocumented_unsafe_blocks)]

mod cf;
mod entry;
mod ffi;
pub mod hid;
pub mod hotkey;
mod objc;
pub mod probe;
pub mod secure;
pub mod session;
pub mod tap;
pub mod tray;
pub mod trust;

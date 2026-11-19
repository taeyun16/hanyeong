# 기여하기

버그 알림, 풀 리퀘스트 모두 반갑습니다. 이 도구는 키보드의 키 매핑과 시스템 단축키를 바꾸기 때문에,
잘못되면 쓰는 사람의 키보드가 이상해집니다. 그래서 아래를 지켜 주세요.

## 준비

- macOS, Xcode 명령행 도구(`xcode-select --install`)
- [rustup](https://rustup.rs). 컴파일러 버전은 `rust-toolchain.toml` 에 적혀 있어 저절로 맞춰집니다.

## 바꾸기 전에

- 큰 변경이나 기능 추가는 먼저 이슈로 이야기해 주세요. hanyeong 은 한영 키와 누를 때 전환만 하는 작은
  도구로 남으려고 합니다.
- 외부 크레이트는 0 개입니다. 더해야 한다면 이유와 빌드 시간·실행 파일 크기의 변화를 함께 적어 주세요.

## 검사

풀 리퀘스트를 열면 CI 가 아래를 돌립니다. 올리기 전에 직접 돌려 주세요.

```sh
cargo fmt --all --check
```

```sh
cargo clippy --locked --workspace --all-targets -- -D warnings
```

```sh
cargo test --locked --workspace
```

CI 가 덮지 못하는 것이 있습니다. `crates/hanyeong-mac` 이나 `crates/hanyeong` 을 바꿨다면 실제 Mac 에서
띄워 보고 `hanyeong status` 출력을 풀 리퀘스트에 붙여 주세요. 이벤트 탭이나 전환 경로를 바꿨다면
`hanyeong probe` 로 전후를 재 주세요(입력 소스가 실제로 바뀝니다).

```sh
cargo build --release --locked -p hanyeong && ./target/release/hanyeong install
```

## 코드 규칙

- **판단과 시스템 호출을 가릅니다.** 무엇을 할지는 `hanyeong-core` 가 정하고 테스트합니다.
  `hanyeong-mac` 은 그대로 실행만 합니다.
- **`unsafe` 는 `hanyeong-mac` 에서만** 쓰고, 블록마다 `// SAFETY:` 로 이유를 적습니다. 나머지 두
  크레이트는 `#![forbid(unsafe_code)]` 입니다.
- **이벤트 탭 콜백은 모든 키 입력이 지나갑니다.** 그 안에서 할당하지 않습니다.
- `.unwrap()`, `panic!`, 넘칠 수 있는 산술은 쓰지 않습니다. `checked_`/`saturating_` 을 씁니다.
- 버그를 고칠 때는 먼저 그 버그를 재현해 실패하는 테스트를 넣습니다.
- 바꾸기 전의 시스템 값은 남겨 두고, 끝날 때 우리가 쓴 것만 되돌립니다.

자세한 규칙은 [`CLAUDE.md`](CLAUDE.md) 에 있습니다.

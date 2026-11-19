# hanyeong 작업 규칙

이 저장소를 바꾸는 모든 에이전트가 따른다. 변경은 1절의 검사를 전부 돌리고 그 결과를
숫자와 함께 보고했을 때 끝난 것이다. 돌리지 못한 검사는 못 돌렸다고 적는다. 통과로 적지 않는다.

## 1. 코드 변경마다 거치는 검사

`crates/` 를 바꾸면 아래를 모두 통과해야 한다.

1. **포맷과 린트**: `cargo fmt --all --check`, `cargo clippy --locked --workspace --all-targets -- -D warnings`.
   `hanyeong-core` 와 `hanyeong` 은 `#![forbid(unsafe_code)]`, `hanyeong-mac` 은
   `clippy::undocumented_unsafe_blocks` 를 거부한다. 통과시키려고 이 줄들을 지우지 않는다.
2. **테스트**: `cargo test --locked --workspace`.
3. **빌드 비용**: 의존성·기능 플래그·빌드 프로필을 바꾸면 깨끗한 상태의 빌드 시간, `target/` 크기,
   릴리스 바이너리 크기를 전후로 적는다. 지금 외부 의존성은 0 개다.
4. **실기기 확인**: `crates/hanyeong-mac`·`crates/hanyeong` 를 바꿨으면 실제로 띄워 `hanyeong status`
   와 `hanyeong probe` 의 출력을 보고에 붙인다. 이 코드는 테스트가 덮지 못한다. 이벤트 탭이나 전환
   경로를 바꿨으면 `probe` 로 전후를 재서 `docs/perf-progress.md` 에 남긴다.

## 2. 테스트 정책

- **공개된 표면으로 시험한다.** 판단 로직은 `hanyeong-core` 의 공개 함수로 시험한다: 키 이벤트를
  넣어 나오는 판단, 매핑 목록을 넣어 나오는 새 목록.
- **실패는 회귀 테스트가 된다.** 버그를 찾으면 먼저 그것을 재현해 실패하는 테스트를 넣고 고친다.
- 기대값은 실제 기기에서 본 값이나 원본(gksdud)의 동작에서 가져온다. 후보가 낸 출력으로 기대값을
  다시 만들어 통과시키지 않는다.
- 테스트는 결정적이어야 한다. 벽시계 시간, 실제 키보드, 실제 시스템 설정에 기대지 않는다.
  시간은 인자로 넣는다(`Key::now_ns`).
- `#[ignore]` 는 찾았지만 아직 고치지 않은 버그의 재현 테스트에만 쓴다. 고치면 뗀다.
  지금은 하나도 없다.

## 3. Rust 코드 제약

- **unsafe**: `crates/hanyeong-mac` 에서만 쓴다. 블록마다 `// SAFETY:` 를 적는다(clippy 가 거부한다).
  다른 크레이트에서 필요해지면 그 호출을 `hanyeong-mac` 의 안전한 함수로 옮긴다.
- **시스템 호출과 판단을 가른다.** 무엇을 할지는 `hanyeong-core` 가 정하고, `hanyeong-mac` 은 그대로
  실행만 한다. 분기가 `hanyeong-mac` 에 생기면 시험할 수 없게 된다.
- **이벤트 탭 콜백은 모든 키 입력이 지나간다.** 그 안에서 할당하지 않고, 대상 키가 아니면 키 코드
  하나만 읽고 돌려준다.
- **복사·패닉·산술**: `.clone()`·`.to_string()`, `.unwrap()`·`panic!`·`a[i]`, 넘칠 수 있는 산술을
  쓰지 않는다. 린트가 막지 않으니 리뷰에서 본다. `saturating_`/`wrapping_`/`checked_` 를 쓴다.
- **상한**: 새 컬렉션에는 상한 상수를 둔다(`MAX_MAPPINGS`, `MAX_KEYBOARDS`).
- **의존성**: 외부 크레이트 0 개로 시작했다. 더하려면 이유와 빌드 시간·바이너리 크기 영향을 적는다.
- **린트 끄기**: `#[expect(lint, reason = "…")]` 를 가장 작은 항목에만 붙인다.
- **macOS 전용**이다. `hanyeong-core` 는 어느 OS 에서나 빌드·시험된다.

## 4. 사용자의 시스템을 바꾸는 코드

이 도구는 키보드의 키 매핑과 시스템 단축키를 바꾼다. 잘못되면 키보드가 이상해진다.

- 바꾸기 전에 원래 값을 남기고, 끝날 때 우리가 쓴 것만 되돌린다. 남이 그 뒤에 바꾼 값은 건드리지
  않는다.
- 같은 수법의 다른 도구(gksdud)와 함께 돌면 서로의 키 이벤트를 주고받는다. 시작할 때 확인해
  거부하고, 그래도 생기면 폭주 차단(`press.rs`)이 끊는다. 이 둘을 약하게 만들지 않는다.
- 실기기 측정(`probe`)은 사용자의 입력 소스를 실제로 바꾼다. 짝수 번만 누르고, 돌리기 전에 알린다.

## 5. 근거와 함께 보고한다

"했다", "고쳤다", "통과한다"는 결과가 아니다.

- **명령과 출력.** 정확한 명령과 해당 출력: 종료 코드, 통과·실패 건수, 오류 문구.
- **구체적인 숫자.** 단위, 실행 횟수, 기준값과 후보값을 나란히.
- **무엇으로 쟀는지.** 커밋, 툴체인, 빌드 프로필, 기기와 부하(`uptime`).
- **확인한 것과 못 한 것을 가른다.** 돌려서 본 것과, 쓰기만 했거나 추론한 것을 구분한다.
- 측정 기록은 `docs/perf-progress.md` 에 커밋, 명령, 기기 상태와 함께 남긴다.

## 6. 도구

| 필요한 것 | 도구 | 절차 |
|---|---|---|
| 실기기의 전환 지연·키 통과 지연 | `hanyeong probe`, `tools/measure_live.sh` | `docs/perf-progress.md` |
| gksdud·도구 없음·hanyeong 을 번갈아 비교 | `tools/compare_live.sh` | `docs/perf-progress.md`. 합성 키로는 전환 지연을 잴 수 없다 |

## 7. CI 와 배포

- `.github/workflows/ci.yml`: 풀 리퀘스트와 `main` 마다 1절의 1·2 를 macOS 에서, `Cargo.toml` 의
  `rust-version`(Homebrew 가 쓰는 Rust 가 이보다 낮으면 설치가 깨진다)으로 빌드·테스트를 macOS 에서, `hanyeong-core` 의
  clippy·테스트를 Linux 에서, `install.sh`·`tools/*.sh` 를 shellcheck 로 돌린다. 실기기 확인은 CI 가
  대신하지 못한다.
- `.github/workflows/release.yml`: `v*` 태그(= `Cargo.toml` 의 버전)를 올리면 universal 실행 파일을
  Releases 에 올리고, `HOMEBREW_TAP_DEPLOY_KEY`(tap 에 쓰기로 등록한 배포 키)가 있으면 `taeyun16/homebrew-tap` 의 formula 를 바꾼 뒤
  `brew install` 로 확인한다. formula 의 원본은 `packaging/homebrew/hanyeong.rb`.
- 설치는 어느 경로든 실행 파일 하나를 받고 `hanyeong install` 이 번들과 LaunchAgent 를 만든다
  (`crates/hanyeong/src/install.rs`, 내용은 `hanyeong_core::install`). 설치 경로를 더할 때도 이 명령을 쓴다.
- 워크플로의 액션은 커밋 해시로 고정한다. 올리는 것은 dependabot 이 한다.

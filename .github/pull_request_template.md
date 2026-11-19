## 무엇을, 왜

<!-- 바꾼 것과 그 이유. 관련 이슈가 있으면 적어 주세요. -->

## 확인한 것

- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --locked --workspace --all-targets -- -D warnings`
- [ ] `cargo test --locked --workspace` (통과 건수: )
- [ ] 버그를 고쳤다면, 고치기 전에 실패하는 테스트를 먼저 넣었다

`crates/hanyeong-mac` 이나 `crates/hanyeong` 을 바꿨다면 (CI 가 덮지 못하는 부분입니다)

- [ ] 실제 Mac 에서 띄워 본 `hanyeong status` 출력을 아래에 붙였다
- [ ] 이벤트 탭이나 전환 경로를 바꿨다면 `hanyeong probe` 로 전후를 쟀다

```
(hanyeong status 출력)
```

## 확인하지 못한 것

<!-- 돌려 보지 못한 경우, 추측으로 남은 부분. 없으면 "없음". -->

#!/bin/sh
# hanyeong 을 GitHub Releases 에서 받아 설치한다.
#
#   curl -fsSL https://raw.githubusercontent.com/taeyun16/hanyeong/main/install.sh | sh
#
# 실행 파일을 ~/.local/bin 에 두고 `hanyeong install` 로 앱 번들과 로그인 항목을 만든다.
#   HANYEONG_VERSION=v0.1.0   이 버전을 받는다(기본: 가장 최근 릴리스)
#   HANYEONG_BIN_DIR=경로     실행 파일을 둘 곳(기본: ~/.local/bin)
#   HANYEONG_NO_SETUP=1       실행 파일만 두고 `hanyeong install` 은 돌리지 않는다
#
# 내려받은 묶음은 함께 올린 SHA-256 으로 확인한다. 관리자 권한은 쓰지 않는다.
set -eu

main() {
    repo=taeyun16/hanyeong
    asset=hanyeong-universal-apple-darwin.tar.gz
    version=${HANYEONG_VERSION:-latest}
    bin_dir=${HANYEONG_BIN_DIR:-$HOME/.local/bin}

    if [ "$(uname -s)" != Darwin ]; then
        echo "hanyeong 은 macOS 전용입니다." >&2
        exit 1
    fi
    if [ "$version" = latest ]; then
        base="https://github.com/$repo/releases/latest/download"
    else
        base="https://github.com/$repo/releases/download/$version"
    fi

    tmp=$(mktemp -d)
    trap 'rm -rf "$tmp"' EXIT INT TERM
    echo "받는 중: $base/$asset"
    curl -fsSL --proto '=https' --tlsv1.2 -o "$tmp/$asset" "$base/$asset"
    curl -fsSL --proto '=https' --tlsv1.2 -o "$tmp/$asset.sha256" "$base/$asset.sha256"
    if ! (cd "$tmp" && shasum -a 256 -c "$asset.sha256" >/dev/null); then
        echo "체크섬이 맞지 않습니다. 설치하지 않습니다." >&2
        exit 1
    fi
    tar -xzf "$tmp/$asset" -C "$tmp"

    mkdir -p "$bin_dir"
    install -m 755 "$tmp/hanyeong" "$bin_dir/hanyeong"
    echo "실행 파일: $bin_dir/hanyeong ($("$bin_dir/hanyeong" version))"

    if [ "${HANYEONG_NO_SETUP:-}" != 1 ]; then
        "$bin_dir/hanyeong" install
    fi
    case ":$PATH:" in
        *":$bin_dir:"*) ;;
        *) echo "참고: $bin_dir 가 PATH 에 없습니다. 셸 설정에 export PATH=\"$bin_dir:\$PATH\" 를 더하세요." ;;
    esac
}

main "$@"

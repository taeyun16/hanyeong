# 릴리스 워크플로(.github/workflows/release.yml)가 @VERSION@ 과 @SHA256@ 을 채워
# taeyun16/homebrew-tap 의 Formula/hanyeong.rb 로 올린다. 이 파일을 직접 tap 에 복사하지 않는다.
class Hanyeong < Formula
  desc "Korean/English input switch for macOS that fires on key press"
  homepage "https://github.com/taeyun16/hanyeong"
  url "https://github.com/taeyun16/hanyeong/archive/refs/tags/v@VERSION@.tar.gz"
  sha256 "@SHA256@"
  license "Apache-2.0"
  head "https://github.com/taeyun16/hanyeong.git", branch: "main"

  depends_on "rust" => :build
  depends_on :macos

  def install
    system "cargo", "install", *std_cargo_args(path: "crates/hanyeong")
  end

  def caveats
    <<~EOS
      한영 키를 켜려면 한 번 실행하세요:
        hanyeong install
      ~/Applications/hanyeong.app 을 만들고 로그인할 때 켜지도록 등록합니다.
      처음에는 손쉬운 사용 목록에서 hanyeong 을 켜야 합니다.

      brew upgrade 뒤에는 hanyeong install 을 다시 실행하세요.
      brew uninstall 전에는 hanyeong uninstall 을 먼저 실행하세요.
    EOS
  end

  test do
    assert_match(/^hanyeong \d+\.\d+\.\d+/, shell_output("#{bin}/hanyeong version"))
  end
end

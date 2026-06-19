# typed: true
# frozen_string_literal: true

# Homebrew formula for seekit
# Usage:
#   brew install noisystreet/tap/seekit
#
# For the tap to work, this formula must be pushed to:
#   https://github.com/noisystreet/homebrew-tap
# in the path: Formula/seekit.rb

class Seekit < Formula
  desc "CLI web search tool using DuckDuckGo and SearXNG"
  homepage "https://github.com/noisystreet/seekit"
  license "MIT"
  version "0.1.2"

  if OS.mac? && Hardware::CPU.arm?
    url "https://github.com/noisystreet/seekit/releases/latest/download/seekit-aarch64-apple-darwin.tar.gz"
    sha256 "0000000000000000000000000000000000000000000000000000000000000000"
  elsif OS.mac? && Hardware::CPU.intel?
    url "https://github.com/noisystreet/seekit/releases/latest/download/seekit-x86_64-apple-darwin.tar.gz"
    sha256 "0000000000000000000000000000000000000000000000000000000000000000"
  elsif OS.linux? && Hardware::CPU.arm?
    url "https://github.com/noisystreet/seekit/releases/latest/download/seekit-aarch64-unknown-linux-gnu.tar.gz"
    sha256 "0000000000000000000000000000000000000000000000000000000000000000"
  elsif OS.linux? && Hardware::CPU.intel?
    url "https://github.com/noisystreet/seekit/releases/latest/download/seekit-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "0000000000000000000000000000000000000000000000000000000000000000"
  end

  def install
    bin.install "seekit"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/seekit --version")
  end
end

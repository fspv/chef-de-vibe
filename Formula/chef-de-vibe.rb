class ChefDeVibe < Formula
  desc "Chef de Vibe - A Rust application with embedded React frontend"
  homepage "https://github.com/fspv/chef-de-vibe"
  license "MIT"
  head "https://github.com/fspv/chef-de-vibe.git", branch: "master"

  depends_on "rust" => :build
  depends_on "node" => :build
  depends_on "pkg-config" => :build
  depends_on "openssl@3"

  def install
    # Build frontend first
    cd "frontend" do
      system "npm", "ci", "--legacy-peer-deps"
      system "npm", "run", "build"
    end

    # Build Rust application with embedded frontend
    system "cargo", "install", "--locked", "--root", prefix, "--path", "."
  end

  test do
    # Start the server in the background
    port = free_port
    pid = fork do
      ENV["PORT"] = port.to_s
      exec "#{bin}/chef-de-vibe"
    end
    sleep 5

    # Test that the server is running
    assert_match "200", shell_output("curl -I -s -o /dev/null -w '%{http_code}' http://localhost:#{port}")
  ensure
    Process.kill("TERM", pid) if pid
    Process.wait(pid) if pid
  end

  def free_port
    server = TCPServer.new("127.0.0.1", 0)
    port = server.addr[1]
    server.close
    port
  end
end
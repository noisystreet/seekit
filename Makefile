.PHONY: all build test lint format clean setup check deploy deploy-up deploy-down deploy-logs deploy-status deploy-restart

# 默认目标
all: build test lint

# 构建
build:
	cargo build

release:
	cargo build --release

# 测试
test:
	cargo test

test-all:
	cargo test -- --include-ignored

coverage:
	cargo tarpaulin --out Html --output-dir coverage && echo "Coverage report: file://$(PWD)/coverage/tarpaulin-report.html"

# Lint
lint:
	cargo clippy -- -D warnings

lint-deny:
	cargo clippy -- -D warnings

# 格式化
format:
	cargo fmt

format-check:
	cargo fmt --check

# 安全检查
audit:
	cargo audit

# 清理
clean:
	cargo clean
	rm -rf coverage/

# 开发环境准备
setup:
	rustup component add clippy rustfmt
	@echo "Development environment ready."
	@echo "Run 'make precommit-install' to install git hooks."

# Pre-commit
precommit-install:
	@which pre-commit || pip install pre-commit
	pre-commit install
	pre-commit install --hook-type commit-msg
	@echo "pre-commit hooks installed."

precommit-run:
	pre-commit run --all-files

# 运行
run:
	cargo run -- $(ARGS)

# 安装
install: release
	@echo "Installing seekit to /usr/local/bin..."
	cp target/release/seekit /usr/local/bin/
	@echo "✓ Installed. Run: seekit \"your query\""

install-home:
	@echo "Installing seekit to ~/.cargo/bin..."
	cargo install --path .
	@echo "✓ Installed. Run: seekit \"your query\""

# SearXNG 部署管理
deploy-up:
	@echo "Starting SearXNG..."
	docker compose -f deploy/docker-compose.yml up -d
	@echo "SearXNG started at http://localhost:8080"
	@echo "Test with: curl --noproxy '*' http://localhost:8080/search?q=test&format=json"

deploy-down:
	@echo "Stopping SearXNG..."
	docker compose -f deploy/docker-compose.yml down

deploy-logs:
	docker compose -f deploy/docker-compose.yml logs -f

deploy-status:
	@docker compose -f deploy/docker-compose.yml ps

deploy-restart:
	@echo "Restarting SearXNG..."
	docker compose -f deploy/docker-compose.yml restart
	@sleep 3
	@echo "SearXNG restarted."
	@curl --noproxy '*' -s -o /dev/null -w "Status: %{http_code}\n" http://localhost:8080/

deploy: deploy-up

# 帮助
help:
	@echo "Usage: make <target>"
	@echo ""
	@echo "Build & Test:"
	@echo "  build              Build the project (debug)"
	@echo "  release            Build the project (release)"
	@echo "  test               Run tests"
	@echo "  test-all           Run all tests (including ignored)"
	@echo "  lint               Run clippy"
	@echo "  format             Format code"
	@echo "  format-check       Check formatting"
	@echo ""
	@echo "Install:"
	@echo "  install            Build release and copy to /usr/local/bin"
	@echo "  install-home       Install to ~/.cargo/bin (via cargo install)"
	@echo ""
	@echo "SearXNG Deployment:"
	@echo "  deploy             Start SearXNG (same as deploy-up)"
	@echo "  deploy-up          Start SearXNG container"
	@echo "  deploy-down        Stop SearXNG container"
	@echo "  deploy-restart     Restart SearXNG container"
	@echo "  deploy-logs        Follow SearXNG logs"
	@echo "  deploy-status      Check SearXNG container status"
	@echo ""
	@echo "Maintenance:"
	@echo "  clean              Clean build artifacts"
	@echo "  audit              Run cargo audit"
	@echo "  setup              Install dev dependencies"
	@echo "  run ARGS=\"...\"    Run with arguments"
	@echo "  precommit-install  Install git hooks"
	@echo "  precommit-run      Run pre-commit on all files"

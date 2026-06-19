# AGENTS.md

Project guide for AI Agents. Please read this before modifying any code.

## Project Identity

- **Project**: seekit — Rust CLI Web search tool
- **Tech stack**: Rust (stable), clap, reqwest, scraper, tokio, serde
- **Directory structure**:

```
src/
├── main.rs            # CLI entry point
├── lib.rs             # Public API
├── cli.rs             # clap argument parsing
├── engine/
│   ├── mod.rs         # EngineType enum
│   ├── trait.rs       # SearchEngine trait
│   ├── duckduckgo.rs  # DuckDuckGo implementation
│   ├── searxng.rs     # SearXNG implementation
│   └── fusion.rs      # Multi-engine fusion (auto mode)
├── config.rs          # TOML configuration
├── cache.rs           # Disk cache
├── fetcher.rs         # Page content fetcher (HTML → Markdown)
├── output.rs          # Terminal/JSON/Raw output
└── error.rs           # Error types

tests/
└── integration_test.rs   # Integration tests

docs/
├── MANUAL.md          # English user manual
├── MANUAL.zh.md       # Chinese user manual
└── adr/
    └── DESIGN.md      # Architecture design document (Chinese)

deploy/
├── docker-compose.yml # SearXNG deployment
└── searxng/
    ├── settings.yml   # SearXNG config
    └── limiter.toml   # Rate limiter config

homebrew/
└── Formula/
    └── seekit.rb      # Homebrew formula

install.sh               # One-command installer: curl | sh

## Hard Constraints

1. **Dependency direction**: `engine/` → independent, `cli.rs` → independent, `lib.rs` orchestrates all. Reverse dependencies are prohibited.
2. **Third-party libraries**: No hard restrictions, but any new dependency must be justified in the PR description.
3. **Documentation changes**:
   - `docs/adr/DESIGN.md` — requires manual approval before modification
   - `AGENTS.md` — can be modified directly
4. **Security red lines**:
   - Never include keys, tokens, certificates or other sensitive information in code or commits
   - Never bypass permission checks
   - Never use `unwrap()` / `expect()` in library code (allowed only in main.rs and tests)
5. **Test requirements**: New features must include tests.
6. **Documentation sync**: When updating documentation, Chinese and English versions must be kept in sync. If you modify `docs/MANUAL.md`, update `docs/MANUAL.zh.md` accordingly (and vice versa).
7. **CHANGELOG language**: `CHANGELOG.md` must be written in English only.
8. **Project structure sync**: When adding or moving files under `src/` or `docs/`, update the directory structure in this file and `docs/adr/DESIGN.md` accordingly to keep them in sync.
9. **Commit rules**: Never use `--no-verify` or `-n` when committing. All pre-commit hooks must run. If a hook fails, fix the issue rather than bypassing it.
10. **GitHub Flow**: All development must follow the GitHub Flow process:
    - **Always start by pulling latest main**: `git checkout main && git pull`
    - Create a new branch from `main` for each feature/fix (`git checkout -b feat/xxx` or `fix/xxx`)
    - Commit changes on the branch (never commit directly to main)
    - Push the branch to origin
    - Create a Pull Request on GitHub (title = commit message, body = summary of changes)
    - Wait for CI to pass before merging
    - Squash merge into main on GitHub
    - Delete the remote branch after merge
    - Sync locally: `git checkout main && git pull`

## Verification

After making changes, the Agent must run:

```bash
cargo fmt --check    # Formatting
cargo clippy         # Lint
cargo test           # Tests
cargo build          # Build
```

## Conventions

- Commit message format: `<type>(<scope>): <subject>` (English first line, Chinese body allowed)
- Code comments in Chinese
- New modules must be registered in `lib.rs`

## References

- [CONTRIBUTING.md](CONTRIBUTING.md)
- [SECURITY.md](SECURITY.md)

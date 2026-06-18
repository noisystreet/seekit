# Contributing to seekit

Thank you for your interest in seekit! We welcome all kinds of contributions.

## Ways to Contribute

- **Report a Bug**: Submit via GitHub Issues using the Bug Report template
- **Feature Request**: Submit via GitHub Issues using the Feature Request template
- **Code Contribution**: Fork the repository → Create a branch → Submit a PR

## Development Workflow

1. Make sure Rust is installed (see version in `rust-toolchain.toml`)
2. `make setup` — Install development dependencies (pre-commit hooks, etc.)
3. `make build` — Build the project
4. `make test` — Run tests
5. `make lint` — Run lint checks
6. `make format` — Format code

## Submitting a PR

1. Ensure all CI checks pass
2. New features must include tests
3. Update relevant documentation (`docs/MANUAL.md` and `docs/MANUAL.zh.md` must be kept in sync)
4. Describe the changes and testing strategy in the PR description

## Documentation Sync Rules

The project maintains bilingual documentation (Chinese and English). When making changes, both versions must be updated together:

| English | Chinese | Description |
|---------|---------|-------------|
| `MANUAL.md` | `MANUAL.zh.md` | User manual, must be synced |
| `README.md` | `README.zh.md` | Project intro, must be synced |
| `AGENTS.md` | — | Agent guide, English only |

## Security Issues

Please **do not** report security vulnerabilities in public Issues. Report them via the method described in `SECURITY.md`.

## License

This project is licensed under the MIT License. By contributing, you agree that your contributions will be licensed under the same license.

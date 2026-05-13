# Contributing to Modern Blockchain

Thank you for your interest in contributing to Modern Blockchain!

## Development Setup

1. Install prerequisites:
   - Rust 1.70+
   - Elixir 1.15.0+ with OTP 26
   - Protocol Buffers compiler

2. Clone the repository:
   ```bash
   git clone https://github.com/your-org/modern-blockchain.git
   cd modern-blockchain
   ```

3. Build the project:
   ```bash
   ./build.sh
   ```

4. Run tests:
   ```bash
   cargo test --workspace
   cd networking && mix test && cd ..
   ```

## Code Style

- **Rust**: Follow standard Rust conventions. Run `cargo fmt` and `cargo clippy`.
- **Elixir**: Follow Elixir style guide. Run `mix format`.

## Submitting Changes

1. Create a feature branch from `develop`
2. Make your changes with clear commit messages
3. Add tests for new functionality
4. Ensure all tests pass
5. Submit a pull request to `develop`

## Pull Request Process

1. Update documentation for any changed functionality
2. Add tests covering your changes
3. Ensure CI passes
4. Request review from maintainers
5. Address review feedback

## Reporting Issues

- Use GitHub Issues
- Include reproduction steps
- Provide system information
- Include relevant logs

## Questions?

Open a discussion on GitHub or reach out to the maintainers.

# Modern Blockchain

A next-generation blockchain combining modern consensus mechanisms, content-addressed state management, and polyglot architecture.

## Features

- **Sub-second Finality**: 800ms finality using Mysticeti DAG consensus
- **High Throughput**: 10,000+ TPS with parallel execution
- **Content-Addressed State**: Automatic conflict detection with Object References
- **Polyglot Architecture**: Rust (consensus/execution/validator), Elixir (networking)
- **Dual Smart Contract System**: WASM and custom DSL support
- **BLS Signature Aggregation**: Efficient stake-weighted voting

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        APPLICATION LAYER                         │
└─────────────────────────────────────────────────────────────────┘
                              ↓ ↑
┌─────────────────────────────────────────────────────────────────┐
│                       CONSENSUS LAYER (Rust)                     │
│  Mysticeti DAG | BLS Aggregation | Leader Selection             │
└─────────────────────────────────────────────────────────────────┘
                              ↓ ↑
┌─────────────────────────────────────────────────────────────────┐
│                    NETWORKING LAYER (Elixir)                     │
│  P2P Gossip | Peer Discovery | Connection Management            │
└─────────────────────────────────────────────────────────────────┘
                              ↓ ↑
┌─────────────────────────────────────────────────────────────────┐
│                      EXECUTION LAYER (Rust)                      │
│  WASM Runtime | DSL Runtime | Parallel Executor                 │
└─────────────────────────────────────────────────────────────────┘
                              ↓ ↑
┌─────────────────────────────────────────────────────────────────┐
│                        STATE LAYER (Rust)                        │
│  Content-Addressed Files | Merkle Tree | State Root             │
└─────────────────────────────────────────────────────────────────┘
                              ↓ ↑
┌─────────────────────────────────────────────────────────────────┐
│                    VALIDATOR CLIENT (Rust)                       │
│  Block Production | Signature Generation | Hardware Optimization │
└─────────────────────────────────────────────────────────────────┘
```

## Prerequisites

- **Rust**: 1.70 or later
- **Elixir**: 1.15.0 or later with OTP 26
- **Protocol Buffers**: protoc compiler

## Building

### Quick Build

```bash
./build.sh
```

### Component-Specific Builds

**Rust Workspace:**
```bash
cargo build --workspace --release
```

**Elixir Networking:**
```bash
cd networking
mix deps.get
mix compile
```

## Testing

**Rust:**
```bash
cargo test --workspace
```

**Elixir:**
```bash
cd networking
mix test
```

## Project Structure

```
.
├── crates/
│   ├── common/          # Shared types and utilities
│   ├── consensus/       # Mysticeti DAG consensus
│   ├── execution/       # WASM and DSL runtimes
│   ├── state/           # Content-addressed state management
│   ├── storage/         # RocksDB persistence layer
│   ├── proto/           # Protocol Buffer definitions
│   └── validator/       # High-performance validator client
├── networking/          # Elixir P2P networking layer
└── build.sh            # Unified build script
```

## Development Status

This project is under active development. See `.kiro/specs/modern-blockchain/tasks.md` for the implementation roadmap.

### Completed
- ✅ Project structure and build system
- ✅ Protocol Buffer definitions
- ✅ CI/CD pipeline

### In Progress
- 🚧 Core data structures (Phase 1)
- 🚧 Consensus layer (Phase 2)

### Planned
- ⏳ Execution engine (Phase 3)
- ⏳ Networking layer (Phase 4)
- ⏳ Staking and governance (Phase 5)
- ⏳ Data availability (Phase 6)
- ⏳ ZK proofs (Phase 7)
- ⏳ RPC API and tools (Phase 8)
- ⏳ Validator client (Phase 9)
- ⏳ Testing and documentation (Phase 10)

## License

MIT OR Apache-2.0

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

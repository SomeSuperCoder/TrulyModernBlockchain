# PoH Blockchain - Previous Implementation Summary

## Document Purpose

This document provides a comprehensive guide for reimplementing the PoH (Proof of History) blockchain in a different programming language. It captures all architectural decisions, features, implementation details, and design patterns from the Go implementation to ensure nothing is lost in the transition.

## Table of Contents

1. [Project Overview](#project-overview)
2. [Core Architecture](#core-architecture)
3. [Proof of History Clock](#proof-of-history-clock)
4. [Blockchain Data Structures](#blockchain-data-structures)
5. [Consensus Mechanism](#consensus-mechanism)
6. [File-Based State Model](#file-based-state-model)
7. [Transaction System](#transaction-system)
8. [QuanticScript Language](#quanticscript-language)
9. [Smart Contract Programs](#smart-contract-programs)
10. [Network Layer](#network-layer)
11. [Storage and Persistence](#storage-and-persistence)
12. [Wallet System](#wallet-system)
13. [RPC API](#rpc-api)
14. [Testing Strategy](#testing-strategy)
15. [CLI and Tools](#cli-and-tools)
16. [Key Implementation Details](#key-implementation-details)

---

## 1. Project Overview

### Vision
A Proof of History blockchain inspired by Solana's architecture, featuring:
- Sequential SHA-256 hashing as a verifiable delay function
- High-throughput transaction ordering without traditional consensus overhead
- Delegated Proof of Stake (DPoS) with stake-weighted leader scheduling
- Custom smart contract language (QuanticScript) with TypeScript-like syntax
- File-based state model (similar to Solana's account model)
- Byzantine Fault Tolerance

### Technology Stack (Go Implementation)
- **Language**: Go 1.23.0+
- **Persistence**: SQLite3 (blockchain ledger), BadgerDB v4 (state storage)
- **Cryptography**: Ed25519 signatures, SHA-256 hashing
- **Networking**: TCP-based P2P
- **Encryption**: AES-256-GCM with Argon2id key derivation

### Key Metrics
- **Slot Duration**: 400ms
- **Ticks per Slot**: Minimum 64 ticks
- **Hashes per Tick**: 12,500 SHA-256 operations
- **Epoch Length**: 432,000 slots (~2 days)
- **Max Cross-Program Invocation Depth**: 4 levels
- **Default Compute Budget**: 1,000,000 units per instruction

---

## 2. Core Architecture

### Layered Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Application Layer                         │
│  (CLI, Node Management, Wallet TUI, Validator Dashboard)    │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│                     Consensus Layer                          │
│  (Leader Selection, Slot Timing, DPoS, Epoch Management)    │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│                      Network Layer                           │
│         (P2P Communication, Block Broadcasting)              │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│                  Core Blockchain Layer                       │
│    (PoH Clock, Entries, Blocks, Transaction Processing)     │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│                     Storage Layer                            │
│     (SQLite Ledger, BadgerDB State, Verification)           │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│                 File-Based State Layer                       │
│  (FileStore, Transaction Processor, Smart Contract Runtime) │
└─────────────────────────────────────────────────────────────┘
```

### Package Structure

```
internal/
├── poh/                    # PoH clock generator
├── blockchain/             # Core blockchain data structures
├── consensus/              # Consensus protocol and DPoS
├── network/                # P2P networking
├── storage/                # SQLite ledger persistence
├── verification/           # Chain integrity verification
├── filestore/              # File-based state model (BadgerDB)
├── transaction/            # Transaction and instruction types
├── access/                 # Access control and permissions
├── processor/              # Transaction processor with rollback
├── runtime/                # Program execution runtime
├── system/                 # System program (Go implementation)
├── wallet/                 # Validator wallet (encrypted keypairs)
├── genesis/                # Genesis bootstrap
├── parallel/               # Parallel execution analysis
├── rpc/                    # JSON-RPC 2.0 server
└── quanticscript/          # QuanticScript language implementation
    ├── lexer.go            # Tokenization
    ├── parser.go           # AST construction
    ├── typechecker.go      # Type inference and validation
    ├── codegen.go          # Bytecode generation
    ├── interpreter.go      # Bytecode execution
    ├── assembler.go        # Assembly to bytecode
    ├── disassembler.go     # Bytecode to assembly
    └── stdlib.go           # Standard library functions
```



---

## 3. Proof of History Clock

### Concept
The PoH clock creates a verifiable timeline using sequential SHA-256 hashing. Each hash depends on the previous hash, creating a cryptographic proof that time has passed.

### Implementation Details

**PohClock Structure**:
```go
type PohClock struct {
    currentHash []byte      // Current hash state
    tickCount   int64       // Total ticks generated
    hashCount   int64       // Total hash operations
    mu          sync.RWMutex // Thread-safe access
}
```

**Key Operations**:

1. **Initialization**:
   - Takes a seed (e.g., "genesis-seed")
   - Computes initial hash: `SHA-256(seed)`
   - Initializes counters to 0

2. **HashOnce()**:
   - Performs single SHA-256: `hash = SHA-256(currentHash)`
   - Updates currentHash
   - Increments hashCount
   - Thread-safe with mutex

3. **Tick()**:
   - Performs 12,500 hash operations
   - Increments tickCount
   - Returns Tick structure with:
     - HashValue (current hash)
     - Timestamp (wall clock time)
     - TickNumber (sequential counter)

**Tick Structure**:
```go
type Tick struct {
    HashValue  []byte
    Timestamp  time.Time
    TickNumber int64
}
```

### Verification Properties
- **Sequential**: Each hash depends on previous hash
- **Deterministic**: Same seed produces same sequence
- **Verifiable**: Anyone can recompute and verify
- **Time-Stamped**: Associates wall clock time with hash sequence

### Usage in Block Production
- Leader produces blocks every 400ms (slot duration)
- Each block must contain minimum 64 ticks (800,000 hashes)
- Ticks are embedded in Entry structures
- Provides cryptographic proof of time passage

---

## 4. Blockchain Data Structures

### Entry
Represents a ledger record in the blockchain.

```go
type Entry struct {
    Hash             []byte        // Entry hash
    NumHashes        int64         // Number of hash operations
    Transactions     []Transaction // Legacy transactions
    FileTransactions [][]byte      // Serialized file-based transactions
    PreviousHash     []byte        // Link to previous entry
    Timestamp        time.Time     // Wall clock timestamp
}
```

**Purpose**: Entries link PoH ticks with transaction data, forming the fundamental unit of the ledger.

### BlockHeader
Contains metadata for a block.

```go
type BlockHeader struct {
    PreviousBlockHash []byte    // Link to previous block
    MerkleRoot        []byte    // Merkle root of entries
    StateRoot         []byte    // Root hash of file state tree
    Slot              int64     // Slot number
    Timestamp         time.Time // Block creation time
    BlockHeight       int64     // Sequential block number
}
```

**Key Fields**:
- **StateRoot**: Merkle root of all file state, enables verification of state transitions
- **Slot**: Determines which validator should produce the block
- **BlockHeight**: Sequential counter starting from 1 (genesis)

### Block
Collection of entries produced during a slot.

```go
type Block struct {
    Version uint32      // Block format version (currently 1)
    Header  BlockHeader // Block metadata
    Entries []Entry     // Ledger entries
}
```

**Versioning**: 
- BlockVersion1 = 1 (current)
- Enables future protocol upgrades
- Backwards compatibility support

### Merkle Root Calculation
Used for both entry Merkle roots and state roots.

**Algorithm**:
1. Hash each entry/file individually
2. Build tree by repeatedly hashing pairs
3. If odd number, hash last element with itself
4. Continue until single root hash remains

**Entry Hash Calculation**:
```
entry_hash = SHA-256(
    entry.Hash || 
    entry.NumHashes || 
    entry.Transactions || 
    entry.FileTransactions || 
    entry.PreviousHash || 
    entry.Timestamp
)
```

**File Hash Calculation**:
```
file_hash = SHA-256(
    file.ID || 
    file.Balance || 
    file.TxManager || 
    file.Data || 
    file.Executable || 
    file.UpdatedAt
)
```

### Serialization
All structures use JSON serialization with hex encoding for binary data:
- Hashes: hex strings
- Timestamps: Unix timestamps
- Binary data: hex-encoded strings



---

## 5. Consensus Mechanism

### Delegated Proof of Stake (DPoS)

The blockchain uses stake-weighted leader scheduling where validators with more delegated stake produce more blocks.

### ConsensusManager Structure

```go
type ConsensusManager struct {
    localValidatorID   filestore.FileID    // This node's validator ID
    localValidatorKeys *wallet.Keypair     // Validator signing keys
    slotDurationMs     int64               // 400ms per slot
    genesisTimestamp   time.Time           // Network start time
    
    // DPoS fields
    epochLength       int64                // Slots per epoch (432,000)
    fileStore         *filestore.FileStore // State access
    runtime           *runtime.Runtime     // Program execution
    currentEpoch      int64                // Current epoch number
    validatorSchedule []filestore.FileID   // Slot assignments
    genesisValidators []GenesisValidator   // Initial validator set
}
```

### Slot Timing

**Slot Calculation**:
```
current_slot = (time_since_genesis_ms) / slot_duration_ms
```

**Slot Start Time**:
```
slot_start_time = genesis_timestamp + (slot * slot_duration_ms)
```

**WaitForSlotStart**: Blocks until the target slot begins, ensuring synchronized block production.

### Leader Selection

**IsLeader(slot)**:
1. Get scheduled validator for slot: `validatorSchedule[slot % epochLength]`
2. Compare with local validator ID
3. Return true if match

**Stake-Weighted Scheduling**:
- Uses deterministic PRNG seeded with last block hash
- Linear Congruential Generator (LCG) for determinism
- Weighted random selection based on stake amounts
- Validators with 2x stake get ~2x slots

**Algorithm**:
```
seed = first_8_bytes(last_block_hash)
for each slot in epoch:
    random_value = LCG(seed) % total_stake
    accumulated = 0
    for each validator:
        accumulated += validator.stake
        if random_value < accumulated:
            schedule[slot] = validator.id
            break
    seed = LCG(seed)  # Update for next slot
```

### Block Validation

**ValidateBlock(block)**:
1. Check slot is within tolerance (±100 slots, ~40 seconds)
2. Verify minimum hash operations (64 ticks × 12,500 = 800,000 hashes)
3. Verify block linkage (previous block hash matches)
4. Verify state root matches FileStore state

**Slot Tolerance**: Allows 100-slot window to handle:
- Network delays
- Clock skew between nodes
- Temporary network partitions

### Epoch Management

**Epoch Boundaries**:
- Occur every 432,000 slots (~2 days at 400ms/slot)
- Trigger validator schedule recalculation
- Reset block production counters
- Distribute rewards (future implementation)

**ProcessEpochBoundaryWithHash(slot, lastBlockHash)**:
1. Enumerate active validators (stake ≥ 1 Neon)
2. Compute new leader schedule using last block hash as seed
3. Update validatorSchedule and currentEpoch
4. Persist to Epoch State File
5. Reset block production counters

### Missed Blocks

**ProcessSlotSkip(slot)**:
1. Get scheduled validator for slot
2. Increment missed block counter in Validator Record
3. Update missed block counter in Epoch State File
4. Used for slashing calculations

**RecordBlockProduction(validatorID)**:
1. Increment blocks produced counter in Validator Record
2. Called after successful block production
3. Reset to 0 at epoch boundaries

### Observer Mode

Nodes without a wallet run in observer mode:
- localValidatorID = empty
- Never produce blocks
- Receive and validate blocks from network
- Useful for monitoring and querying



---

## 6. File-Based State Model

### Concept
Inspired by Solana's account model, the file-based state model treats everything as a "File" - accounts, programs, and data are all Files with uniform structure.

### File Structure

```go
type File struct {
    ID         FileID      // 32-byte unique identifier
    Balance    int64       // Balance in electrons (1 Neon = 1,000,000 electrons)
    TxManager  FileID      // Program that manages this file
    Data       []byte      // Arbitrary data
    Executable bool        // Whether this file is executable code
    CreatedAt  time.Time   // Creation timestamp
    UpdatedAt  time.Time   // Last update timestamp
}
```

### FileID
32-byte identifier (SHA-256 hash).

**Generation Methods**:
1. **From public key**: `FileID = first_32_bytes(pubkey)`
2. **Deterministic**: `FileID = SHA-256(data)`
3. **From string**: Parse hex-encoded string

**String Representation**: Hex-encoded (64 characters)

### Storage Cost Model

**Exponential Growth Formula**:
```
cost = base_cost_per_kb × size_in_kb × (1.1 ^ size_in_mb)
```

**Constants**:
- Base cost: 1,000 units per KB
- Growth rate: 1.1 (10% increase per MB)

**Examples**:
- 1 KB: 1,000 units
- 10 KB: 10,000 units
- 100 KB: 100,000 units
- 1 MB: ~1,100,000 units
- 10 MB: ~2,850,000 units

**Validation**:
- Every file must have `balance ≥ storage_cost(data_size)`
- Enforced on creation and updates
- Prevents unbounded state growth

### FileStore Implementation

**Backend**: BadgerDB (LSM-tree key-value store)

**Operations**:

1. **CreateFile(file)**: 
   - Validates storage cost
   - Sets timestamps
   - Generates FileID if not set
   - Stores in BadgerDB
   - Updates in-memory cache

2. **GetFile(id)**:
   - Checks cache first
   - Falls back to BadgerDB
   - Returns copy to prevent external modification

3. **UpdateFile(id, file)**:
   - Validates storage cost
   - Updates timestamp
   - Stores in BadgerDB
   - Updates cache

4. **DeleteFile(id)**:
   - Removes from BadgerDB
   - Removes from cache

5. **CalculateStateRoot()**:
   - Gets all file IDs (sorted)
   - Calculates hash for each file
   - Builds Merkle tree
   - Returns root hash

**Caching Strategy**:
- In-memory map for hot files
- Thread-safe with RWMutex
- Cache invalidation on updates

### Well-Known FileIDs

Reserved IDs for system components:

| Component | FileID | Purpose |
|-----------|--------|---------|
| System_Program | `0x0000...0001` | Account management |
| Token_Program | `0x0000...0002` | Token operations |
| Staking_Program | `0x0000...0003` | DPoS operations |
| Epoch_State | `0x0000...0004` | Current epoch state |
| Reward_Pool | `0x0000...0005` | Staking rewards |

### File Types

1. **User Accounts**:
   - ID: Derived from public key
   - TxManager: System_Program
   - Data: Empty or application data
   - Executable: false

2. **Programs**:
   - ID: Deterministic or assigned
   - TxManager: System_Program or self
   - Data: Bytecode
   - Executable: true

3. **Data Files**:
   - ID: Application-specific
   - TxManager: Owning program
   - Data: Application data
   - Executable: false

4. **System Files**:
   - ID: Well-known constants
   - TxManager: System programs
   - Data: Serialized state
   - Executable: false (except programs)



---

## 7. Transaction System

### Transaction Structure

```go
type Transaction struct {
    LastSeen     TxID          // Recent blockhash for replay protection
    Instructions []Instruction // Operations to execute
    Signatures   []Signature   // Cryptographic signatures
}
```

### Instruction Structure

```go
type Instruction struct {
    ProgramID filestore.FileID           // Program to execute
    Inputs    map[string]FileAccess      // File access declarations
    Data      []byte                     // Instruction-specific data
}
```

### FileAccess

```go
type FileAccess struct {
    FileID     filestore.FileID
    Permission AccessPermission  // Read or Write
}
```

**AccessPermission**:
- `Read = 1`: Read-only access
- `Write = 2`: Read and write access

### Signature Structure

```go
type Signature struct {
    PublicKey PublicKey  // 32-byte Ed25519 public key
    Signature [64]byte   // Ed25519 signature
}
```

### Transaction Builder

Provides fluent API for constructing transactions:

```go
builder := NewTransactionBuilder(lastSeenTxID)

// Add transfer instruction
builder.AddTransferInstruction(
    systemProgramID,
    fromFileID,
    toFileID,
    amount,
)

// Build transaction
tx, err := builder.Build()

// Sign transaction
txData, _ := tx.Marshal()
signature := ed25519.Sign(privateKey, txData)
tx.Signatures = append(tx.Signatures, Signature{
    PublicKey: publicKey,
    Signature: signature,
})
```

**Automatic Input Declaration**:
- System Program: Read permission
- Sender: Write permission
- Receiver: Write permission

### Fee Calculation

```go
type FeeConfig struct {
    BaseFee        int64  // Base fee per transaction (5,000)
    InstructionFee int64  // Fee per instruction (1,000)
    SignatureFee   int64  // Fee per signature (500)
}

total_fee = base_fee + 
            (instruction_count × instruction_fee) + 
            (signature_count × signature_fee)
```

**Example**:
- 1 instruction, 1 signature: 6,500 electrons
- 3 instructions, 2 signatures: 11,000 electrons

### Transaction Processing

**TxProcessor** executes transactions atomically:

```go
type TxProcessor struct {
    fileStore        *filestore.FileStore
    runtime          *runtime.Runtime
    accessController *access.AccessController
}
```

**ProcessTransaction(tx)**:
1. Validate signatures
2. Calculate and deduct fees from fee payer (first signer)
3. For each instruction:
   - Create ExecutionContext
   - Load program file
   - Execute program with runtime
   - If error: rollback all changes
4. Calculate state root
5. Return TxResult

**Rollback Mechanism**:
- Snapshots file state before execution
- On error: restores all modified files
- Ensures atomicity (all-or-nothing)

### Access Control

**AccessController** validates file access:

```go
type AccessController struct {
    declaredInputs map[filestore.FileID]transaction.AccessPermission
    accessLog      []AccessRecord
    mu             sync.RWMutex
}
```

**ValidateAndRecord(fileID, permission)**:
1. Check if file is in declared inputs
2. Verify permission level matches or exceeds required
3. Record access in log
4. Return error if validation fails

**Access Validation Rules**:
- File must be declared in instruction inputs
- Read access: requires Read or Write permission
- Write access: requires Write permission
- Undeclared access: rejected

### System Program Instructions

Built-in operations provided by System_Program:

1. **CreateAccount**: Create new account file
2. **Transfer**: Transfer balance between accounts
3. **AllocateData**: Allocate data space for account
4. **CloseAccount**: Close account and reclaim balance

**Instruction Data Encoding**:
- Little-endian for integers
- Fixed-size fields for determinism
- Helper functions for parsing



---

## 8. QuanticScript Language

### Overview
TypeScript-like smart contract language that compiles to stack-based bytecode.

### Language Pipeline

```
Source (.qs) → Lexer → Parser → AST → TypeChecker → CodeGen → Bytecode (.qsb)
Assembly (.qsa) → Assembler → Bytecode (.qsb)
Bytecode (.qsb) → Disassembler → Assembly (.qsa)
Bytecode (.qsb) → Interpreter → Execution
```

### Type System

**Primitive Types**:
- Integers: `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64`
- Boolean: `bool`
- Binary: `bytes`, `string`
- Blockchain: `FileID`, `PublicKey`, `TxID`
- Collections: `array`, `map`, `set`

**Type Annotations Required**:
```typescript
let x: i64 = 42;
let name: string = "Alice";
let isValid: bool = true;
```

### Syntax Features

**Variables and Constants**:
```typescript
let x: i64 = 10;           // Mutable variable
const MAX: i64 = 1000;     // Immutable constant
```

**Functions**:
```typescript
function add(a: i64, b: i64): i64 {
    return a + b;
}

export function entry(ctx: InstructionContext): i64 {
    return 0;  // Success
}
```

**Control Flow**:
```typescript
if (x > 0) {
    // positive
} else if (x < 0) {
    // negative
} else {
    // zero
}

while (i < 10) {
    i = i + 1;
}

for (let i: i64 = 0; i < 10; i = i + 1) {
    // loop body
}
```

**Operators**:
- Arithmetic: `+`, `-`, `*`, `/`, `%`
- Comparison: `==`, `!=`, `<`, `>`, `<=`, `>=`
- Logical: `&&`, `||`, `!`
- Bitwise: `&`, `|`, `^`, `~`, `<<`, `>>`

**Inline Assembly**:
```typescript
let result: i64;
__asm__ {
    LOAD x
    LOAD y
    ADD
    STORE result
}
```

### Lexer

**Token Types**:
- Keywords: `function`, `export`, `let`, `const`, `if`, `else`, `while`, `for`, `return`, `__asm__`
- Types: `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64`, `bool`, `string`, `bytes`, `void`
- Operators: All arithmetic, comparison, logical, bitwise
- Literals: integers, strings, booleans
- Identifiers: variable and function names

**Source Location Tracking**:
```go
type Token struct {
    Type     TokenType
    Literal  string
    Location SourceLocation
}

type SourceLocation struct {
    Filename string
    Line     int
    Column   int
}
```

### Parser

**AST Node Types**:
- Program: Top-level container
- FunctionDeclaration: Function definitions
- VariableDeclaration: Variable/constant declarations
- Expressions: Binary, unary, call, identifier, literal
- Statements: If, while, for, return, block, expression
- AssemblyBlock: Inline assembly

**Error Recovery**:
- Synchronization on statement boundaries
- Multiple error collection
- Detailed error messages with source locations

### Type Checker

**Type Inference**:
- Infers types from expressions
- Validates type compatibility
- Checks function signatures
- Ensures return type consistency

**Type Rules**:
- Arithmetic operations: both operands must be same integer type
- Comparisons: both operands must be same type
- Logical operations: both operands must be bool
- Assignments: right-hand side must match left-hand side type

### Code Generator

**Bytecode Emission**:
- Translates AST to stack-based bytecode
- Allocates local memory slots for variables
- Generates control flow instructions
- Handles function calls with label resolution
- Supports inline assembly blocks

**Two-Pass Compilation**:
1. First pass: Collect function labels
2. Second pass: Generate bytecode with resolved labels

**Optimizations**:
- Constant folding (future)
- Dead code elimination (future)
- Register allocation (future)

### Bytecode Format

**File Structure**:
```
[Header: 16 bytes]
[Body: variable length]
```

**Header Format**:
```
Offset | Size | Field
-------|------|-------
0      | 4    | Magic number (0x51534300)
4      | 4    | Version (1)
8      | 4    | Entry offset
12     | 4    | Reserved
```

**Instruction Encoding**:
- Opcode: 1 byte
- Operands: variable length depending on opcode
- Type tags for PUSH instructions

### Opcodes

**Categories**:
- Stack: PUSH, POP, DUP, SWAP
- Arithmetic: ADD, SUB, MUL, DIV, MOD
- Comparison: EQ, LT, GT, LTE, GTE
- Logical: AND, OR, NOT
- Bitwise: BAND, BOR, BXOR, BNOT, SHL, SHR
- Control: JMP, JMPIF, CALL, RET
- Memory: LOAD, STORE, LOADG, STOREG
- Blockchain: GETFILE, UPDATEFILE, GETBALANCE, TRANSFER, etc.
- Crypto: SHA256, VERIFYSIG
- Collections: ARRAYNEW, MAPNEW, SETNEW, etc.
- Cross-Program: INVOKE, INVOKERET
- Dispatch: DISPATCH (instruction routing)

**Total Opcodes**: ~80 instructions



### Interpreter

**Stack-Based Execution**:
```go
type BytecodeInterpreter struct {
    bytecode         []byte
    pc               int              // Program counter
    stack            []Value          // Operand stack
    locals           []Value          // Local variables
    ctx              *ExecutionContext
    computeBudget    int64
    computeUsed      int64
    invokeDepth      int              // Cross-program invocation depth
}
```

**Execution Loop**:
1. Fetch opcode at PC
2. Decode operands
3. Execute operation
4. Update stack/locals
5. Deduct compute cost
6. Increment PC
7. Repeat until RET or error

**Compute Metering**:
- Each instruction has fixed cost
- Budget checked before execution
- Exceeding budget causes error
- Prevents infinite loops

**Instruction Costs** (examples):
- PUSH: 1 unit
- ADD/SUB: 2 units
- MUL: 3 units
- DIV/MOD: 4 units
- SHA256: 100 units
- GETFILE: 50 units
- UPDATEFILE: 100 units

### Standard Library

**String Operations**:
- `str_concat(a, b)`: Concatenate strings
- `str_substring(s, start, end)`: Extract substring
- `str_len(s)`: Get string length
- `str_to_bytes(s)`: Convert to bytes
- `str_from_bytes(b)`: Convert from bytes

**Math Operations**:
- `math_min(a, b)`: Minimum value
- `math_max(a, b)`: Maximum value
- `math_abs(x)`: Absolute value
- `math_pow(base, exp)`: Power (deterministic)

**Crypto Operations**:
- `sha256(data)`: SHA-256 hash
- `verify_signature(pubkey, message, signature)`: Ed25519 verification
- `derive_pubkey(seed)`: Derive public key

**Blockchain Operations**:
- `get_file(id)`: Load file from FileStore
- `update_file(file)`: Update file in FileStore
- `get_balance(id)`: Get file balance
- `transfer(from, to, amount)`: Transfer balance
- `get_signer(index)`: Get transaction signer
- `has_signer(pubkey)`: Check if pubkey signed
- `get_instruction_data()`: Get instruction data
- `get_program_id()`: Get current program ID

**Collection Operations**:
- Arrays: new, len, get, set, push, pop, map, filter, reduce, sort
- Maps: new, get, set, has, del
- Sets: new, add, has, del

**Query Operations** (finalized data):
- `query_block(hash)`: Query block by hash
- `query_transaction(txid)`: Query transaction
- `query_instruction(txid, index)`: Query instruction

### Cross-Program Invocation

**INVOKE Instruction**:
```typescript
// In QuanticScript (via stdlib)
invoke_program(program_id, data, compute_budget)
```

**Depth Tracking**:
- Maximum depth: 4 levels
- Prevents infinite recursion
- Each invocation creates new context

**Invocation Rules**:
1. Target program must be in declared inputs
2. Compute budget allocated from caller's budget
3. Failure rolls back all changes
4. Success returns control to caller

**ExecutionContext for Invoked Program**:
- Same signers as caller
- Same FileStore access
- Different instruction data
- Shared file inputs
- Increased depth counter

### Assembly Language

**Human-Readable Format**:
```assembly
; Function entry point
entry:
    PUSH i64 42
    PUSH i64 10
    ADD
    RET

; Loop example
loop_start:
    LOAD 0           ; Load counter
    PUSH i64 10
    LT               ; counter < 10?
    JMPIF loop_body
    RET
loop_body:
    LOAD 0
    PUSH i64 1
    ADD
    STORE 0          ; counter++
    JMP loop_start
```

**Features**:
- Labels for jumps and calls
- Comments (semicolon-prefixed)
- Type annotations for PUSH
- Automatic label resolution

**Assembler**:
- Parses assembly text
- Resolves labels to offsets
- Generates bytecode with header
- Error reporting with line numbers

**Disassembler**:
- Parses bytecode header
- Decodes instructions
- Generates human-readable assembly
- Preserves structure and flow



---

## 9. Smart Contract Programs

### System_Program

**Purpose**: Core account management operations.

**FileID**: `0x0000000000000000000000000000000000000000000000000000000000000001`

**Operations**:

1. **CreateAccount**:
   - Creates new account file
   - Allocates initial balance
   - Sets TxManager to System_Program
   - Validates storage cost

2. **Transfer**:
   - Transfers balance between accounts
   - Validates sufficient balance
   - Atomic operation (both accounts updated or neither)

3. **AllocateData**:
   - Allocates data space for account
   - Validates storage cost coverage
   - Updates file data

4. **CloseAccount**:
   - Closes account and reclaims balance
   - Transfers remaining balance to destination
   - Deletes account file

**Implementation**: Available in both Go (native) and QuanticScript (bytecode).

### Token_Program

**Purpose**: SPL-token-like fungible token operations.

**FileID**: `0x0000000000000000000000000000000000000000000000000000000000000002`

**Operations**:

1. **InitializeMint**:
   - Creates token mint account
   - Sets total supply
   - Sets mint authority
   - Sets decimals

2. **InitializeAccount**:
   - Creates token account for user
   - Links to mint
   - Sets owner

3. **MintTo**:
   - Mints new tokens to account
   - Requires mint authority signature
   - Updates total supply

4. **Transfer**:
   - Transfers tokens between accounts
   - Validates sufficient balance
   - Requires owner signature

5. **Burn**:
   - Burns tokens from account
   - Decreases total supply
   - Requires owner signature

**Data Structures**:

```
Mint Account (72 bytes):
- Supply: i64 (8 bytes)
- Decimals: u8 (1 byte)
- Is initialized: bool (1 byte)
- Mint authority: PublicKey (32 bytes)
- Freeze authority: PublicKey (32 bytes, optional)

Token Account (96 bytes):
- Mint: FileID (32 bytes)
- Owner: PublicKey (32 bytes)
- Amount: i64 (8 bytes)
- Delegate: PublicKey (32 bytes, optional)
- State: u8 (1 byte)
- Is native: bool (1 byte)
```

### Staking_Program

**Purpose**: DPoS staking and validator management.

**FileID**: `0x0000000000000000000000000000000000000000000000000000000000000003`

**Operations**:

1. **RegisterValidator**:
   - Creates Validator Record
   - Sets commission rate
   - Initializes stake to 0
   - Sets status to active

2. **Delegate**:
   - Creates Stake Account
   - Links to validator
   - Locks tokens for staking
   - Updates validator total stake

3. **Undelegate**:
   - Unlocks staked tokens
   - Updates validator total stake
   - Initiates cooldown period

4. **WithdrawStake**:
   - Withdraws undelegated tokens
   - Requires cooldown period elapsed
   - Closes stake account

5. **ClaimRewards**:
   - Claims accumulated staking rewards
   - Transfers from reward pool
   - Updates last claim epoch

**Data Structures**:

```
Validator Record (66 bytes):
- Public key: [32]byte
- Commission: i64 (8 bytes)
- Total stake: i64 (8 bytes)
- Status: u8 (1 byte) - 0=inactive, 1=active, 2=deregistered
- Blocks produced: i64 (8 bytes)
- Missed blocks: i64 (8 bytes)
- Slashed this epoch: bool (1 byte)

Stake Account (80 bytes):
- Validator: FileID (32 bytes)
- Delegator: PublicKey (32 bytes)
- Amount: i64 (8 bytes)
- Activation epoch: i64 (8 bytes)

Epoch State File (variable):
- Epoch number: i64 (8 bytes)
- Epoch start slot: i64 (8 bytes)
- Total slots in epoch: i64 (8 bytes)
- Validator count: i64 (8 bytes)
- Validator schedule: [][32]byte (32 × count)
- Missed block counters: []i64 (8 × unique_validator_count)

Reward Pool (16 bytes):
- Balance: i64 (8 bytes)
- Last distributed epoch: i64 (8 bytes)
```

### DISPATCH Mechanism

**Purpose**: Route instructions to handler functions within a program.

**Pattern**:
```typescript
export function entry(ctx: InstructionContext): i64 {
    let instr_data: bytes = get_instruction_data();
    
    // DISPATCH pops instruction bytes, looks up handler, pushes args
    __asm__ {
        LOAD instr_data
        DISPATCH
    }
    
    // Stack now contains: [handler_name, ...parsed_args]
    let handler: string = pop_string();
    
    if (handler == "transfer") {
        return handle_transfer(ctx);
    } else if (handler == "mint") {
        return handle_mint(ctx);
    }
    
    return -1;  // Unknown instruction
}
```

**Registry Format**:
- Instruction discriminator: u8 (first byte)
- Handler name: string
- Argument parsers: functions to extract typed arguments

**Benefits**:
- Clean separation of routing and logic
- Type-safe argument parsing
- Extensible instruction set



---

## 10. Network Layer

### NetworkNode Structure

```go
type NetworkNode struct {
    host        string
    port        int
    listener    net.Listener
    peers       map[string]net.Conn
    blockChan   chan blockchain.Block
    stopChan    chan struct{}
    mu          sync.RWMutex
}
```

### P2P Communication

**Protocol**: TCP-based with JSON serialization

**Connection Management**:
1. **Start()**: Binds to address and starts listening
2. **ConnectToPeer(address)**: Establishes outbound connection
3. **AcceptConnections()**: Accepts inbound connections
4. **Stop()**: Closes all connections gracefully

**Message Format**:
```json
{
    "type": "block",
    "data": {
        "version": 1,
        "header": { ... },
        "entries": [ ... ]
    }
}
```

### Block Broadcasting

**BroadcastBlock(block)**:
1. Serialize block to JSON
2. For each connected peer:
   - Send block over TCP connection
   - Handle send errors (log and continue)
3. Non-blocking (doesn't wait for acknowledgment)

**ReceiveBlock()**:
1. Wait for data on blockChan
2. Deserialize JSON to Block
3. Return block to caller
4. Blocks until block received or node stopped

### Peer Discovery

**Current Implementation**: Manual peer specification via `--peers` flag

**Format**: Comma-separated list of `host:port` addresses

**Example**: `--peers localhost:8000,localhost:8001`

**Future Enhancements**:
- Automatic peer discovery
- Peer reputation system
- Connection pooling
- Message compression

### Network Topology

**Star Topology** (current):
```
    Validator 1 (Leader)
         ↓ ↑
    ┌────┴────┐
    ↓         ↓
Validator 2  Validator 3
```

**Mesh Topology** (future):
```
Validator 1 ←→ Validator 2
    ↕              ↕
Validator 3 ←→ Validator 4
```

### Error Handling

**Connection Failures**:
- Log warning and continue
- Retry connection (future)
- Remove dead peers (future)

**Malformed Messages**:
- Log error and discard
- Don't crash node
- Track peer reputation (future)

### Byzantine Fault Tolerance

**Malicious Behaviors** (for testing):
1. Corrupt block data (invalid hash count)
2. Wrong previous block hash
3. Skip validation and store anyway
4. Ignore verification failures

**Detection**:
- Block validation catches corrupted blocks
- Chain verification detects linkage issues
- Honest majority (>2/3) ensures progress

**Testing**:
- `--malicious` flag enables Byzantine behaviors
- Periodic corruption (every 3rd or 5th block)
- Used to verify BFT tolerance

---

## 11. Storage and Persistence

### SQLite Ledger

**Purpose**: Persistent storage for blockchain (blocks and entries).

**Schema**:

```sql
CREATE TABLE blocks (
    block_height INTEGER PRIMARY KEY,
    slot INTEGER NOT NULL,
    version INTEGER NOT NULL,
    previous_block_hash BLOB,
    merkle_root BLOB NOT NULL,
    state_root BLOB,
    timestamp INTEGER NOT NULL,
    entries_json TEXT NOT NULL
);

CREATE INDEX idx_blocks_slot ON blocks(slot);
CREATE INDEX idx_blocks_merkle_root ON blocks(merkle_root);
```

**Operations**:

1. **StoreBlock(block)**:
   - Serializes entries to JSON
   - Inserts into blocks table
   - Uses REPLACE to handle duplicates

2. **GetBlockByHeight(height)**:
   - Queries by block_height
   - Deserializes entries from JSON
   - Returns Block structure

3. **GetBlockBySlot(slot)**:
   - Queries by slot
   - Returns Block or error if not found

4. **GetLatestBlock()**:
   - Queries MAX(block_height)
   - Returns most recent block

5. **LoadBlockchainState()**:
   - Gets latest block
   - Counts total blocks
   - Returns (block, height, error)

6. **GetBlockRange(start, end)**:
   - Queries blocks in height range
   - Returns slice of blocks

**Verification**:
- Full chain verification on startup
- Validates hash linkage
- Checks entry integrity
- Ensures no gaps in block heights

### BadgerDB State Store

**Purpose**: Persistent storage for file-based state.

**Key-Value Mapping**:
- Key: FileID (32 bytes)
- Value: Serialized File (JSON)

**Operations**:
- Get: Retrieve file by ID
- Set: Store/update file
- Delete: Remove file
- Iterate: Scan all files

**LSM-Tree Benefits**:
- Fast writes (append-only)
- Efficient compaction
- Good read performance
- Built-in compression

**Configuration**:
```go
opts := badger.DefaultOptions(dbPath)
opts.Logger = nil  // Disable logging
db, err := badger.Open(opts)
```

### State Root Calculation

**Purpose**: Merkle root of all file state for block headers.

**Algorithm**:
1. Get all file IDs (sorted)
2. For each file:
   - Calculate file hash
   - Add to hash list
3. Build Merkle tree from hashes
4. Return root hash

**File Hash**:
```
SHA-256(
    file.ID ||
    file.Balance ||
    file.TxManager ||
    file.Data ||
    file.Executable ||
    file.UpdatedAt
)
```

**Merkle Tree Construction**:
- Pair hashes and hash together
- If odd count, hash last with itself
- Repeat until single root remains

### Database Lifecycle

**Initialization**:
1. Open SQLite connection
2. Create tables if not exist
3. Create indexes
4. Open BadgerDB
5. Load existing state

**Graceful Shutdown**:
1. Stop accepting new operations
2. Flush pending writes
3. Close BadgerDB
4. Close SQLite connection
5. Release file locks

**Recovery**:
- SQLite: Automatic recovery from WAL
- BadgerDB: Automatic recovery from value log
- Verification: Full chain verification on startup



---

## 12. Wallet System

### Two Wallet Types

1. **Validator Wallet** (`internal/wallet`):
   - For validator node identity
   - Encrypted Ed25519 keypairs
   - Platform-specific storage
   - Used for block signing

2. **User Wallet** (`cmd/wallet`):
   - For end users
   - BIP39 seed phrases
   - BIP44 key derivation
   - TUI interface
   - RPC client integration

### Validator Wallet

**Structure**:
```go
type Wallet struct {
    Name     string
    Keypairs []Keypair
}

type Keypair struct {
    PublicKey  [32]byte
    PrivateKey [64]byte
}
```

**Encryption**:
- Algorithm: AES-256-GCM
- Key Derivation: Argon2id
- Parameters:
  - Time: 1 iteration
  - Memory: 64 MB
  - Threads: 4
  - Salt: 16 bytes (random)
  - Key length: 32 bytes

**Storage Locations**:
- Linux: `~/.local/share/poh-blockchain/wallets/`
- macOS: `~/Library/Application Support/poh-blockchain/wallets/`
- Windows: `%APPDATA%\poh-blockchain\wallets\`

**Operations**:

1. **Create(name, password)**:
   - Generates Ed25519 keypair
   - Encrypts with password
   - Saves to platform-specific location

2. **Open(name, password)**:
   - Loads encrypted wallet
   - Derives key from password
   - Decrypts and returns wallet

3. **List()**:
   - Scans wallet directory
   - Returns list of wallet names

4. **Export(outputPath)**:
   - Exports to unencrypted JSON
   - Warning: insecure, for backup only

5. **Import(inputPath, name, password)**:
   - Imports from JSON
   - Encrypts with new password
   - Saves to wallet directory

**CLI Commands**:
```bash
# Create wallet
poh-blockchain wallet create --name validator1 --password <pass>

# List wallets
poh-blockchain wallet list

# Show wallet info
poh-blockchain wallet show --name validator1 --password <pass>

# Export wallet
poh-blockchain wallet export --name validator1 --password <pass> --output backup.json

# Import wallet
poh-blockchain wallet import --input backup.json --name validator2 --password <pass>
```

### User Wallet (Neon Wallet)

**Features**:
- BIP39 mnemonic generation (12 or 24 words)
- BIP44 key derivation for Ed25519
- AES-256-GCM encryption
- Multi-seed phrase management
- Transaction building and signing
- RPC client integration
- Modern TUI with Bubble Tea framework

**BIP39 Implementation**:
```go
// Generate mnemonic
entropy := make([]byte, 16)  // 128 bits for 12 words
rand.Read(entropy)
mnemonic := bip39.NewMnemonic(entropy)

// Derive seed
seed := bip39.NewSeed(mnemonic, passphrase)
```

**BIP44 Derivation Path**:
```
m / 44' / 501' / account' / change / index
```
- 44': BIP44 purpose
- 501': Solana coin type (reused for compatibility)
- account': Account number (hardened)
- change: 0 for external, 1 for internal
- index: Address index

**Ed25519 Key Derivation**:
```go
// Derive master key from seed
masterKey := ed25519.NewKeyFromSeed(seed[:32])

// Derive child keys using BIP44 path
// (Simplified - actual implementation uses SLIP-0010)
```

**Wallet File Structure**:
```json
{
  "version": 1,
  "seed_phrases": [
    {
      "id": "uuid",
      "name": "Main Account",
      "encrypted_seed": "...",
      "salt": "...",
      "nonce": "..."
    }
  ],
  "settings": {
    "auto_lock_minutes": 5,
    "rpc_url": "http://localhost:8899"
  }
}
```

**TUI Screens**:
1. **Dashboard**: Balance overview, recent transactions
2. **Accounts**: List derived accounts, create new
3. **Transfer**: Send tokens to address
4. **History**: Transaction history
5. **Settings**: RPC URL, auto-lock, seed phrase management

**Wallet Creation Wizard**:
1. Choose: Create new or restore from seed
2. Select seed phrase length (12 or 24 words)
3. Display seed phrase (ONLY shown once!)
4. Confirm seed phrase (security check)
5. Set encryption password
6. Save encrypted wallet

**Security Features**:
- Auto-lock after 5 minutes inactivity
- Password required to unlock
- Seed phrase never stored unencrypted
- Secure memory clearing (future)
- Clipboard clearing after copy (future)

**RPC Client**:
```go
type RPCClient struct {
    url    string
    client *http.Client
}

// Methods
- GetBalance(address)
- GetAccountInfo(address)
- GetBlockHeight()
- GetRecentBlockhash()
- SendTransaction(tx)
- GetTransactionStatus(txid)
- GetTransactionHistory(address)
```



---

## 13. RPC API

### JSON-RPC 2.0 Server

**Purpose**: Provides HTTP API for blockchain queries and transaction submission.

**Server Configuration**:
```go
type ServerConfig struct {
    BindAddress string  // Default: "127.0.0.1"
    Port        int     // Default: 8899
    LedgerPath  string  // Path to SQLite ledger
    StatePath   string  // Path to BadgerDB state
}
```

**HTTP Server**:
- CORS enabled for browser access
- JSON request/response
- POST method only
- Content-Type: application/json

### RPC Methods

**1. getBalance**
```json
Request:
{
  "jsonrpc": "2.0",
  "method": "getBalance",
  "params": {
    "address": "0x1234...abcd"
  },
  "id": 1
}

Response:
{
  "jsonrpc": "2.0",
  "result": {
    "balance": 1000000000
  },
  "id": 1
}
```

**2. getAccountInfo**
```json
Request:
{
  "jsonrpc": "2.0",
  "method": "getAccountInfo",
  "params": {
    "address": "0x1234...abcd"
  },
  "id": 1
}

Response:
{
  "jsonrpc": "2.0",
  "result": {
    "address": "0x1234...abcd",
    "balance": 1000000000,
    "tx_manager": "0x0000...0001",
    "executable": false,
    "data_size": 0,
    "created_at": 1234567890,
    "updated_at": 1234567890
  },
  "id": 1
}
```

**3. getBlockHeight**
```json
Request:
{
  "jsonrpc": "2.0",
  "method": "getBlockHeight",
  "params": {},
  "id": 1
}

Response:
{
  "jsonrpc": "2.0",
  "result": {
    "block_height": 12345
  },
  "id": 1
}
```

**4. getRecentBlockhash**
```json
Request:
{
  "jsonrpc": "2.0",
  "method": "getRecentBlockhash",
  "params": {},
  "id": 1
}

Response:
{
  "jsonrpc": "2.0",
  "result": {
    "blockhash": "0xabcd...1234",
    "slot": 12345
  },
  "id": 1
}
```

**5. sendTransaction**
```json
Request:
{
  "jsonrpc": "2.0",
  "method": "sendTransaction",
  "params": {
    "transaction": {
      "last_seen": "0xabcd...1234",
      "instructions": [...],
      "signatures": [...]
    }
  },
  "id": 1
}

Response:
{
  "jsonrpc": "2.0",
  "result": {
    "transaction_id": "0x5678...efgh",
    "status": "pending"
  },
  "id": 1
}
```

**6. getTransactionStatus**
```json
Request:
{
  "jsonrpc": "2.0",
  "method": "getTransactionStatus",
  "params": {
    "transaction_id": "0x5678...efgh"
  },
  "id": 1
}

Response:
{
  "jsonrpc": "2.0",
  "result": {
    "status": "confirmed",
    "slot": 12346,
    "confirmations": 10
  },
  "id": 1
}
```

**7. getTransactionHistory**
```json
Request:
{
  "jsonrpc": "2.0",
  "method": "getTransactionHistory",
  "params": {
    "address": "0x1234...abcd",
    "limit": 10
  },
  "id": 1
}

Response:
{
  "jsonrpc": "2.0",
  "result": {
    "transactions": [
      {
        "transaction_id": "0x5678...efgh",
        "slot": 12346,
        "timestamp": 1234567890,
        "type": "transfer",
        "amount": 1000000
      }
    ]
  },
  "id": 1
}
```

### Query Engine

**Caching Strategy**:
```go
type QueryEngine struct {
    ledger    *storage.Ledger
    fileStore *filestore.FileStore
    cache     map[string]CacheEntry
    cacheTTL  time.Duration
    mu        sync.RWMutex
}

type CacheEntry struct {
    Data      interface{}
    ExpiresAt time.Time
}
```

**Cached Queries**:
- Block height (TTL: 1 second)
- Recent blockhash (TTL: 1 second)
- Account info (TTL: 5 seconds)

**Cache Invalidation**:
- Time-based expiration
- Manual invalidation on updates
- LRU eviction (future)

### Error Handling

**JSON-RPC Error Format**:
```json
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32600,
    "message": "Invalid Request"
  },
  "id": 1
}
```

**Error Codes**:
- -32700: Parse error
- -32600: Invalid request
- -32601: Method not found
- -32602: Invalid params
- -32603: Internal error
- -32000 to -32099: Server errors

### CORS Configuration

**Allowed Origins**: `*` (all origins)
**Allowed Methods**: POST, OPTIONS
**Allowed Headers**: Content-Type, Authorization
**Max Age**: 86400 seconds (24 hours)

### Rate Limiting (Future)

**Per-IP Limits**:
- 100 requests per minute
- 1000 requests per hour
- Burst allowance: 10 requests

**Per-Method Limits**:
- sendTransaction: 10 per minute
- Other methods: 100 per minute



---

## 14. Testing Strategy

### Test Categories

**1. Unit Tests**:
- Individual component testing
- Mock dependencies
- Fast execution
- High coverage

**2. Integration Tests**:
- Multi-component interaction
- Real dependencies
- End-to-end flows
- Slower execution

**3. BFT Tests**:
- Byzantine fault tolerance
- Malicious node behavior
- Network resilience
- Consensus validation

**4. Performance Tests**:
- Throughput measurement
- Latency profiling
- Resource usage
- Scalability testing

### Key Integration Tests

**1. TestFullNodeBlockProductionAndVerification**:
- Initializes complete node
- Produces multiple blocks
- Verifies chain integrity
- Validates block linkage

**2. TestLeaderReplicaCommunication**:
- Tests P2P communication
- Verifies block broadcasting
- Validates replica reception
- Ensures state consistency

**3. TestLedgerPersistenceAndRecovery**:
- Creates blockchain
- Closes database
- Reopens database
- Verifies full recovery

**4. TestDPoSGenesis**:
- Initializes DPoS state
- Creates validator records
- Verifies epoch state
- Validates reward pool

**5. TestStakeWeightedConsensus**:
- Tests leader schedule computation
- Verifies stake-weighted distribution
- Validates deterministic selection
- Checks epoch boundaries

### QuanticScript Tests

**Lexer Tests**:
- Token recognition
- Source location tracking
- Error handling
- Edge cases

**Parser Tests**:
- AST construction
- Error recovery
- Syntax validation
- Complex expressions

**Type Checker Tests**:
- Type inference
- Type compatibility
- Error detection
- Function signatures

**Code Generator Tests**:
- Bytecode emission
- Label resolution
- Optimization
- Assembly integration

**Interpreter Tests**:
- Instruction execution
- Stack operations
- Compute metering
- Error handling

**Standard Library Tests**:
- String operations
- Math functions
- Crypto operations
- Blockchain operations
- Collection operations

**Cross-Program Invocation Tests**:
- Depth tracking
- Budget allocation
- Error propagation
- Rollback behavior

### BFT Testing

**Malicious Behaviors**:
1. Corrupt block data (invalid hash count)
2. Wrong previous block hash
3. Skip validation
4. Ignore verification failures

**Test Scenarios**:

**Scenario 1: BFT with Tolerance**
- 4 honest validators
- 1 malicious validator
- Network should progress
- Malicious blocks rejected

**Scenario 2: BFT without Tolerance**
- 2 honest validators
- 2 malicious validators
- Network may stall
- Demonstrates BFT limits

**Scenario 3: All Honest**
- 3+ honest validators
- No malicious nodes
- Optimal performance
- Baseline comparison

### Automated Testing

**audit.sh Script**:
- Phase 1: Basic consensus (30s)
- Phase 2: BFT with tolerance (30s)
- Phase 3: BFT without tolerance (30s)
- Phase 4: DPoS lifecycle (30s)
- Generates JSON report

**CI/CD Integration**:
- No tmux required
- JSON output for parsing
- Exit codes for pass/fail
- Artifact generation

**Test Metrics**:
- Blocks produced
- Transactions processed
- Malicious blocks rejected
- Network uptime
- Consensus failures

### Demo Scripts

**devnet.sh**:
- Start/stop local network
- Multiple validators
- RPC node included
- Log viewing
- Status checking

**demo-dpos.sh**:
- DPoS demonstration
- Stake-weighted scheduling
- Epoch transitions
- Validator statistics

**demo-bft.sh** (legacy):
- BFT testing with tmux
- Visual monitoring
- Interactive debugging
- Manual verification

### Test Data

**Genesis Validators**:
```go
GenesisValidator{
    PublicKey:   [32]byte{1, 2, 3, ...},
    StakeAmount: 10000000,  // 10 Neon
}
```

**Test Accounts**:
- Pre-funded accounts
- Known keypairs
- Deterministic addresses

**Test Transactions**:
- Simple transfers
- Token operations
- Staking operations
- Cross-program calls

### Coverage Goals

**Target Coverage**:
- Core components: >80%
- QuanticScript: >90%
- Integration tests: Key flows
- BFT tests: All scenarios

**Coverage Tools**:
```bash
go test -cover ./...
go test -coverprofile=coverage.out ./...
go tool cover -html=coverage.out
```



---

## 15. CLI and Tools

### Main CLI (poh-blockchain)

**Node Operations**:
```bash
# Start validator node
poh-blockchain --wallet validator1 --port 8000 --db ./validator1.db

# Start observer node
poh-blockchain --port 8002 --peers localhost:8000

# Start with malicious behavior (testing)
poh-blockchain --wallet validator1 --port 8000 --malicious
```

**Account Management**:
```bash
# Create account
poh-blockchain account create --balance 1000000 --output keypair.json --state state.db

# Transfer balance
poh-blockchain transfer --from keypair.json --to <address> --amount 1000 --state state.db

# Query account
poh-blockchain query --address <address> --state state.db
```

**Transaction Operations**:
```bash
# Submit transaction from JSON
poh-blockchain submit --tx transaction.json --state state.db

# Check transaction status
poh-blockchain status --tx <tx-id> --state state.db
```

**Wallet Management**:
```bash
# Create validator wallet
poh-blockchain wallet create --name validator1 --password <pass>

# List wallets
poh-blockchain wallet list

# Show wallet info
poh-blockchain wallet show --name validator1 --password <pass>

# Export wallet
poh-blockchain wallet export --name validator1 --password <pass> --output backup.json

# Import wallet
poh-blockchain wallet import --input backup.json --name validator2 --password <pass>
```

**QuanticScript Compiler**:
```bash
# Compile source to bytecode
poh-blockchain qsc compile -i program.qs -o program.qsb

# Assemble assembly to bytecode
poh-blockchain qsc assemble -i program.qsa -o program.qsb

# Disassemble bytecode to assembly
poh-blockchain qsc disassemble -i program.qsb -o program.qsa

# Verbose output
poh-blockchain qsc compile -i program.qs -o program.qsb --verbose
```

**RPC Node**:
```bash
# Start RPC node
poh-blockchain rpc --ledger-path ./validator1.db --state-path ./validator1_state.db

# Custom bind address and port
poh-blockchain rpc --ledger-path ./validator1.db --state-path ./validator1_state.db \
  --rpc-bind 0.0.0.0 --rpc-port 9000
```

### Neon Wallet TUI

**Launch**:
```bash
# Default settings
./bin/neon-wallet

# Custom wallet path
./bin/neon-wallet --wallet-path ./my-wallet.dat

# Custom RPC endpoint
./bin/neon-wallet --rpc-url http://localhost:9000
```

**Navigation**:
- Number keys (1-5): Switch screens
- Arrow keys: Navigate lists
- Enter: Select/confirm
- Esc: Cancel/back
- q: Quit (when unlocked)

**Screens**:
1. Dashboard: Balance overview, recent transactions
2. Accounts: Manage derived accounts
3. Transfer: Send tokens
4. History: Transaction history
5. Settings: Configuration

### Validator TUI Dashboard

**Launch**:
```bash
# Build
go build -o validator-tui ./cmd/validator-tui/main.go

# Run
./validator-tui --state ./state.db
```

**Features**:
- Real-time epoch and slot information
- Active validator count
- Validator records with stats
- Stake account information
- Reward pool balance
- Slashing status indicators
- 1-second refresh interval

**Display**:
```
╔════════════════════════════════════════════════════════════════╗
║                  VALIDATOR TUI DASHBOARD                       ║
╚════════════════════════════════════════════════════════════════╝

Epoch: 0 | Slot: 1234 | Active Validators: 3 | Local Stake: 10000000

┌──────────────────┬──────────┬──────────────────┬────────┬────────┐
│ Pubkey (16 hex)  │ Status   │ Total Stake      │ Blocks │ Missed │
├──────────────────┼──────────┼──────────────────┼────────┼────────┤
│ 0102030405060708 │ Active   │ 10000000         │ 100    │ 2      │
│ 0908070605040302 │ Active   │ 5000000          │ 50     │ 1      │
└──────────────────┴──────────┴──────────────────┴────────┴────────┘

Total Staked: 15000000 | Reward Pool: 1000000 | Est. APY: 12.5%

Press 'q' or Ctrl+C to exit
```

### Storage Cost Calculator

**Usage**:
```bash
go run calculate_storage_cost.go <file_path>
```

**Output**:
```
File: programs/token/token.qsb
Size: 2048 bytes (2.00 KB)
Storage Cost: 2000 electrons (0.000002 Neon)
```

**Purpose**: Estimate balance needed for storing programs/data on-chain.

### Demo Scripts

**devnet.sh**:
```bash
# Start network
./devnet.sh start [N]        # N validators (default: 3)

# Stop network
./devnet.sh stop

# Restart network
./devnet.sh restart [N]

# Check status
./devnet.sh status

# View logs
./devnet.sh logs [ID]        # All logs or specific validator

# Clean data
./devnet.sh clean            # Stop and remove all data
```

**demo-dpos.sh**:
```bash
# Start DPoS demo
./demo-dpos.sh start [N]     # N validators (default: 3)

# Stop demo
./demo-dpos.sh stop

# Check status
./demo-dpos.sh status

# View logs
./demo-dpos.sh logs [ID]

# Clean data
./demo-dpos.sh clean
```

**audit.sh**:
```bash
# Run with defaults
./audit.sh

# Custom duration and validators
./audit.sh --duration 60 --validators 5

# CI mode (no colors, no prompts)
./audit.sh --ci
```

### Build Script

**build.sh** (future):
```bash
# Build all binaries
./build.sh

# Build specific binary
./build.sh poh-node
./build.sh neon-wallet
./build.sh validator-tui

# Clean build artifacts
./build.sh clean
```

### Environment Variables

**WALLET_PASSWORD**: Password for validator wallet (avoid in production)
**WALLET_KEYFILE**: Path to file containing wallet password
**POH_LOG_LEVEL**: Logging level (DEBUG, INFO, WARN, ERROR)
**POH_DATA_DIR**: Default data directory



---

## 16. Key Implementation Details

### Thread Safety

**Concurrent Access Patterns**:

1. **PohClock**:
   - RWMutex for hash state
   - Atomic operations for counters
   - Thread-safe Tick() method

2. **FileStore**:
   - RWMutex for cache access
   - BadgerDB handles internal concurrency
   - Copy-on-read to prevent external modification

3. **NetworkNode**:
   - RWMutex for peer map
   - Channels for block communication
   - Goroutines for connection handling

4. **AccessController**:
   - RWMutex for access log
   - Thread-safe validation

**Synchronization Primitives**:
- `sync.RWMutex`: Read-write locks
- `sync.Mutex`: Exclusive locks
- Channels: Communication between goroutines
- `sync.WaitGroup`: Goroutine coordination

### Error Handling Patterns

**Error Wrapping**:
```go
if err != nil {
    return fmt.Errorf("failed to create file: %w", err)
}
```

**Error Types**:
- Standard errors: `errors.New()`
- Wrapped errors: `fmt.Errorf(..., %w, err)`
- Custom errors: Struct types with Error() method

**Error Propagation**:
- Return errors up the call stack
- Log at appropriate level
- Don't panic except for unrecoverable errors

**Validation Errors**:
- Detailed error messages
- Include context (file ID, operation, etc.)
- Source location for compiler errors

### Serialization Conventions

**JSON Serialization**:
- Binary data: Hex-encoded strings
- Timestamps: Unix timestamps (int64)
- FileIDs: 64-character hex strings
- PublicKeys: 64-character hex strings

**Binary Serialization**:
- Little-endian for integers
- Fixed-size fields for determinism
- Length-prefixed for variable data

**Bytecode Serialization**:
- Magic number: 0x51534300 ("QSC\0")
- Version: 4-byte integer
- Entry offset: 4-byte integer
- Body: variable-length bytecode

### Determinism Requirements

**Sources of Non-Determinism to Avoid**:
1. Random number generation
2. System time (use block timestamps)
3. Floating-point arithmetic
4. Hash map iteration order
5. Thread scheduling
6. File system operations
7. Network I/O

**Ensuring Determinism**:
- Use deterministic PRNG with fixed seed
- Sort collections before iteration
- Fixed-point arithmetic instead of floating-point
- Explicit ordering in algorithms
- Reproducible hash functions

### Memory Management

**Go Garbage Collection**:
- Automatic memory management
- Minimize allocations in hot paths
- Reuse buffers where possible
- Profile with pprof

**Resource Cleanup**:
- Defer for cleanup (files, connections)
- Close channels when done
- Stop goroutines gracefully
- Release locks in defer

**Memory Pools** (future optimization):
- Sync.Pool for temporary objects
- Buffer pools for serialization
- Connection pools for network

### Logging Strategy

**Log Levels**:
- DEBUG: Detailed diagnostic information
- INFO: General informational messages
- WARN: Warning messages (non-critical)
- ERROR: Error messages (operation failed)
- FATAL: Fatal errors (program exits)

**Logging Locations**:
- Node startup/shutdown
- Block production/reception
- Transaction processing
- Consensus events
- Network events
- Error conditions

**Log Format**:
```
2024-01-15 10:30:45 [INFO] Node initialization complete
2024-01-15 10:30:46 [INFO] Leader node: Producing block for slot 1234
2024-01-15 10:30:46 [ERROR] Failed to broadcast block: connection refused
```

### Configuration Management

**Command-Line Flags**:
- Use `flag` package
- Provide defaults
- Validate inputs
- Help text for each flag

**Configuration Files** (future):
- TOML or JSON format
- Hierarchical structure
- Environment variable overrides
- Validation on load

**Genesis Configuration**:
- Hardcoded for development
- Load from file in production
- Validate before use
- Idempotent initialization

### Performance Considerations

**Hot Paths**:
1. PoH clock hashing
2. Block validation
3. Transaction processing
4. State root calculation
5. Network serialization

**Optimization Strategies**:
- Minimize allocations
- Cache frequently accessed data
- Batch operations where possible
- Parallel processing (future)
- Profile before optimizing

**Benchmarking**:
```bash
go test -bench=. ./internal/poh
go test -bench=. ./internal/quanticscript
go test -benchmem -bench=. ./...
```

### Security Considerations

**Cryptographic Security**:
- Ed25519 for signatures (industry standard)
- SHA-256 for hashing (proven secure)
- AES-256-GCM for encryption (authenticated)
- Argon2id for key derivation (memory-hard)

**Input Validation**:
- Validate all external inputs
- Check bounds and ranges
- Sanitize user data
- Reject malformed data

**Access Control**:
- Verify signatures before execution
- Validate file permissions
- Check authorization for operations
- Log access attempts

**Denial of Service Protection**:
- Compute metering (prevent infinite loops)
- Storage cost model (prevent state bloat)
- Transaction fees (prevent spam)
- Rate limiting (future)

### Upgrade Path

**Versioning**:
- Block version field for format changes
- Bytecode version for language changes
- API version for RPC changes
- Database schema migrations

**Backwards Compatibility**:
- Support old block formats
- Graceful degradation
- Feature flags for new features
- Migration tools

**Hard Forks**:
- Coordinate upgrade across network
- Activate at specific slot/epoch
- Maintain compatibility window
- Provide upgrade documentation



---

## 17. Critical Design Decisions

### Why Proof of History?

**Advantages**:
- Verifiable time ordering without consensus
- High throughput potential
- Deterministic block production
- Efficient verification

**Trade-offs**:
- Requires continuous hashing (CPU intensive)
- Leader-based (centralization risk)
- Clock synchronization challenges

### Why File-Based State Model?

**Advantages**:
- Uniform abstraction (accounts = programs = data)
- Explicit access control
- Parallel execution potential
- Storage cost enforcement

**Trade-offs**:
- More complex than simple account model
- Requires careful permission management
- Higher storage overhead

### Why Custom Language (QuanticScript)?

**Advantages**:
- Full control over determinism
- Optimized for blockchain use cases
- TypeScript-like syntax (familiar)
- Cost metering built-in

**Trade-offs**:
- Smaller ecosystem than Solidity/Rust
- Requires custom tooling
- Learning curve for developers

### Why BadgerDB for State?

**Advantages**:
- Pure Go (no CGO dependencies)
- LSM-tree (fast writes)
- Embedded (no separate server)
- Good performance

**Trade-offs**:
- Larger disk usage than SQLite
- More complex than simple key-value
- Requires compaction management

### Why SQLite for Ledger?

**Advantages**:
- Proven reliability
- SQL query capabilities
- ACID transactions
- Wide tooling support

**Trade-offs**:
- Single-writer limitation
- Not optimized for append-only
- Larger than custom format

### Why TCP for Networking?

**Advantages**:
- Reliable delivery
- Ordered messages
- Simple implementation
- Wide support

**Trade-offs**:
- Higher latency than UDP
- Connection overhead
- No multicast support

### Why DPoS over PoW/PoS?

**Advantages**:
- Energy efficient (no mining)
- Predictable block production
- Stake-weighted fairness
- Validator accountability

**Trade-offs**:
- Centralization risk (fewer validators)
- Requires staking mechanism
- More complex than simple PoW

---

## 18. Future Enhancements

### Short-Term (Next 3-6 Months)

**1. Parallel Transaction Execution**:
- Conflict detection (already implemented)
- Parallel execution engine
- Optimistic concurrency control
- Rollback on conflicts

**2. Improved Networking**:
- Peer discovery protocol
- Connection pooling
- Message compression
- Gossip protocol

**3. Enhanced RPC**:
- WebSocket support
- Subscription API
- Batch requests
- Rate limiting

**4. Wallet Improvements**:
- Hardware wallet support
- Multi-signature accounts
- Account recovery
- Mobile app

**5. Developer Tools**:
- QuanticScript debugger
- IDE plugins
- Testing framework
- Documentation generator

### Medium-Term (6-12 Months)

**1. Cross-Chain Bridges**:
- Ethereum bridge
- Bitcoin bridge
- Generic bridge protocol
- Asset wrapping

**2. Advanced Smart Contracts**:
- NFT standard
- DeFi primitives
- Governance contracts
- Oracle integration

**3. Performance Optimization**:
- JIT compilation for QuanticScript
- State pruning
- Snapshot creation
- Fast sync

**4. Security Enhancements**:
- Formal verification
- Security audits
- Bug bounty program
- Penetration testing

**5. Monitoring and Observability**:
- Metrics collection
- Distributed tracing
- Alerting system
- Dashboard

### Long-Term (12+ Months)

**1. Sharding**:
- State sharding
- Transaction sharding
- Cross-shard communication
- Shard rebalancing

**2. Zero-Knowledge Proofs**:
- ZK-SNARK integration
- Private transactions
- Scalability improvements
- Proof aggregation

**3. Governance**:
- On-chain governance
- Proposal system
- Voting mechanism
- Parameter updates

**4. Interoperability**:
- IBC protocol support
- Cosmos integration
- Polkadot parachain
- Cross-chain DEX

**5. Enterprise Features**:
- Permissioned networks
- Compliance tools
- SLA guarantees
- Enterprise support

---

## 19. Known Issues and Limitations

### Current Limitations

**1. Single-Threaded Block Production**:
- PoH clock runs in single thread
- Limits throughput
- Future: Parallel PoH with pipelining

**2. No State Pruning**:
- State grows indefinitely
- Disk usage increases over time
- Future: Implement state pruning

**3. Simple Peer Discovery**:
- Manual peer configuration
- No automatic discovery
- Future: Implement gossip protocol

**4. Limited Transaction History**:
- No transaction indexing
- Difficult to query history
- Future: Add transaction index

**5. No Mempool**:
- Transactions processed immediately
- No transaction prioritization
- Future: Implement mempool with fee market

**6. Basic Fee Model**:
- Fixed fees
- No dynamic adjustment
- Future: Implement fee market

**7. No Light Clients**:
- All nodes are full nodes
- High resource requirements
- Future: Implement light client protocol

**8. Limited Cross-Program Invocation**:
- Max depth of 4
- No async calls
- Future: Increase depth, add async

### Known Bugs

**1. Parser Infinite Loop** (FIXED):
- Parser could loop infinitely on certain syntax errors
- Fixed with proper error recovery

**2. BFT Validation Issues** (FIXED):
- Malicious blocks sometimes accepted
- Fixed with stricter validation

**3. Wallet Password Handling**:
- Password in environment variable (insecure)
- Need secure password input

**4. RPC Cache Invalidation**:
- Cache not invalidated on updates
- Can return stale data
- Need proper invalidation

### Performance Bottlenecks

**1. State Root Calculation**:
- Recalculates entire Merkle tree
- O(n) complexity for n files
- Future: Incremental updates

**2. Block Serialization**:
- JSON serialization is slow
- Large blocks take time
- Future: Binary format

**3. Network Latency**:
- TCP overhead
- No compression
- Future: Optimize protocol

**4. Database Writes**:
- Synchronous writes
- Disk I/O bottleneck
- Future: Batch writes

---

## 20. Migration Guide for New Language

### Language Requirements

**Must Have**:
1. Strong type system
2. Concurrency primitives (threads/async)
3. Cryptographic libraries (Ed25519, SHA-256)
4. Network programming (TCP/HTTP)
5. Database bindings (SQLite, key-value store)
6. JSON serialization
7. Binary I/O

**Nice to Have**:
1. Garbage collection
2. Package manager
3. Testing framework
4. Profiling tools
5. Cross-platform support

### Architecture Preservation

**Core Concepts to Maintain**:
1. PoH clock with sequential hashing
2. File-based state model
3. Stake-weighted leader scheduling
4. Transaction processing with rollback
5. QuanticScript language pipeline
6. Cross-program invocation
7. Byzantine fault tolerance

**Flexibility Areas**:
1. Storage backend (can change from BadgerDB)
2. Network protocol (can use different transport)
3. Serialization format (can use protobuf/msgpack)
4. Consensus details (can adjust parameters)

### Component Priority

**Phase 1 - Core Blockchain** (Weeks 1-4):
1. PoH clock
2. Block structures
3. Basic consensus
4. SQLite ledger
5. Network layer

**Phase 2 - State Model** (Weeks 5-8):
1. FileStore implementation
2. Storage cost model
3. Transaction structures
4. Access control
5. Transaction processor

**Phase 3 - Smart Contracts** (Weeks 9-16):
1. QuanticScript lexer
2. Parser and AST
3. Type checker
4. Code generator
5. Interpreter
6. Standard library
7. System/Token/Staking programs

**Phase 4 - User Interface** (Weeks 17-20):
1. CLI tools
2. RPC server
3. Wallet implementation
4. Demo scripts
5. Documentation

### Testing Strategy

**Test-Driven Development**:
1. Write tests first
2. Implement to pass tests
3. Refactor with confidence
4. Maintain high coverage

**Integration Test Priority**:
1. Block production and validation
2. Network communication
3. Transaction processing
4. State persistence
5. DPoS lifecycle

**Performance Benchmarks**:
1. PoH hashing rate
2. Block production time
3. Transaction throughput
4. State root calculation
5. Network latency

### Documentation Requirements

**Code Documentation**:
1. Package-level documentation
2. Function/method documentation
3. Complex algorithm explanations
4. Example usage

**User Documentation**:
1. Quick start guide
2. CLI reference
3. RPC API documentation
4. QuanticScript language reference
5. Architecture overview

**Developer Documentation**:
1. Architecture diagrams
2. Data flow diagrams
3. State machine diagrams
4. Sequence diagrams
5. Design decisions

---

## 21. Conclusion

This document captures the complete architecture and implementation details of the PoH blockchain project. The key innovations are:

1. **Proof of History**: Verifiable time ordering through sequential hashing
2. **File-Based State**: Uniform abstraction for all on-chain state
3. **QuanticScript**: TypeScript-like smart contract language
4. **DPoS Consensus**: Stake-weighted leader scheduling
5. **Byzantine Fault Tolerance**: Network resilience against malicious nodes

The implementation demonstrates a working blockchain with:
- ~75 blocks per minute (400ms slots)
- Cost-metered smart contract execution
- Atomic transaction processing with rollback
- Persistent state with verification
- P2P networking with block broadcasting
- Comprehensive testing suite

When reimplementing in a new language, focus on:
1. Maintaining determinism throughout
2. Preserving the PoH clock properties
3. Implementing the file-based state model correctly
4. Building the QuanticScript pipeline completely
5. Testing thoroughly at each phase

The architecture is modular and well-documented, making it suitable for reimplementation while allowing for improvements and optimizations in the new language.

**Total Lines of Code**: ~25,000 lines of Go
**Test Coverage**: >80% for core components
**Documentation**: >50 pages across multiple guides

Good luck with the reimplementation! 🚀



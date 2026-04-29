# StellAIverse Contracts

Core Soroban smart contracts for the StellAIverse AI agent platform on Stellar.

## Project Structure

```
contracts/
├── agent-nft/          # Agent NFT/Token contract - Mint agents as NFTs with metadata
├── execution-hub/      # Rules engine for agent actions and execution
├── marketplace/        # Buy/sell/lease agents with royalty splits
├── evolution/          # Stake-based agent training and upgrade system
├── oracle/            # Oracle integration for real-world data feeds
└── faucet/            # Test agent distribution for testnet

shared/
└── src/lib.rs         # Common types and utilities shared across contracts
```

## Contracts

### Agent NFT (`agent-nft`)
Manages agent creation and ownership via NFT/custom assets.

**Key Functions:**
- `mint_agent()` - Create a new agent NFT
- `get_agent()` - Retrieve agent metadata
- `update_agent()` - Update agent properties
- `total_agents()` - Get total minted count

### Execution Hub (`execution-hub`)
On-chain rules engine for managing agent execution policies.

**Key Functions:**
- `register_rule()` - Define execution rules for an agent
- `execute_action()` - Execute an agent action (validated against rules)
- `get_history()` - Retrieve execution history
- `revoke_rule()` - Remove a rule

### Marketplace (`marketplace`)
Platform for trading and leasing agents with royalty support.

shared/
└── src/lib.rs         # Common types and utilities shared across contracts

**Key Functions:**
- `create_listing()` - List an agent for sale, lease, or auction
- `buy_agent()` - Purchase or lease an agent
- `cancel_listing()` - Delist an agent
- `get_listings()` - Browse active listings
- `set_royalty()` - Configure royalty splits

### Evolution System (`evolution`)
Token-stake-based mechanism for upgrading agent intelligence.

**Key Functions:**
- `request_upgrade()` - Initiate an upgrade (stakes tokens)
- `complete_upgrade()` - Finalize upgrade with new model hash
- `get_upgrade_history()` - View upgrade history
- `claim_stake()` - Recover staked tokens after upgrade
- `get_evolution_level()` - Check current evolution level

### Oracle (`oracle`)
Feed real-world data (prices, news, market data) to agents.

**Key Functions:**
- `register_provider()` - Authorize a data provider
- `submit_data()` - Post oracle data
- `get_data()` - Retrieve latest data
- `get_history()` - Get historical data
- `is_data_fresh()` - Verify data freshness

### Faucet (`faucet`)
Testnet utility for distributing test agents.

**Key Functions:**
- `claim_test_agent()` - Claim a test agent (testnet only)
- `is_eligible()` - Check claim eligibility
- `set_parameters()` - Configure faucet rules

## Setup

### Prerequisites
- Rust 1.70+
- Soroban CLI

### Installation

1. **Install Rust** (if not already installed):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **Install Soroban CLI**:
   ```bash
   cargo install --locked soroban-cli
   ```

3. **Build all contracts**:
   ```bash
   cargo build --release
   ```

4. **Build a specific contract**:
   ```bash
   cargo build --release -p agent-nft
   ```

## Development

### Testing
```bash
cargo test
```

### Running Tests for Specific Contract
```bash
cargo test -p agent-nft
```

### Building WASM
Contracts are built as WASM for deployment:
```bash
cargo build --release --target wasm32-unknown-unknown
```

## Deployment

### Testnet
```bash
soroban contract deploy \
  --wasm target/release/agent_nft.wasm \
  --network testnet
```

### Mainnet
Follow the same pattern but use `--network public`.

## Key Design Decisions

1. **Modular Architecture**: Each contract is independent but can interact via cross-contract calls.
2. **Shared Types**: Common data structures defined in `shared/` crate for consistency.
3. **Oracle Integration**: Evolution system uses oracle for off-chain AI training coordination.
4. **Royalty System**: Marketplace supports creator royalties on secondary sales.
5. **Rate Limiting**: Faucet includes cooldowns to prevent abuse.

## Event Emission

All contracts emit events for indexing and UI updates:
- `AgentMinted` - New agent created
- `ActionExecuted` - Agent action completed
- `ListingCreated` / `ListingCancelled` - Marketplace events
- `UpgradeRequested` / `UpgradeCompleted` - Evolution events
- `DataSubmitted` - Oracle data posted
- `AgentClaimed` - Faucet claim

## Future Enhancements

- Multi-sig governance for critical functions
- Agent lending pools
- Composable agent capabilities
- Advanced ML reward mechanisms
- Cross-chain interoperability

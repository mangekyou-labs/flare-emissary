# FlareEmissary

> Real-time event indexing, alert matching, and multi-channel notifications for Flare Network's enshrined protocols.

FlareEmissary is a permissionless push-notification layer for Flare Network. It indexes FTSO v2, FDC, and FAsset events in real-time, matches them against user-defined subscriptions, and delivers alerts via Telegram, Discord, and Email.

## Architecture

```
Flare RPC → Block Poller → Reorg Detector → Decoder Registry
                                               ├── FTSO v2 Decoder
                                               ├── FDC Decoder
                                               ├── FAsset Decoder
                                               └── Generic Decoder (opt-in)
                                                        ↓
                                              Event Persistence (PostgreSQL/TimescaleDB)
                                                        ↓
                        Alert Matcher → Hysteresis Engine → Redis Queue → Notification Workers
```

## Project Structure

```
flare-emissary/
├── crates/
│   ├── common/        # Shared types, config, DB/Redis pool, error handling
│   ├── decoders/      # Protocol-specific event decoders (FTSO, FDC, FAsset, Generic)
│   ├── indexer/       # Block poller, reorg detection, event persistence (binary)
│   ├── engine/        # Alert matching, hysteresis, CR calculator (stub)
│   ├── api/           # Axum REST API with SIWE auth (stub)
│   └── notifier/      # Telegram, Discord, Email delivery workers (stub)
├── migrations/        # SQL migrations (sqlx)
├── .env.example       # Environment variable documentation
└── Cargo.toml         # Workspace root
```

## Prerequisites

- **Rust** ≥ 1.85 (edition 2024)
- **PostgreSQL** ≥ 14 (with optional [TimescaleDB](https://www.timescale.com/) for hypertables)
- **Redis** ≥ 7
- **Flare RPC endpoint** — public or [NaaS provider](https://docs.flare.network/dev/reference/network-configs/) (recommended for production)

## Setup

```bash
# Clone
git clone https://github.com/mangekyou-labs/flare-emissary.git
cd flare-emissary

# Configure environment
cp .env.example .env
# Edit .env with your DATABASE_URL, FLARE_RPC_URL, JWT_SECRET, etc.

# Create database
createdb flare_emissary

# Build
cargo build

# Run the indexer (applies migrations automatically on startup)
cargo run --bin flare-indexer
```

## Configuration

All configuration is via environment variables (or `.env` file):

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `DATABASE_URL` | ✅ | — | PostgreSQL connection string |
| `REDIS_URL` | | `redis://localhost:6379` | Redis connection string |
| `FLARE_RPC_URL` | | Public Flare RPC | Primary RPC endpoint (NaaS recommended) |
| `FLARE_RPC_FALLBACK_URL` | | — | Fallback RPC endpoint |
| `INDEXER_POLL_INTERVAL_MS` | | `1500` | Block polling interval in ms |
| `INDEXER_REORG_WINDOW` | | `10` | Number of recent blocks tracked for reorg detection |
| `JWT_SECRET` | ✅ | — | Secret for JWT token signing |
| `JWT_EXPIRY_HOURS` | | `24` | JWT token lifetime |
| `TELEGRAM_BOT_TOKEN` | | — | Telegram bot token for notifications |
| `DISCORD_BOT_TOKEN` | | — | Discord bot token |
| `RESEND_API_KEY` | | — | Resend API key for email delivery |
| `EMAIL_FROM` | | — | Sender address for email notifications |

## Decoded Events

### FTSO v2
- `PriceEpochFinalized` — A price epoch has been finalized
- `VotePowerChanged` — A provider's vote power changed
- `RewardEpochStarted` — A new reward epoch started

### Flare Data Connector (FDC)
- `AttestationRequested` — Cross-chain attestation requested
- `AttestationProved` — Attestation proof submitted
- `RoundFinalized` — Attestation round finalized

### FAssets
- `CollateralDeposited` / `CollateralWithdrawn` — Agent vault collateral changes
- `MintingExecuted` — FAsset minting completed
- `RedemptionRequested` — FAsset redemption initiated
- `LiquidationStarted` — Agent liquidation triggered

## Development

```bash
# Check all crates
cargo check

# Run unit tests (no DB required)
cargo test --workspace

# Run integration tests (requires PostgreSQL)
# Tests that hit the DB are `#[ignored]` by default
cargo test --test integration -- --ignored --nocapture

# Run pipeline benchmark (live RPC latency test)
cargo run --bin benchmark
# Custom settings:
# FLARE_RPC_URL="..." BENCHMARK_BLOCKS=100 cargo run --bin benchmark

# Lint
cargo clippy --all-targets

# Format
cargo fmt
```

## Roadmap

| Milestone | Status | Description |
|-----------|--------|-------------|
| **M1** Indexer | 🟡 ~85% | Block polling, reorg detection, event decoding, persistence |
| **M2** Backend Logic | ⬜ | Alert matching, hysteresis, subscription CRUD, auth |
| **M3** Frontend | ⬜ | React dashboard — address search, event discovery, alert config |
| **M4** FAsset Health | ⬜ | Real-time CR calculator, vault health dashboard |
| **M5** Delivery | ⬜ | Telegram, Discord, Email notification workers |
| **M6** Launch & SDK | ⬜ | TypeScript SDK, Docker, production deployment |

## License

MIT

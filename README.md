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
│   ├── engine/        # Alert matching, hysteresis, cooldown, subscriptions, address analyzer
│   ├── api/           # Axum REST API with SIWE auth, JWT middleware (binary)
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

## Quick Start

### Option A: Docker (recommended for development)

```bash
# Clone
git clone https://github.com/mangekyou-labs/flare-emissary.git
cd flare-emissary

# Start PostgreSQL + Redis via Docker
docker run -d --name flare-pg \
  -e POSTGRES_USER=flare \
  -e POSTGRES_PASSWORD=flare \
  -e POSTGRES_DB=flare_emissary \
  -p 5432:5432 \
  postgres:16-alpine

docker run -d --name flare-redis -p 6379:6379 redis:7-alpine

# Configure environment
cp .env.example .env
# The defaults in .env.example match the Docker containers above.
# Edit JWT_SECRET and FLARE_RPC_URL as needed.

# Build
cargo build --workspace

# Run the indexer (applies migrations automatically on startup)
cargo run --bin flare-indexer

# Run the API server (port 3000)
cargo run --bin flare-api
```

### Option B: Local PostgreSQL

```bash
# Create database (assumes PostgreSQL is already running)
createdb flare_emissary

# Configure
cp .env.example .env
# Set DATABASE_URL to your local Postgres connection string

cargo build --workspace
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
cargo check --workspace

# Run unit tests (no DB required — 67 tests)
cargo test --workspace

# Run integration tests (requires PostgreSQL + Redis — see Quick Start above)
# Integration tests are #[ignored] by default to avoid requiring a DB for CI.
# Make sure DATABASE_URL is set or pass it inline:

# All integration tests at once (21 tests across 3 crates):
DATABASE_URL="postgres://flare:flare@localhost:5432/flare_emissary" \
  cargo test --workspace -- --ignored --nocapture

# Or run by crate:
# Indexer (4 tests): event persistence, dedup, state, reorg
DATABASE_URL="postgres://flare:flare@localhost:5432/flare_emissary" \
  cargo test -p flare-indexer --test integration -- --ignored --nocapture

# Engine (12 tests): subscription CRUD, alert matcher, address analyzer
DATABASE_URL="postgres://flare:flare@localhost:5432/flare_emissary" \
  cargo test -p flare-engine --test integration -- --ignored --nocapture

# API (5 tests): health, subscription routes, auth, address analysis
DATABASE_URL="postgres://flare:flare@localhost:5432/flare_emissary" \
  cargo test -p flare-api --test integration -- --ignored --nocapture

# Run pipeline benchmark (live RPC latency test)
cargo run --bin benchmark
# Custom settings:
# FLARE_RPC_URL="..." BENCHMARK_BLOCKS=100 cargo run --bin benchmark

# Lint (0 warnings required)
cargo clippy --workspace -- -D warnings

# Format
cargo fmt --all
```

### Stopping Docker containers

```bash
docker stop flare-pg flare-redis
docker rm flare-pg flare-redis
```

## Roadmap

| Milestone | Status | Description |
|-----------|--------|-------------|
| **M1** Indexer | ✅ Done | Block polling, reorg detection, event decoding, persistence |
| **M2** Backend Logic | ✅ Done | Alert matching, hysteresis, cooldown, subscription CRUD, JWT auth, API |
| **M3** Frontend | ⬜ | React dashboard — address search, event discovery, alert config |
| **M4** FAsset Health | ⬜ | Real-time CR calculator, vault health dashboard |
| **M5** Delivery | ⬜ | Telegram, Discord, Email notification workers |
| **M6** Launch & SDK | ⬜ | TypeScript SDK, Docker, production deployment |
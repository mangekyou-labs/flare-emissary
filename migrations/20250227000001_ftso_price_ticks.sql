-- FlareEmissary Migration 002: FTSO price ticks hypertable
-- Stores every FTSO v2 price update per feed_id for CR calculation and price history.

CREATE TABLE IF NOT EXISTS ftso_price_ticks (
    id BIGSERIAL,
    feed_id TEXT NOT NULL,
    price NUMERIC NOT NULL,
    decimals SMALLINT NOT NULL DEFAULT 5,
    block_number BIGINT NOT NULL,
    block_timestamp TIMESTAMPTZ NOT NULL,
    chain TEXT NOT NULL DEFAULT 'flare',
    epoch_id BIGINT,
    tx_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Primary key uses (block_timestamp, id) for TimescaleDB chunk-aware indexing
ALTER TABLE ftso_price_ticks ADD PRIMARY KEY (block_timestamp, id);

-- Convert to TimescaleDB hypertable (uncomment when TimescaleDB is available)
-- SELECT create_hypertable('ftso_price_ticks', 'block_timestamp', migrate_data => true);

-- Indexes for common query patterns
CREATE INDEX idx_price_ticks_feed ON ftso_price_ticks(feed_id);
CREATE INDEX idx_price_ticks_feed_time ON ftso_price_ticks(feed_id, block_timestamp DESC);
CREATE INDEX idx_price_ticks_block ON ftso_price_ticks(block_number);
CREATE INDEX idx_price_ticks_epoch ON ftso_price_ticks(epoch_id) WHERE epoch_id IS NOT NULL;

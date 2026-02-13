-- FlareEmissary Database Schema
-- Migration 001: Initial schema setup with TimescaleDB

-- Enable TimescaleDB extension
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
-- CREATE EXTENSION IF NOT EXISTS timescaledb; -- Uncomment when TimescaleDB is available

-- ============================================================
-- Indexer state tracking
-- ============================================================
CREATE TABLE IF NOT EXISTS indexer_state (
    chain TEXT PRIMARY KEY,
    last_block BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ============================================================
-- Users (wallet-first via SIWE)
-- ============================================================
CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    wallet_address TEXT NOT NULL UNIQUE,
    email TEXT,
    api_key TEXT UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_users_wallet ON users(wallet_address);
CREATE INDEX idx_users_api_key ON users(api_key) WHERE api_key IS NOT NULL;

-- ============================================================
-- Monitored addresses
-- ============================================================
CREATE TABLE IF NOT EXISTS monitored_addresses (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    address TEXT NOT NULL,
    chain TEXT NOT NULL DEFAULT 'flare',
    address_type TEXT NOT NULL DEFAULT 'generic_contract',
    detected_events JSONB DEFAULT '[]'::jsonb,
    last_indexed_at TIMESTAMPTZ,
    UNIQUE(address, chain)
);

CREATE INDEX idx_monitored_address ON monitored_addresses(address);

-- ============================================================
-- Notification channels
-- ============================================================
CREATE TABLE IF NOT EXISTS notification_channels (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    channel_type TEXT NOT NULL, -- 'telegram' | 'discord' | 'email'
    config JSONB NOT NULL DEFAULT '{}'::jsonb,
    verified BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_channels_user ON notification_channels(user_id);

-- ============================================================
-- Subscriptions
-- ============================================================
CREATE TABLE IF NOT EXISTS subscriptions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    address_id UUID NOT NULL REFERENCES monitored_addresses(id) ON DELETE CASCADE,
    channel_id UUID NOT NULL REFERENCES notification_channels(id) ON DELETE CASCADE,
    event_type TEXT NOT NULL,
    threshold_config JSONB NOT NULL DEFAULT '{}'::jsonb,
    active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_subscriptions_user ON subscriptions(user_id);
CREATE INDEX idx_subscriptions_address ON subscriptions(address_id);
CREATE INDEX idx_subscriptions_active ON subscriptions(active) WHERE active = true;

-- ============================================================
-- Indexed events (TimescaleDB hypertable candidate)
-- ============================================================
CREATE TABLE IF NOT EXISTS indexed_events (
    id BIGSERIAL PRIMARY KEY,
    tx_hash TEXT NOT NULL,
    log_index BIGINT,
    block_number BIGINT NOT NULL,
    block_timestamp TIMESTAMPTZ NOT NULL,
    chain TEXT NOT NULL,
    address TEXT NOT NULL,
    event_type TEXT NOT NULL,
    decoded_data JSONB NOT NULL DEFAULT '{}'::jsonb,
    is_reorged BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Unique constraint includes log_index to handle multiple events of the same type in one tx
CREATE UNIQUE INDEX idx_events_unique ON indexed_events(tx_hash, log_index);
CREATE INDEX idx_events_block ON indexed_events(block_number);
CREATE INDEX idx_events_address ON indexed_events(address);
CREATE INDEX idx_events_type ON indexed_events(event_type);
CREATE INDEX idx_events_timestamp ON indexed_events(block_timestamp DESC);
CREATE INDEX idx_events_chain ON indexed_events(chain);

-- Convert to TimescaleDB hypertable (uncomment when TimescaleDB is available)
-- SELECT create_hypertable('indexed_events', 'block_timestamp', migrate_data => true);

-- ============================================================
-- Alerts
-- ============================================================
CREATE TABLE IF NOT EXISTS alerts (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    subscription_id UUID NOT NULL REFERENCES subscriptions(id) ON DELETE CASCADE,
    event_id BIGINT NOT NULL REFERENCES indexed_events(id),
    severity TEXT NOT NULL DEFAULT 'info', -- 'info' | 'warning' | 'critical'
    message TEXT NOT NULL,
    triggered_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_alerts_subscription ON alerts(subscription_id);
CREATE INDEX idx_alerts_triggered ON alerts(triggered_at DESC);

-- ============================================================
-- Notifications
-- ============================================================
CREATE TABLE IF NOT EXISTS notifications (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    alert_id UUID NOT NULL REFERENCES alerts(id) ON DELETE CASCADE,
    channel_id UUID NOT NULL REFERENCES notification_channels(id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'pending', -- 'pending' | 'sent' | 'failed'
    sent_at TIMESTAMPTZ,
    error_detail TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_notifications_status ON notifications(status) WHERE status = 'pending';
CREATE INDEX idx_notifications_alert ON notifications(alert_id);

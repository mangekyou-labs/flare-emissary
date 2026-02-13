use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Supported blockchain networks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
pub enum Chain {
    Flare,
    Songbird,
}

impl std::fmt::Display for Chain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Chain::Flare => write!(f, "flare"),
            Chain::Songbird => write!(f, "songbird"),
        }
    }
}

/// Classification of an on-chain address.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum AddressType {
    FtsoProvider,
    FassetAgent,
    GenericContract,
    Eoa,
}

/// Types of events that can be decoded from on-chain logs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum EventType {
    // FTSO events
    PriceEpochFinalized,
    VotePowerChanged,
    RewardEpochStarted,

    // FDC events
    AttestationRequested,
    AttestationProved,
    RoundFinalized,

    // FAsset events
    CollateralDeposited,
    CollateralWithdrawn,
    MintingExecuted,
    RedemptionRequested,
    LiquidationStarted,

    // Generic
    GenericEvent,
}

impl std::fmt::Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EventType::PriceEpochFinalized => write!(f, "price_epoch_finalized"),
            EventType::VotePowerChanged => write!(f, "vote_power_changed"),
            EventType::RewardEpochStarted => write!(f, "reward_epoch_started"),
            EventType::AttestationRequested => write!(f, "attestation_requested"),
            EventType::AttestationProved => write!(f, "attestation_proved"),
            EventType::RoundFinalized => write!(f, "round_finalized"),
            EventType::CollateralDeposited => write!(f, "collateral_deposited"),
            EventType::CollateralWithdrawn => write!(f, "collateral_withdrawn"),
            EventType::MintingExecuted => write!(f, "minting_executed"),
            EventType::RedemptionRequested => write!(f, "redemption_requested"),
            EventType::LiquidationStarted => write!(f, "liquidation_started"),
            EventType::GenericEvent => write!(f, "generic_event"),
        }
    }
}

/// Alert severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

/// Notification delivery status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
pub enum DeliveryStatus {
    Pending,
    Sent,
    Failed,
}

/// Notification channel type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
pub enum ChannelType {
    Telegram,
    Discord,
    Email,
}

/// A decoded on-chain event ready for persistence and alert matching.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecodedEvent {
    pub tx_hash: String,
    pub log_index: Option<u64>,
    pub block_number: u64,
    pub block_timestamp: DateTime<Utc>,
    pub chain: Chain,
    pub address: String,
    pub event_type: EventType,
    pub decoded_data: serde_json::Value,
}

/// A monitored blockchain address.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoredAddress {
    pub id: Uuid,
    pub address: String,
    pub chain: Chain,
    pub address_type: AddressType,
    pub detected_events: serde_json::Value,
    pub last_indexed_at: Option<DateTime<Utc>>,
}

/// A user in the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub wallet_address: String,
    pub email: Option<String>,
    pub api_key: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A user's alert subscription.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub id: Uuid,
    pub user_id: Uuid,
    pub address_id: Uuid,
    pub channel_id: Uuid,
    pub event_type: EventType,
    pub threshold_config: serde_json::Value,
    pub active: bool,
    pub created_at: DateTime<Utc>,
}

/// A user's configured notification channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationChannel {
    pub id: Uuid,
    pub user_id: Uuid,
    pub channel_type: ChannelType,
    pub config: serde_json::Value,
    pub verified: bool,
    pub created_at: DateTime<Utc>,
}

/// A triggered alert.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub id: Uuid,
    pub subscription_id: Uuid,
    pub event_id: i64,
    pub severity: Severity,
    pub message: String,
    pub triggered_at: DateTime<Utc>,
}

/// A notification queued for delivery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub id: Uuid,
    pub alert_id: Uuid,
    pub channel_id: Uuid,
    pub status: DeliveryStatus,
    pub sent_at: Option<DateTime<Utc>>,
    pub error_detail: Option<String>,
    pub created_at: DateTime<Utc>,
}

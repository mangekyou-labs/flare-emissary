//! Integration tests for all protocol decoders and DecoderRegistry routing.
//!
//! These tests construct realistic `alloy::primitives::Log` structs matching
//! on-chain EVM log layout and verify that every decoder correctly extracts
//! event types and decoded fields.

use alloy::primitives::{keccak256, Address, Bytes, Log, LogData, B256, U256};
use chrono::Utc;

use flare_common::types::{Chain, EventType};

use crate::fasset::FassetDecoder;
use crate::fdc::FdcDecoder;
use crate::ftso::FtsoDecoder;
use crate::generic::GenericDecoder;
use crate::{DecoderRegistry, EventDecoder};

// ───────────────────────────── helpers ──────────────────────────────

/// Build an `alloy::primitives::Log` for testing.
///
/// `topics` must include topic0 (the event signature hash) as the first element.
fn build_log(topics: Vec<B256>, data: Vec<u8>, address: Address) -> Log {
    Log {
        address,
        data: LogData::new(topics, Bytes::from(data)).expect("valid log data"),
    }
}

/// ABI-encode a `uint256` value into 32 bytes (big-endian, zero-padded).
fn encode_u256(val: u64) -> [u8; 32] {
    U256::from(val).to_be_bytes::<32>()
}

/// Left-pad an address into a 32-byte topic (EVM indexed address encoding).
fn address_to_topic(addr: Address) -> B256 {
    let mut bytes = [0u8; 32];
    bytes[12..32].copy_from_slice(addr.as_slice());
    B256::from(bytes)
}

const BLOCK_NUMBER: u64 = 42_000;
const CONTRACT: Address = Address::ZERO;

fn now() -> chrono::DateTime<Utc> {
    Utc::now()
}

// ═══════════════════════════════════════════════════════════════════
//  FTSO v2 Decoder
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_ftso_price_epoch_finalized() {
    let decoder = FtsoDecoder::new();
    let topic0 = keccak256("PriceEpochFinalized(uint256,uint256)");
    let epoch_id: u64 = 1234;

    let log = build_log(
        vec![topic0, B256::from(encode_u256(epoch_id))],
        encode_u256(1_700_000_000).to_vec(), // timestamp in data
        CONTRACT,
    );

    let event = decoder.decode(&log, BLOCK_NUMBER, now(), Chain::Flare).unwrap();
    assert_eq!(event.event_type, EventType::PriceEpochFinalized);
    assert_eq!(event.block_number, BLOCK_NUMBER);
    assert_eq!(event.decoded_data["epoch_id"], epoch_id);
}

#[test]
fn test_ftso_vote_power_changed() {
    let decoder = FtsoDecoder::new();
    let topic0 = keccak256("VotePowerChanged(address,uint256)");
    let provider_addr = Address::repeat_byte(0xAB);

    let log = build_log(
        vec![topic0, address_to_topic(provider_addr)],
        encode_u256(500_000).to_vec(), // newVotePower
        CONTRACT,
    );

    let event = decoder.decode(&log, BLOCK_NUMBER, now(), Chain::Flare).unwrap();
    assert_eq!(event.event_type, EventType::VotePowerChanged);

    // Provider address is lowercased hex with 0x prefix
    let provider_str = event.decoded_data["provider"].as_str().unwrap();
    assert!(provider_str.contains("abababab"), "expected provider address in decoded data, got {provider_str}");

    assert_eq!(event.decoded_data["new_vote_power"].as_str().unwrap(), "500000");
}

#[test]
fn test_ftso_reward_epoch_started() {
    let decoder = FtsoDecoder::new();
    let topic0 = keccak256("RewardEpochStarted(uint256,uint256)");
    let epoch_id: u64 = 99;

    let log = build_log(
        vec![topic0, B256::from(encode_u256(epoch_id))],
        vec![],
        CONTRACT,
    );

    let event = decoder.decode(&log, BLOCK_NUMBER, now(), Chain::Flare).unwrap();
    assert_eq!(event.event_type, EventType::RewardEpochStarted);
    assert_eq!(event.decoded_data["epoch_id"], epoch_id);
}

#[test]
fn test_ftso_unknown_topic_returns_none() {
    let decoder = FtsoDecoder::new();
    let unknown_topic = keccak256("SomeUnknownEvent(uint256)");

    let log = build_log(vec![unknown_topic], vec![], CONTRACT);
    assert!(decoder.decode(&log, BLOCK_NUMBER, now(), Chain::Flare).is_none());
}

// ═══════════════════════════════════════════════════════════════════
//  FDC Decoder
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_fdc_attestation_requested() {
    let decoder = FdcDecoder::new();
    let topic0 = keccak256("AttestationRequested(bytes32,address)");
    let request_id = B256::repeat_byte(0x11);
    let requester = Address::repeat_byte(0xCC);

    let log = build_log(
        vec![topic0, request_id, address_to_topic(requester)],
        vec![],
        CONTRACT,
    );

    let event = decoder.decode(&log, BLOCK_NUMBER, now(), Chain::Flare).unwrap();
    assert_eq!(event.event_type, EventType::AttestationRequested);

    let rid = event.decoded_data["request_id"].as_str().unwrap();
    assert!(rid.contains("1111"), "expected request_id to contain 1111, got {rid}");

    let req = event.decoded_data["requester"].as_str().unwrap();
    assert!(req.contains("cccccc"), "expected requester addr, got {req}");
}

#[test]
fn test_fdc_attestation_proved() {
    let decoder = FdcDecoder::new();
    let topic0 = keccak256("AttestationProved(bytes32,bytes32)");
    let request_id = B256::repeat_byte(0x22);
    let merkle_root = B256::repeat_byte(0x33);

    let log = build_log(
        vec![topic0, request_id, merkle_root],
        vec![],
        CONTRACT,
    );

    let event = decoder.decode(&log, BLOCK_NUMBER, now(), Chain::Flare).unwrap();
    assert_eq!(event.event_type, EventType::AttestationProved);
    assert!(event.decoded_data["request_id"].as_str().unwrap().contains("2222"));
    assert!(event.decoded_data["merkle_root"].as_str().unwrap().contains("3333"));
}

#[test]
fn test_fdc_round_finalized() {
    let decoder = FdcDecoder::new();
    let topic0 = keccak256("RoundFinalized(uint256,bytes32)");
    let round_id: u64 = 777;

    let log = build_log(
        vec![topic0, B256::from(encode_u256(round_id))],
        vec![],
        CONTRACT,
    );

    let event = decoder.decode(&log, BLOCK_NUMBER, now(), Chain::Flare).unwrap();
    assert_eq!(event.event_type, EventType::RoundFinalized);
    assert_eq!(event.decoded_data["round_id"], round_id);
}

// ═══════════════════════════════════════════════════════════════════
//  FAsset Decoder
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_fasset_collateral_deposited() {
    let decoder = FassetDecoder::new();
    let topic0 = keccak256("CollateralDeposited(address,uint256)");
    let agent = Address::repeat_byte(0xAA);

    let log = build_log(
        vec![topic0, address_to_topic(agent)],
        encode_u256(1_000_000).to_vec(),
        CONTRACT,
    );

    let event = decoder.decode(&log, BLOCK_NUMBER, now(), Chain::Flare).unwrap();
    assert_eq!(event.event_type, EventType::CollateralDeposited);
    assert!(event.decoded_data["agent"].as_str().unwrap().contains("aaaaaa"));
    assert_eq!(event.decoded_data["amount"].as_str().unwrap(), "1000000");
}

#[test]
fn test_fasset_collateral_withdrawn() {
    let decoder = FassetDecoder::new();
    let topic0 = keccak256("CollateralWithdrawn(address,uint256)");
    let agent = Address::repeat_byte(0xBB);

    let log = build_log(
        vec![topic0, address_to_topic(agent)],
        encode_u256(500_000).to_vec(),
        CONTRACT,
    );

    let event = decoder.decode(&log, BLOCK_NUMBER, now(), Chain::Flare).unwrap();
    assert_eq!(event.event_type, EventType::CollateralWithdrawn);
    assert!(event.decoded_data["agent"].as_str().unwrap().contains("bbbbbb"));
    assert_eq!(event.decoded_data["amount"].as_str().unwrap(), "500000");
}

#[test]
fn test_fasset_minting_executed() {
    let decoder = FassetDecoder::new();
    let topic0 = keccak256("MintingExecuted(address,address,uint256)");
    let minter = Address::repeat_byte(0x11);
    let agent = Address::repeat_byte(0x22);

    let log = build_log(
        vec![topic0, address_to_topic(minter), address_to_topic(agent)],
        encode_u256(10).to_vec(), // lots
        CONTRACT,
    );

    let event = decoder.decode(&log, BLOCK_NUMBER, now(), Chain::Flare).unwrap();
    assert_eq!(event.event_type, EventType::MintingExecuted);
    assert!(event.decoded_data["minter"].as_str().unwrap().contains("111111"));
    assert!(event.decoded_data["agent"].as_str().unwrap().contains("222222"));
    assert_eq!(event.decoded_data["lots"].as_str().unwrap(), "10");
}

#[test]
fn test_fasset_redemption_requested() {
    let decoder = FassetDecoder::new();
    let topic0 = keccak256("RedemptionRequested(address,address,uint256)");
    let redeemer = Address::repeat_byte(0x33);
    let agent = Address::repeat_byte(0x44);

    let log = build_log(
        vec![topic0, address_to_topic(redeemer), address_to_topic(agent)],
        encode_u256(5).to_vec(),
        CONTRACT,
    );

    let event = decoder.decode(&log, BLOCK_NUMBER, now(), Chain::Flare).unwrap();
    assert_eq!(event.event_type, EventType::RedemptionRequested);
    assert!(event.decoded_data["redeemer"].as_str().unwrap().contains("333333"));
    assert!(event.decoded_data["agent"].as_str().unwrap().contains("444444"));
    assert_eq!(event.decoded_data["lots"].as_str().unwrap(), "5");
}

#[test]
fn test_fasset_liquidation_started() {
    let decoder = FassetDecoder::new();
    let topic0 = keccak256("LiquidationStarted(address,uint256)");
    let agent = Address::repeat_byte(0x55);

    let log = build_log(
        vec![topic0, address_to_topic(agent)],
        vec![],
        CONTRACT,
    );

    let event = decoder.decode(&log, BLOCK_NUMBER, now(), Chain::Flare).unwrap();
    assert_eq!(event.event_type, EventType::LiquidationStarted);
    assert!(event.decoded_data["agent"].as_str().unwrap().contains("555555"));
}

// ═══════════════════════════════════════════════════════════════════
//  Generic Decoder
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_generic_captures_any_event() {
    let decoder = GenericDecoder::new();
    let random_topic = keccak256("SomeRandomEvent(uint256,address)");

    let log = build_log(
        vec![random_topic],
        encode_u256(42).to_vec(),
        CONTRACT,
    );

    let event = decoder.decode(&log, BLOCK_NUMBER, now(), Chain::Flare).unwrap();
    assert_eq!(event.event_type, EventType::GenericEvent);
    assert!(event.decoded_data["topic0"].as_str().is_some());
    assert!(event.decoded_data["data"].as_str().is_some());
}

#[test]
fn test_generic_ignores_empty_topics() {
    let decoder = GenericDecoder::new();
    // Log with no topics — most decoders require at least topic0
    let log = build_log(vec![], vec![], CONTRACT);
    assert!(decoder.decode(&log, BLOCK_NUMBER, now(), Chain::Flare).is_none());
}

// ═══════════════════════════════════════════════════════════════════
//  Robustness — empty / malformed data
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_empty_log_data_does_not_panic() {
    let ftso = FtsoDecoder::new();
    let fdc = FdcDecoder::new();
    let fasset = FassetDecoder::new();
    let generic = GenericDecoder::new();

    // Valid topic0 but completely empty data
    let ftso_topic = keccak256("PriceEpochFinalized(uint256,uint256)");
    let log = build_log(vec![ftso_topic], vec![], CONTRACT);
    // Should not panic — may return Some with None fields or Some with defaults
    let _ = ftso.decode(&log, BLOCK_NUMBER, now(), Chain::Flare);

    let fdc_topic = keccak256("AttestationRequested(bytes32,address)");
    let log = build_log(vec![fdc_topic], vec![], CONTRACT);
    let _ = fdc.decode(&log, BLOCK_NUMBER, now(), Chain::Flare);

    let fasset_topic = keccak256("CollateralDeposited(address,uint256)");
    let log = build_log(vec![fasset_topic], vec![], CONTRACT);
    let _ = fasset.decode(&log, BLOCK_NUMBER, now(), Chain::Flare);

    // Generic should still work with minimal data
    let random_topic = keccak256("Anything()");
    let log = build_log(vec![random_topic], vec![], CONTRACT);
    let _ = generic.decode(&log, BLOCK_NUMBER, now(), Chain::Flare);
}

#[test]
fn test_all_decoders_return_none_for_no_topics() {
    let ftso = FtsoDecoder::new();
    let fdc = FdcDecoder::new();
    let fasset = FassetDecoder::new();
    let generic = GenericDecoder::new();

    let log = build_log(vec![], vec![], CONTRACT);
    assert!(ftso.decode(&log, BLOCK_NUMBER, now(), Chain::Flare).is_none());
    assert!(fdc.decode(&log, BLOCK_NUMBER, now(), Chain::Flare).is_none());
    assert!(fasset.decode(&log, BLOCK_NUMBER, now(), Chain::Flare).is_none());
    assert!(generic.decode(&log, BLOCK_NUMBER, now(), Chain::Flare).is_none());
}

// ═══════════════════════════════════════════════════════════════════
//  DecoderRegistry Routing
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_registry_routes_ftso_event() {
    let registry = DecoderRegistry::new();
    let topic0 = keccak256("PriceEpochFinalized(uint256,uint256)");

    let log = build_log(
        vec![topic0, B256::from(encode_u256(100))],
        vec![],
        CONTRACT,
    );

    let event = registry.decode(&log, BLOCK_NUMBER, now(), Chain::Flare).unwrap();
    assert_eq!(event.event_type, EventType::PriceEpochFinalized);
}

#[test]
fn test_registry_routes_fdc_event() {
    let registry = DecoderRegistry::new();
    let topic0 = keccak256("RoundFinalized(uint256,bytes32)");

    let log = build_log(
        vec![topic0, B256::from(encode_u256(42))],
        vec![],
        CONTRACT,
    );

    let event = registry.decode(&log, BLOCK_NUMBER, now(), Chain::Flare).unwrap();
    assert_eq!(event.event_type, EventType::RoundFinalized);
}

#[test]
fn test_registry_routes_fasset_event() {
    let registry = DecoderRegistry::new();
    let topic0 = keccak256("LiquidationStarted(address,uint256)");
    let agent = Address::repeat_byte(0xFF);

    let log = build_log(
        vec![topic0, address_to_topic(agent)],
        vec![],
        CONTRACT,
    );

    let event = registry.decode(&log, BLOCK_NUMBER, now(), Chain::Flare).unwrap();
    assert_eq!(event.event_type, EventType::LiquidationStarted);
}

#[test]
fn test_registry_without_generic_skips_unknown() {
    let registry = DecoderRegistry::new();
    let unknown = keccak256("NobodyKnowsThisEvent(bytes32)");

    let log = build_log(vec![unknown], vec![], CONTRACT);
    assert!(registry.decode(&log, BLOCK_NUMBER, now(), Chain::Flare).is_none());
}

#[test]
fn test_registry_with_generic_catches_unknown() {
    let registry = DecoderRegistry::with_generic();
    let unknown = keccak256("NobodyKnowsThisEvent(bytes32)");

    let log = build_log(vec![unknown], vec![], CONTRACT);
    let event = registry.decode(&log, BLOCK_NUMBER, now(), Chain::Flare).unwrap();
    assert_eq!(event.event_type, EventType::GenericEvent);
}

#[test]
fn test_registry_all_signatures_count() {
    let registry = DecoderRegistry::new();
    // FTSO: 3, FDC: 3, FAsset: 5 = 11 total (Generic has 0)
    assert_eq!(registry.all_signatures().len(), 11);
}

#[test]
fn test_registry_with_generic_signatures_count() {
    let registry = DecoderRegistry::with_generic();
    // Generic returns 0 signatures, so same as without
    assert_eq!(registry.all_signatures().len(), 11);
}

#[test]
fn test_registry_decoded_event_has_correct_chain() {
    let registry = DecoderRegistry::new();
    let topic0 = keccak256("RewardEpochStarted(uint256,uint256)");

    let log = build_log(vec![topic0, B256::from(encode_u256(1))], vec![], CONTRACT);

    let flare_event = registry.decode(&log, BLOCK_NUMBER, now(), Chain::Flare).unwrap();
    assert_eq!(flare_event.chain, Chain::Flare);

    let songbird_event = registry.decode(&log, BLOCK_NUMBER, now(), Chain::Songbird).unwrap();
    assert_eq!(songbird_event.chain, Chain::Songbird);
}

#[test]
fn test_registry_decoded_event_has_correct_address() {
    let registry = DecoderRegistry::new();
    let topic0 = keccak256("PriceEpochFinalized(uint256,uint256)");
    let custom_addr = Address::repeat_byte(0xDE);

    let log = build_log(
        vec![topic0, B256::from(encode_u256(1))],
        vec![],
        custom_addr,
    );

    let event = registry.decode(&log, BLOCK_NUMBER, now(), Chain::Flare).unwrap();
    assert!(event.address.contains("dededede"), "expected contract address in event, got {}", event.address);
}

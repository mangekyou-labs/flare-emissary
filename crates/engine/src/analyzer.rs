// TODO: Milestone 2 — Address analyzer
//
// Classifies an address using FlareContractRegistry + on-chain queries:
// 1. Check if address is FTSO data provider
// 2. Check if address is FAsset agent via AssetManager.getAgentInfo()
// 3. Check if address has code (contract) or is EOA
// 4. Return detected event types that can be subscribed to

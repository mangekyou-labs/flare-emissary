// TODO: Milestone 2 — Event processing pipeline
//
// Receives decoded events from the indexer and:
// 1. Persists them to TimescaleDB
// 2. Routes to CR calculator for FAsset events
// 3. Routes to alert matcher for subscription evaluation

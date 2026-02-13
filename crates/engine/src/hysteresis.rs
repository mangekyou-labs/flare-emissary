// TODO: Milestone 2 — Hysteresis & Cooldown engine
//
// Prevents alert fatigue:
// 1. Hysteresis: price must stay above/below threshold for N consecutive blocks
// 2. Cooldown: per-subscription timer in Redis (default 5 minutes)
// 3. Only emit alert when both conditions pass

// TODO: Milestone 2 — Alert matcher
//
// For each incoming event, evaluates all matching subscriptions:
// 1. Load active subscriptions filtering by address + event_type
// 2. Evaluate threshold_config against event decoded_data
// 3. Pass qualifying alerts through hysteresis engine
// 4. Generate Alert and Notification records

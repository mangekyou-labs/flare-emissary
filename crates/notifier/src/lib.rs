// TODO: Milestone 5 — Notification workers
//
// Workers that consume the Redis notification queue and deliver via:
// - Telegram (teloxide crate)
// - Discord (webhook + bot)
// - Email (Resend HTTP API)
//
// Features:
// - Retry with exponential backoff
// - Delivery status tracking (pending → sent → failed)
// - Dead letter queue for permanently failed notifications

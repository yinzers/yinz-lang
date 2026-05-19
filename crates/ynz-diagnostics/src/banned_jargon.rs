// Thin adapter — all data lives in registry/features.toml.
// Edit that file to add or remove banned-jargon entries.
// Schema reference: design/feature-registry.md #banned_jargon

pub use ynz_registry::{BannedJargonEntry, banned_jargon, banned_jargon_lookup};

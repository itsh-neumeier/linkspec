//! LinkSpec core.
//!
//! Models, USB Power Delivery decoders, the verdict engine and the mapping
//! to the `linkspec.diagnose.v1` schema. This crate uses no operating system
//! APIs, so all verdict tests run on every CI runner against fixtures.

#![forbid(unsafe_code)]

pub mod analyze;
pub mod catalog;
pub mod diagnosis;
pub mod display;
pub mod edid;
pub mod i18n;
pub mod observation;
pub mod pd;
pub mod session;
pub mod types;

pub use analyze::{Context, VERDICT_CODES, diagnose};
pub use catalog::{Catalog, EmptyCatalog, HistoryLookup, cable_fingerprint};
pub use diagnosis::{Diagnosis, Headline, Limiter, SCHEMA_ID};
pub use i18n::{I18n, Lang};
pub use observation::Snapshot;

/// Current time as RFC 3339 (UTC, seconds).
pub fn now_rfc3339() -> String {
    let now = time::OffsetDateTime::now_utc()
        .replace_nanosecond(0)
        .unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    now.format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

//! LinkSpec engine: one diagnosis pipeline for CLI and app ("kein zweites
//! Diagnose-Hirn"). Selects the platform backend, runs the session tracker,
//! records the cable history and produces the `linkspec.diagnose.v1` model.

#![forbid(unsafe_code)]

pub mod settings;

use std::path::Path;

use linkspec_core::catalog::cable_fingerprint;
use linkspec_core::diagnosis::Diagnosis;
use linkspec_core::observation::{CableObservation, Snapshot};
use linkspec_core::session::{FaultEvent, SessionTracker};
use linkspec_core::{Context, I18n, Lang, diagnose};
use linkspec_db::VendorDb;
use linkspec_history::{CableSighting, History};
use linkspec_platform::{PortSource, ReadOptions, SourceError};

pub use linkspec_platform as platform;

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// System language (German or English).
pub fn system_lang() -> Lang {
    sys_locale::get_locale()
        .map(|l| Lang::from_locale(&l))
        .unwrap_or(Lang::En)
}

/// Open the platform backend. `LINKSPEC_FAKE=<fixture>` selects the fake
/// backend (tests, demos, screenshots).
pub fn open_source(fake: Option<&Path>) -> Result<Box<dyn PortSource>, SourceError> {
    let tick = std::env::var("LINKSPEC_FAKE_TICK_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1000);
    let env = std::env::var_os("LINKSPEC_FAKE").map(std::path::PathBuf::from);
    if let Some(p) = fake.or(env.as_deref()) {
        return Ok(Box::new(
            linkspec_fake::FakeSource::from_file(p)?
                .with_tick(std::time::Duration::from_millis(tick)),
        ));
    }
    platform_source()
}

#[cfg(target_os = "linux")]
fn platform_source() -> Result<Box<dyn PortSource>, SourceError> {
    Ok(Box::new(linkspec_backend_linux::LinuxSource::new()))
}

#[cfg(target_os = "macos")]
fn platform_source() -> Result<Box<dyn PortSource>, SourceError> {
    Ok(Box::new(linkspec_backend_macos::MacSource::new()?))
}

#[cfg(windows)]
fn platform_source() -> Result<Box<dyn PortSource>, SourceError> {
    Ok(Box::new(linkspec_backend_windows::WindowsSource::new()?))
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn platform_source() -> Result<Box<dyn PortSource>, SourceError> {
    Err(SourceError::Unsupported(linkspec_platform::Unsupported::Os))
}

/// Options of the pipeline.
#[derive(Debug, Clone)]
pub struct EngineOptions {
    pub lang: Lang,
    pub usb_probe: bool,
    pub include_raw: bool,
    /// Record cables and faults in the local history.
    pub record_history: bool,
}

impl Default for EngineOptions {
    fn default() -> Self {
        Self {
            lang: system_lang(),
            usb_probe: true,
            include_raw: false,
            record_history: true,
        }
    }
}

/// The result of one run.
pub struct RunResult {
    pub snapshot: Snapshot,
    pub diagnosis: Diagnosis,
    pub new_faults: Vec<FaultEvent>,
}

pub struct Engine {
    source: Box<dyn PortSource>,
    catalog: VendorDb,
    history: Option<History>,
    tracker: SessionTracker,
    i18n: I18n,
    pub opts: EngineOptions,
}

impl Engine {
    pub fn new(source: Box<dyn PortSource>, opts: EngineOptions) -> Result<Self, String> {
        let history = if opts.record_history {
            match History::open_default() {
                Ok(h) => Some(h),
                Err(e) => {
                    tracing::warn!("history disabled: {e}");
                    None
                }
            }
        } else {
            None
        };
        Self::with_history(source, opts, history)
    }

    pub fn with_history(
        source: Box<dyn PortSource>,
        opts: EngineOptions,
        history: Option<History>,
    ) -> Result<Self, String> {
        Ok(Self {
            source,
            catalog: VendorDb::open_bundled().map_err(|e| e.to_string())?,
            history,
            tracker: SessionTracker::new(),
            i18n: I18n::new(opts.lang),
            opts,
        })
    }

    pub fn set_lang(&mut self, lang: Lang) {
        self.opts.lang = lang;
        self.i18n = I18n::new(lang);
    }

    pub fn i18n(&self) -> &I18n {
        &self.i18n
    }

    pub fn source_mut(&mut self) -> &mut dyn PortSource {
        self.source.as_mut()
    }

    pub fn history(&self) -> Option<&History> {
        self.history.as_ref()
    }

    pub fn catalog(&self) -> &VendorDb {
        &self.catalog
    }

    pub fn dismiss(&mut self, port_id: &str, kind: linkspec_core::observation::FaultKind) {
        self.tracker.dismiss(port_id, kind);
    }

    /// Read, track, record, diagnose.
    pub fn run(&mut self) -> Result<RunResult, SourceError> {
        let mut snap = self.source.snapshot(ReadOptions {
            usb_probe: self.opts.usb_probe,
        })?;
        let now = linkspec_core::now_rfc3339();
        let new_faults = self.tracker.update(&mut snap, &now);
        // Diagnose once without history to get localized cable summaries
        // for the sightings, then record and diagnose with history.
        let ctx = Context {
            i18n: &self.i18n,
            catalog: &self.catalog,
            history: None,
            include_raw: self.opts.include_raw,
            app_version: APP_VERSION,
            generated_at: now.clone(),
        };
        let mut diagnosis = diagnose(&snap, &ctx);
        if let Some(h) = &self.history {
            for (port, pd) in snap.ports.iter().zip(&diagnosis.ports) {
                let sighting = match &port.cable {
                    CableObservation::Emarker { identity, .. } => Some(CableSighting {
                        fingerprint: cable_fingerprint(identity),
                        vid: (identity.id_header & 0xffff) as u16,
                        pid: (identity.product >> 16) as u16,
                        summary: pd
                            .cable
                            .as_ref()
                            .map(|c| c.summary.clone())
                            .unwrap_or_default(),
                        contract_w: pd.power.as_ref().and_then(|p| p.contract_w.flatten()),
                        link_gbps: pd.link.as_ref().and_then(|l| l.fabric_gbps.or(l.usb_gbps)),
                        transports: pd
                            .link
                            .as_ref()
                            .map(|l| {
                                l.transports
                                    .iter()
                                    .map(|t| t.as_str().to_string())
                                    .collect()
                            })
                            .unwrap_or_default(),
                    }),
                    _ => None,
                };
                if let Err(e) = h.observe_port(&port.id, sighting.as_ref(), &now) {
                    tracing::warn!("history: {e}");
                }
            }
            for f in &new_faults {
                if let Err(e) = h.record_fault(&f.port_id, f.kind.as_str(), &f.at) {
                    tracing::warn!("history: {e}");
                }
            }
            let ctx = Context {
                history: Some(h),
                ..ctx
            };
            diagnosis = diagnose(&snap, &ctx);
        }
        Ok(RunResult {
            snapshot: snap,
            diagnosis,
            new_faults,
        })
    }
}

/// Diagnosis without volatile fields, for change detection in `--watch`.
pub fn comparable(d: &Diagnosis) -> serde_json::Value {
    let mut v = serde_json::to_value(d).unwrap_or_default();
    if let Some(o) = v.as_object_mut() {
        o.remove("generated_at");
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/core")
            .join(name)
    }

    #[test]
    fn pipeline_records_history_and_recognizes_cable() {
        let dir = tempfile::tempdir().unwrap();
        let h = History::open(&dir.path().join("history.db")).unwrap();
        let src = open_source(Some(&fixture("c02_usb2_device_on_40g_cable.json"))).unwrap();
        let mut e = Engine::with_history(
            src,
            EngineOptions {
                lang: Lang::De,
                ..Default::default()
            },
            Some(h),
        )
        .unwrap();
        let r = e.run().unwrap();
        let hist = r.diagnosis.ports[0]
            .cable
            .as_ref()
            .unwrap()
            .history
            .as_ref()
            .unwrap();
        assert_eq!(hist.uses, 1);
        let list = e.history().unwrap().list().unwrap();
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn output_validates_against_schema() {
        let schema: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../schemas/linkspec.diagnose.v1.json"),
            )
            .unwrap(),
        )
        .unwrap();
        let v = jsonschema::validator_for(&schema).unwrap();
        let src = open_source(Some(&fixture("c10_dock_usb3_and_dp.json"))).unwrap();
        let mut e = Engine::with_history(src, EngineOptions::default(), None).unwrap();
        let r = e.run().unwrap();
        let out = serde_json::to_value(&r.diagnosis).unwrap();
        assert!(v.is_valid(&out));
    }
}

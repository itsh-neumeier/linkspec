//! Fake backend. Replays fixture snapshots so that the CLI, the app and the
//! tests run without special hardware.
//!
//! Input formats:
//! * a core fixture (`{"snapshot": {...}, "expect": ...}`),
//! * a bare snapshot (`{"host": ..., "ports": [...]}`),
//! * a sequence (`{"sequence": [snapshot, snapshot, ...]}`): each
//!   [`FakeSource::advance`] or watch tick moves to the next snapshot.
//!
//! The fake counts the active USB queries it would send (one per USB
//! device) when deep probe is on. Criterion 19 tests that the counter
//! stays at zero with `usb_probe = false`.

#![forbid(unsafe_code)]

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use linkspec_core::Snapshot;
use linkspec_core::observation::{Notice, UsbDeviceObservation};
use linkspec_platform::{
    CAPTURE_SCHEMA, ChangeEvent, MonitorCapabilities, PortSource, PowerSample, ReadOptions,
    SourceError,
};

pub struct FakeSource {
    steps: Vec<Snapshot>,
    pos: Arc<AtomicUsize>,
    active_queries: u64,
    tick: Option<Duration>,
    samples: u64,
}

impl FakeSource {
    pub fn new(steps: Vec<Snapshot>) -> Self {
        Self {
            steps,
            pos: Arc::new(AtomicUsize::new(0)),
            active_queries: 0,
            tick: None,
            samples: 0,
        }
    }

    /// Load a fixture, bare snapshot or sequence file.
    pub fn from_file(path: &Path) -> Result<Self, SourceError> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| SourceError::NotReadable(format!("{}: {e}", path.display())))?;
        Self::from_json(&text)
    }

    pub fn from_json(text: &str) -> Result<Self, SourceError> {
        let v: serde_json::Value =
            serde_json::from_str(text).map_err(|e| SourceError::Other(e.to_string()))?;
        let parse = |v: &serde_json::Value| -> Result<Snapshot, SourceError> {
            serde_json::from_value(v.clone()).map_err(|e| SourceError::Other(e.to_string()))
        };
        let steps = if let Some(seq) = v.get("sequence").and_then(|s| s.as_array()) {
            seq.iter().map(parse).collect::<Result<Vec<_>, _>>()?
        } else if let Some(s) = v.get("snapshot") {
            vec![parse(s)?]
        } else {
            vec![parse(&v)?]
        };
        if steps.is_empty() {
            return Err(SourceError::Other("empty fixture".into()));
        }
        Ok(Self::new(steps))
    }

    /// Emit a change event every `tick` while watching (sequences only).
    pub fn with_tick(mut self, tick: Duration) -> Self {
        self.tick = Some(tick);
        self
    }

    /// Move to the next snapshot of a sequence (stays on the last one).
    pub fn advance(&mut self) {
        let p = self.pos.load(Ordering::SeqCst);
        if p + 1 < self.steps.len() {
            self.pos.store(p + 1, Ordering::SeqCst);
        }
    }

    fn count_devices(devs: &[UsbDeviceObservation]) -> u64 {
        devs.iter()
            .map(|d| 1 + Self::count_devices(&d.children))
            .sum()
    }
}

impl PortSource for FakeSource {
    fn name(&self) -> &'static str {
        "fake"
    }

    fn snapshot(&mut self, opts: ReadOptions) -> Result<Snapshot, SourceError> {
        let mut s = self.steps[self.pos.load(Ordering::SeqCst)].clone();
        s.usb_probe = opts.usb_probe;
        if opts.usb_probe {
            self.active_queries += s
                .ports
                .iter()
                .map(|p| Self::count_devices(&p.link.usb_devices))
                .sum::<u64>();
        } else if !s.notices.contains(&Notice::UsbProbeOff) {
            s.notices.push(Notice::UsbProbeOff);
        }
        Ok(s)
    }

    fn watch(&mut self) -> Result<Receiver<ChangeEvent>, SourceError> {
        let (tx, rx) = channel();
        if let Some(tick) = self.tick {
            let n = self.steps.len();
            let pos = Arc::clone(&self.pos);
            std::thread::spawn(move || {
                for i in 1..n {
                    std::thread::sleep(tick);
                    pos.store(i, Ordering::SeqCst);
                    if tx.send(ChangeEvent::Changed).is_err() {
                        return;
                    }
                }
                let _ = tx.send(ChangeEvent::Stopped);
            });
        }
        Ok(rx)
    }

    fn capture(&mut self, opts: ReadOptions) -> Result<serde_json::Value, SourceError> {
        let s = self.snapshot(opts)?;
        Ok(serde_json::json!({
            "schema": CAPTURE_SCHEMA,
            "backend": "fake",
            "anonymized": true,
            "synthetic": true,
            "host": s.host,
            "data": { "snapshot": s },
        }))
    }

    fn power_sample(&mut self) -> Result<Option<PowerSample>, SourceError> {
        // Replays the measured power of the first port with power data.
        self.samples += 1;
        let s = &self.steps[self.pos.load(Ordering::SeqCst)];
        for p in &s.ports {
            if let linkspec_core::observation::PowerObservation::Available(pd) = &p.power {
                let w = pd.measured_w.or(pd.contract.as_ref().map(|c| c.power_w));
                if w.is_some() {
                    return Ok(Some(PowerSample {
                        t: self.samples as f64,
                        watts: w,
                        volts: pd.measured_v.or(pd.contract.as_ref().map(|c| c.voltage_v)),
                        amps: pd.measured_a.or(pd.contract.as_ref().map(|c| c.current_a)),
                        port_index: Some(p.index),
                    }));
                }
            }
        }
        Ok(None)
    }

    fn monitor_capabilities(&self) -> MonitorCapabilities {
        MonitorCapabilities {
            watts: true,
            volts: true,
            amps: true,
            sources: vec!["fixture".into()],
        }
    }

    fn active_query_count(&self) -> u64 {
        self.active_queries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> FakeSource {
        let p = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/core")
            .join(name);
        FakeSource::from_file(&p).unwrap()
    }

    /// Criterion 19: deep probe off → no active device queries.
    #[test]
    fn probe_off_sends_no_active_queries() {
        let mut f = fixture("c10_dock_usb3_and_dp.json");
        let s = f.snapshot(ReadOptions { usb_probe: false }).unwrap();
        assert_eq!(f.active_query_count(), 0);
        assert!(!s.usb_probe);
        assert!(s.notices.contains(&Notice::UsbProbeOff));
        f.snapshot(ReadOptions { usb_probe: true }).unwrap();
        assert!(f.active_query_count() > 0);
    }
}

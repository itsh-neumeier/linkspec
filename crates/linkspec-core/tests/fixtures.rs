//! Fixture suite (prompt section 11). Every file in `fixtures/core/` is a
//! synthetic snapshot with expectations. The test diagnoses it in German and
//! English, validates the JSON against the schema and checks the
//! expectations. A second test makes sure every verdict code has a fixture.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use linkspec_core::catalog::Catalog;
use linkspec_core::{Context, I18n, Lang, Snapshot, VERDICT_CODES, diagnose};
use serde_json::Value;

pub struct FixtureCatalog {
    vendors: BTreeMap<u16, String>,
    xids: BTreeMap<u32, String>,
}

impl FixtureCatalog {
    fn from(v: Option<&Value>) -> Self {
        let mut vendors = BTreeMap::new();
        let mut xids = BTreeMap::new();
        if let Some(v) = v {
            if let Some(m) = v.get("vendors").and_then(Value::as_object) {
                for (k, n) in m {
                    vendors.insert(
                        u16::from_str_radix(k, 16).unwrap(),
                        n.as_str().unwrap().to_string(),
                    );
                }
            }
            if let Some(m) = v.get("xids").and_then(Value::as_object) {
                for (k, n) in m {
                    xids.insert(
                        u32::from_str_radix(k, 16).unwrap(),
                        n.as_str().unwrap().to_string(),
                    );
                }
            }
        }
        Self { vendors, xids }
    }
}

impl Catalog for FixtureCatalog {
    fn vendor_name(&self, vid: u16) -> Option<String> {
        self.vendors.get(&vid).cloned()
    }
    fn certified_name(&self, xid: u32) -> Option<String> {
        self.xids.get(&xid).cloned()
    }
    fn has_certification_list(&self) -> bool {
        !self.xids.is_empty()
    }
    fn known_cable(&self, _fingerprint: &str) -> Option<String> {
        None
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixtures() -> Vec<(String, Value)> {
    let dir = root().join("fixtures/core");
    let mut out = Vec::new();
    for e in std::fs::read_dir(&dir).expect("fixtures/core exists") {
        let p = e.unwrap().path();
        if p.extension().is_some_and(|x| x == "json") {
            let v: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap())
                .unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            out.push((p.file_stem().unwrap().to_string_lossy().to_string(), v));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    assert!(!out.is_empty());
    out
}

fn schema_validator() -> jsonschema::Validator {
    let schema: Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("schemas/linkspec.diagnose.v1.json")).unwrap(),
    )
    .unwrap();
    jsonschema::validator_for(&schema).expect("schema compiles")
}

fn run(fixture: &Value, lang: Lang) -> Value {
    let snap: Snapshot = serde_json::from_value(fixture["snapshot"].clone()).expect("snapshot");
    let i18n = I18n::new(lang);
    let catalog = FixtureCatalog::from(fixture.get("catalog"));
    let ctx = Context {
        i18n: &i18n,
        catalog: &catalog,
        history: None,
        include_raw: true,
        app_version: "0.0.0-test",
        generated_at: "2026-10-20T12:00:00Z".into(),
    };
    serde_json::to_value(diagnose(&snap, &ctx)).unwrap()
}

fn get<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = v;
    for part in path.split('.') {
        cur = cur.get(part)?;
    }
    Some(cur)
}

fn partial_match(name: &str, actual: &Value, expected: &Value, what: &str) {
    for (k, ev) in expected.as_object().unwrap() {
        let av = actual.get(k);
        assert!(
            av.is_some(),
            "{name}: {what}.{k} missing, expected {ev}, got {actual}"
        );
        let av = av.unwrap();
        let equal = match (av.as_f64(), ev.as_f64()) {
            (Some(a), Some(e)) => (a - e).abs() < 1e-9,
            _ => av == ev,
        };
        assert!(equal, "{name}: {what}.{k} = {av}, expected {ev}");
    }
}

fn check(name: &str, fixture: &Value, out_de: &Value, out_en: &Value) {
    let expect = &fixture["expect"];
    let outs = [("de", out_de), ("en", out_en)];
    if let Some(n) = expect.get("notices") {
        let want: BTreeSet<_> = n.as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect();
        let have: BTreeSet<_> = out_de["notices"]
            .as_array()
            .map(|a| a.iter().map(|x| x["code"].as_str().unwrap()).collect())
            .unwrap_or_default();
        assert_eq!(have, want, "{name}: notices");
    }
    for (i, pe) in expect["ports"].as_array().unwrap().iter().enumerate() {
        let port_de = &out_de["ports"][i];
        let port_en = &out_en["ports"][i];
        let ports = [("de", port_de), ("en", port_en)];
        if let Some(id) = pe.get("id") {
            assert_eq!(&port_de["id"], id, "{name}: id");
        }
        if let Some(h) = pe.get("headline") {
            assert_eq!(&port_de["headline"], h, "{name}: headline");
        }
        if let Some(s) = pe.get("status") {
            assert_eq!(&port_de["status"], s, "{name}: status");
        }
        if let Some(texts) = pe.get("headline_text") {
            for (lang, p) in ports {
                assert_eq!(&p["headline_text"], &texts[lang], "{name}: headline_text {lang}");
            }
        }
        if let Some(vs) = pe.get("verdicts") {
            let have: Vec<Value> = port_de["verdicts"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| {
                    serde_json::json!({"domain": v["domain"], "limiter": v["limiter"], "code": v["code"]})
                })
                .collect();
            assert_eq!(&Value::Array(have), vs, "{name}: verdicts");
        }
        if let Some(sc) = pe.get("summary_contains") {
            for (lang, p) in ports {
                if let Some(want) = sc.get(lang).and_then(Value::as_str) {
                    let s = p["summary"].as_str().unwrap();
                    assert!(s.contains(want), "{name}: summary {lang} = {s:?}, wanted {want:?}");
                }
            }
        }
        if let Some(vc) = pe.get("verdict_contains").and_then(Value::as_object) {
            for (code, texts) in vc {
                for (lang, p) in ports {
                    let v = p["verdicts"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|v| v["code"] == code.as_str())
                        .unwrap_or_else(|| panic!("{name}: verdict {code} missing"));
                    let want = texts[lang].as_str().unwrap();
                    let s = v["summary"].as_str().unwrap();
                    assert!(s.contains(want), "{name}: verdict {code} {lang} = {s:?}");
                }
            }
        }
        if let Some(c) = pe.get("cable") {
            partial_match(name, &port_de["cable"], c, "cable");
        }
        if let Some(c) = pe.get("power") {
            partial_match(name, &port_de["power"], c, "power");
        }
        if let Some(cs) = pe.get("cable_summary") {
            for (lang, p) in ports {
                assert_eq!(&p["cable"]["summary"], &cs[lang], "{name}: cable summary {lang}");
            }
        }
        if let Some(t) = pe.get("transports") {
            let want: BTreeSet<_> = t.as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect();
            let have: BTreeSet<_> = port_de["link"]["transports"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_str().unwrap())
                .collect();
            assert_eq!(have, want, "{name}: transports");
        }
        if let Some(abs) = pe.get("absent") {
            for path in abs.as_array().unwrap() {
                let path = path.as_str().unwrap();
                assert!(
                    get(port_de, path).is_none(),
                    "{name}: {path} must be absent, got {:?}",
                    get(port_de, path)
                );
            }
        }
        if let Some(u) = pe.get("unavailable") {
            let have: BTreeSet<_> = port_de["unavailable"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x["field"].as_str().unwrap())
                .collect();
            for f in u.as_array().unwrap() {
                assert!(have.contains(f.as_str().unwrap()), "{name}: unavailable {f}");
            }
        }
        if let Some(uc) = pe.get("unavailable_contains").and_then(Value::as_object) {
            for (field, texts) in uc {
                for (lang, p) in ports {
                    let Some(want) = texts.get(lang).and_then(Value::as_str) else {
                        continue;
                    };
                    let entry = p["unavailable"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|x| x["field"] == field.as_str())
                        .unwrap();
                    let s = entry["summary"].as_str().unwrap();
                    assert!(s.contains(want), "{name}: unavailable {field} {lang} = {s:?}");
                }
            }
        }
        let flags = |p: &Value| -> Vec<String> {
            p["cable"]["trust_flags"]
                .as_array()
                .map(|a| a.iter().map(|f| f["code"].as_str().unwrap().to_string()).collect())
                .unwrap_or_default()
        };
        if let Some(tf) = pe.get("trust_flags") {
            let want: Vec<String> = tf.as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect();
            assert_eq!(flags(port_de), want, "{name}: trust_flags");
        }
        if pe.get("no_trust_flags").is_some() {
            assert!(flags(port_de).is_empty(), "{name}: unexpected trust flags {:?}", flags(port_de));
        }
        if let Some(n) = pe.get("notes") {
            let have: BTreeSet<_> = port_de["notes"]
                .as_array()
                .map(|a| a.iter().map(|x| x["code"].as_str().unwrap()).collect())
                .unwrap_or_default();
            for c in n.as_array().unwrap() {
                assert!(have.contains(c.as_str().unwrap()), "{name}: note {c}");
            }
        }
        if let Some(words) = pe.get("no_text") {
            for (_, out) in outs {
                let text = out.to_string();
                for w in words.as_array().unwrap() {
                    assert!(!text.contains(w.as_str().unwrap()), "{name}: forbidden text {w}");
                }
            }
        }
    }
}

#[test]
fn fixture_suite() {
    let validator = schema_validator();
    for (name, fixture) in fixtures() {
        assert_eq!(
            fixture["meta"]["synthetic"],
            Value::Bool(true),
            "{name}: core fixtures must be marked synthetic"
        );
        let de = run(&fixture, Lang::De);
        let en = run(&fixture, Lang::En);
        for (lang, out) in [("de", &de), ("en", &en)] {
            let errors: Vec<String> = validator
                .iter_errors(out)
                .map(|e| format!("{} at {}", e, e.instance_path()))
                .collect();
            assert!(errors.is_empty(), "{name} ({lang}): schema errors {errors:#?}\n{out:#}");
            // No unresolved message ids in any text.
            let text = out.to_string();
            for prefix in ["verdict-", "headline-", "trust-", "fault-", "unavailable-", "reason-", "cable-kind-"] {
                assert!(
                    !text.contains(&format!("\"{prefix}")),
                    "{name} ({lang}): untranslated id with prefix {prefix}"
                );
            }
        }
        check(&name, &fixture, &de, &en);
    }
}

#[test]
fn every_verdict_code_has_a_fixture() {
    let mut seen = BTreeSet::new();
    for (_, fixture) in fixtures() {
        for p in fixture["expect"]["ports"].as_array().unwrap() {
            if let Some(vs) = p.get("verdicts").and_then(Value::as_array) {
                for v in vs {
                    seen.insert(v["code"].as_str().unwrap().to_string());
                }
            }
        }
    }
    let missing: Vec<_> = VERDICT_CODES.iter().filter(|c| !seen.contains(**c)).collect();
    assert!(missing.is_empty(), "verdict codes without fixture: {missing:?}");
}

#[test]
fn every_verdict_code_has_text_in_both_languages() {
    for lang in [Lang::De, Lang::En] {
        let i18n = I18n::new(lang);
        for code in VERDICT_CODES {
            assert!(i18n.has(&format!("verdict-{code}")), "{lang:?}: verdict-{code}");
        }
    }
}

/// Texts are pinned per fixture and language (insta snapshots).
#[test]
fn texts_are_stable() {
    for (name, fixture) in fixtures() {
        for lang in [Lang::De, Lang::En] {
            let out = run(&fixture, lang);
            let mut lines = Vec::new();
            for p in out["ports"].as_array().unwrap() {
                lines.push(format!(
                    "{} | {} | {}",
                    p["label"].as_str().unwrap(),
                    p["headline_text"].as_str().unwrap(),
                    p["summary"].as_str().unwrap()
                ));
                if let Some(c) = p.get("cable") {
                    lines.push(format!("  cable: {}", c["summary"].as_str().unwrap()));
                    if let Some(flags) = c["trust_flags"].as_array() {
                        for f in flags {
                            lines.push(format!("  trust: {}", f["summary"].as_str().unwrap()));
                        }
                    }
                }
                for v in p["verdicts"].as_array().unwrap() {
                    lines.push(format!(
                        "  {}/{}: {}",
                        v["domain"].as_str().unwrap(),
                        v["limiter"].as_str().unwrap(),
                        v["summary"].as_str().unwrap()
                    ));
                }
                for u in p["unavailable"].as_array().unwrap() {
                    lines.push(format!("  unavailable {}: {}", u["field"].as_str().unwrap(), u["summary"].as_str().unwrap()));
                }
                for n in p.get("notes").and_then(Value::as_array).into_iter().flatten() {
                    lines.push(format!("  note: {}", n["summary"].as_str().unwrap()));
                }
            }
            insta::assert_snapshot!(format!("{name}_{}", lang.code()), lines.join("\n"));
        }
    }
}

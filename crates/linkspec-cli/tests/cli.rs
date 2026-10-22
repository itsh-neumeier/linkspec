//! CLI acceptance tests (criteria 14, 16, 17, 19) against fixtures.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use linkspec_platform::anonymize::violations;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn bin() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_linkspec"));
    c.env("LINKSPEC_LOG", "debug"); // logs must still never reach stdout
    c.env_remove("LINKSPEC_FAKE");
    c
}

fn fixtures() -> Vec<PathBuf> {
    let mut v: Vec<_> = std::fs::read_dir(root().join("fixtures/core"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    v.sort();
    v
}

/// Criterion 14: `--json` is valid against the schema, stdout has no logs.
#[test]
fn json_output_validates_and_stdout_is_clean() {
    let schema: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("schemas/linkspec.diagnose.v1.json")).unwrap(),
    )
    .unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    for f in fixtures() {
        for lang in ["de", "en"] {
            let out = bin()
                .args(["--json", "--lang", lang, "--fake"])
                .arg(&f)
                .output()
                .unwrap();
            assert!(out.status.success(), "{}: {:?}", f.display(), out);
            let stdout = String::from_utf8(out.stdout).unwrap();
            let v: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
                panic!("{}: stdout is not one JSON document: {e}", f.display())
            });
            assert!(validator.is_valid(&v), "{}: schema", f.display());
            assert_eq!(v["schema"], "linkspec.diagnose.v1");
        }
    }
}

#[test]
fn raw_json_validates() {
    let schema: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("schemas/linkspec.diagnose.v1.json")).unwrap(),
    )
    .unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let out = bin()
        .args(["--json", "--raw", "--fake"])
        .arg(root().join("fixtures/core/c02_usb2_device_on_40g_cable.json"))
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(validator.is_valid(&v));
    assert!(v["ports"][0]["raw"]["cable_vdos"].is_object());
}

#[test]
fn text_output_has_symbols_and_no_color_when_piped() {
    let out = bin()
        .args(["--lang", "de", "--fake"])
        .arg(root().join("fixtures/core/c03_cable_limits_power.json"))
        .output()
        .unwrap();
    let s = String::from_utf8(out.stdout).unwrap();
    assert!(s.starts_with("! USB-C 1"), "{s}");
    assert!(!s.contains('\x1b'), "no ANSI codes when piped");
    insta::assert_snapshot!("text_c03_de", s);
}

#[test]
fn usage_error_exits_2() {
    let out = bin().arg("--does-not-exist").output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    let out = bin().args(["--lang", "fr"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn watch_json_emits_ndjson_per_change() {
    // A two-step sequence: the second snapshot differs.
    let a: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("fixtures/core/c01_empty_port.json")).unwrap(),
    )
    .unwrap();
    let b: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("fixtures/core/c02_usb2_device_on_40g_cable.json"))
            .unwrap(),
    )
    .unwrap();
    let seq = serde_json::json!({"sequence": [a["snapshot"], b["snapshot"]]});
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("seq.json");
    std::fs::write(&p, seq.to_string()).unwrap();
    let out = bin()
        .args(["--watch", "--json", "--fake"])
        .arg(&p)
        .env("LINKSPEC_FAKE_TICK_MS", "50")
        .output()
        .unwrap();
    assert!(out.status.success());
    let lines: Vec<_> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(lines.len(), 2, "{lines:?}");
    for l in lines {
        let v: serde_json::Value = serde_json::from_str(&l).unwrap();
        assert_eq!(v["schema"], "linkspec.diagnose.v1");
    }
}

/// Criterion 17: captures contain no serials, user or host names, or paths.
#[test]
fn capture_is_anonymized() {
    let dir = tempfile::tempdir().unwrap();
    let out_path = dir.path().join("cap.json");
    let out = bin()
        .args(["--capture", "--output"])
        .arg(&out_path)
        .arg("--fake")
        .arg(root().join("fixtures/core/c02_usb2_device_on_40g_cable.json"))
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&out_path).unwrap()).unwrap();
    assert!(violations(&v, &linkspec_platform::anonymize::local_secrets()).is_empty());
    assert_eq!(v["anonymized"], true);
}

/// Criterion 17 for all stored platform fixtures and the report fields.
#[test]
fn fixtures_and_reports_have_no_personal_data() {
    for dir in ["fixtures/linux", "fixtures/macos", "fixtures/windows"] {
        for e in std::fs::read_dir(root().join(dir)).unwrap() {
            let p = e.unwrap().path();
            let v: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
            let bad = violations(&v, &[]);
            assert!(bad.is_empty(), "{}: {bad:?}", p.display());
        }
    }
    // Report fields: fixed allow-list of keys.
    let out = bin()
        .args(["--json", "--fake"])
        .arg(root().join("fixtures/core/c02_usb2_device_on_40g_cable.json"))
        .output()
        .unwrap();
    assert!(out.status.success());
    let allowed: BTreeSet<&str> = [
        "vid",
        "pid",
        "cable_type",
        "speed",
        "power",
        "xid",
        "fingerprint",
        "vdos",
        "platform",
        "observed",
    ]
    .into_iter()
    .collect();
    let fake = linkspec_engine::open_source(Some(
        &root().join("fixtures/core/c02_usb2_device_on_40g_cable.json"),
    ))
    .unwrap();
    let mut eng = linkspec_engine::Engine::with_history(fake, Default::default(), None).unwrap();
    let r = eng.run().unwrap();
    let fields = linkspec_cli::report::fields(&r.snapshot, &r.diagnosis).unwrap();
    let map: BTreeMap<_, _> = fields.iter().cloned().collect();
    assert!(map.keys().all(|k| allowed.contains(k)));
    let url = linkspec_cli::report::url(&fields);
    assert!(url.starts_with(
        "https://github.com/itsh-neumeier/linkspec/issues/new?template=kabel-melden.yml"
    ));
    let as_json = serde_json::to_value(&map).unwrap();
    assert!(violations(&as_json, &linkspec_platform::anonymize::local_secrets()).is_empty());
}

/// Criterion 16: the CLI has no network code. The dependency tree of the
/// binary contains no HTTP, TLS or socket crate (checked in Cargo.lock).
#[test]
fn no_network_dependencies() {
    let lock = std::fs::read_to_string(root().join("Cargo.lock")).unwrap();
    let mut deps: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut name = String::new();
    let mut in_deps = false;
    for line in lock.lines() {
        if let Some(n) = line.strip_prefix("name = ") {
            name = n.trim_matches('"').to_string();
            deps.entry(name.clone()).or_default();
            in_deps = false;
        } else if line.starts_with("dependencies = [") {
            in_deps = true;
        } else if in_deps && line.starts_with(']') {
            in_deps = false;
        } else if in_deps {
            let d = line.trim().trim_matches(|c| c == '"' || c == ',');
            let d = d.split(' ').next().unwrap_or_default().to_string();
            deps.entry(name.clone()).or_default().push(d);
        }
    }
    let mut seen = BTreeSet::new();
    let mut stack = vec!["linkspec-cli".to_string()];
    while let Some(n) = stack.pop() {
        if !seen.insert(n.clone()) {
            continue;
        }
        stack.extend(deps.get(&n).cloned().unwrap_or_default());
    }
    for bad in [
        "reqwest",
        "ureq",
        "hyper",
        "rustls",
        "native-tls",
        "openssl",
        "curl",
        "isahc",
        "attohttpc",
        "tokio",
    ] {
        assert!(!seen.contains(bad), "network crate in CLI tree: {bad}");
    }
}

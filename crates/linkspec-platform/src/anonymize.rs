//! Anonymization for captures and reports (prompt section 7).
//!
//! Captures never contain serial numbers, user names, host names, file
//! paths or clipboard content. Backends collect only allow-listed values;
//! this module is the second line of defence and the test oracle.

use serde_json::Value;

/// Key fragments that are never allowed in a capture (case-insensitive).
pub const FORBIDDEN_KEY_PARTS: &[&str] = &[
    "serial",
    "uuid",
    "unique_id",
    "uniqueid",
    "hostname",
    "host_name",
    "computername",
    "username",
    "user_name",
    "login",
    "email",
    "clipboard",
    "home",
    "mac_address",
    "macaddress",
    "ioplatformuuid",
    "uid",
];

/// Keys that contain a forbidden fragment but are known to be harmless.
const ALLOWED_KEYS: &[&str] = &["guid_class", "class_guid", "interface_guid"];

pub const REDACTED: &str = "<redacted>";

fn key_forbidden(k: &str) -> bool {
    let lk = k.to_ascii_lowercase();
    if ALLOWED_KEYS.contains(&lk.as_str()) {
        return false;
    }
    FORBIDDEN_KEY_PARTS.iter().any(|p| {
        // "uid" alone must not match "guid" etc.: require a word boundary.
        if *p == "uid" {
            lk == "uid" || lk.ends_with("_uid") || lk.starts_with("uid_")
        } else {
            lk.contains(p)
        }
    })
}

/// Strings that look like user file paths.
fn value_forbidden(s: &str, secrets: &[String]) -> bool {
    let l = s.to_ascii_lowercase();
    if l.contains("/home/")
        || l.contains("/users/")
        || l.contains("\\users\\")
        || l.contains("/root/")
        || l.contains("c:\\")
        || l.contains("/var/folders/")
        || l.contains("appdata")
    {
        return true;
    }
    secrets.iter().any(|sec| !sec.is_empty() && l.contains(&sec.to_ascii_lowercase()))
}

/// Host-specific secrets: user name and host name from the environment.
/// Values with fewer than 3 characters are ignored (too many false hits).
pub fn local_secrets() -> Vec<String> {
    let mut out = Vec::new();
    for var in ["USER", "USERNAME", "LOGNAME", "HOSTNAME", "COMPUTERNAME"] {
        if let Ok(v) = std::env::var(var)
            && v.len() >= 3
            && v != "root"
        {
            out.push(v);
        }
    }
    if let Ok(h) = std::fs::read_to_string("/etc/hostname") {
        let h = h.trim().to_string();
        if h.len() >= 3 {
            out.push(h);
        }
    }
    out
}

/// Remove forbidden keys and redact forbidden values in place.
pub fn scrub(v: &mut Value, secrets: &[String]) {
    match v {
        Value::Object(map) => {
            map.retain(|k, _| !key_forbidden(k));
            for (_, val) in map.iter_mut() {
                scrub(val, secrets);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| scrub(x, secrets)),
        Value::String(s) if value_forbidden(s, secrets) => {
            *s = REDACTED.to_string();
        }
        _ => {}
    }
}

/// Return every violation (for tests): forbidden keys and values.
pub fn violations(v: &Value, secrets: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(v: &Value, path: &str, secrets: &[String], out: &mut Vec<String>) {
        match v {
            Value::Object(map) => {
                for (k, val) in map {
                    let p = format!("{path}/{k}");
                    if key_forbidden(k) {
                        out.push(format!("key {p}"));
                    }
                    walk(val, &p, secrets, out);
                }
            }
            Value::Array(a) => {
                for (i, x) in a.iter().enumerate() {
                    walk(x, &format!("{path}[{i}]"), secrets, out);
                }
            }
            Value::String(s) if value_forbidden(s, secrets) => {
                out.push(format!("value {path}"));
            }
            _ => {}
        }
    }
    walk(v, "", secrets, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn removes_serials_and_paths() {
        let mut v = json!({
            "port": {"serial": "ABC123", "USB Serial Number": "X", "speed": "480"},
            "unique_id": "deadbeef",
            "file": "/home/alice/x",
            "win": "C:\\Users\\alice",
            "class_guid": "{36fc9e60-c465-11cf-8056-444553540000}",
            "name": "box-of-alice",
        });
        let secrets = vec!["box-of-alice".to_string()];
        assert!(!violations(&v, &secrets).is_empty());
        scrub(&mut v, &secrets);
        assert!(violations(&v, &secrets).is_empty(), "{v}");
        assert_eq!(v["port"]["speed"], "480");
        assert_eq!(v["class_guid"], "{36fc9e60-c465-11cf-8056-444553540000}");
        assert_eq!(v["name"], REDACTED);
    }

    #[test]
    fn uid_word_boundary() {
        assert!(key_forbidden("uid"));
        assert!(key_forbidden("device_uid"));
        assert!(!key_forbidden("guid_class"));
        assert!(!key_forbidden("fluid"));
    }
}

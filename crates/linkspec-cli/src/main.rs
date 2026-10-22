//! `linkspec` – what can this USB-C cable do, and which link limits the
//! connection: port, cable, charger or device?
//!
//! Exit codes: 0 diagnosis created (also with `unknown`), 2 usage error,
//! 3 platform not supported or data source not readable.

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{ArgGroup, Parser};
use linkspec_core::{I18n, Lang};
use linkspec_engine::{Engine, EngineOptions, comparable, open_source, system_lang};
use linkspec_platform::{ChangeEvent, ReadOptions, SourceError, Unsupported};

#[derive(Parser, Debug)]
#[command(
    name = "linkspec",
    version,
    about = "USB-C / Thunderbolt / USB4 diagnosis: what the cable can do and which link limits the connection",
    long_about = "USB-C / Thunderbolt / USB4 diagnosis.\n\nLinkSpec reads what the port controllers and cable E-Markers report. It is not a cable tester: E-Markers can report wrong values.",
    group(ArgGroup::new("mode").args(["monitor", "monitor_json", "dashboard", "report", "capture", "tray", "window"]))
)]
struct Cli {
    /// One JSON document (schema linkspec.diagnose.v1) on stdout.
    #[arg(long)]
    json: bool,
    /// Repeat the output on every change (NDJSON with --json).
    #[arg(long)]
    watch: bool,
    /// Add decoded raw fields and platform source values.
    #[arg(long)]
    raw: bool,
    /// Do not send active USB queries (deep probe).
    #[arg(long)]
    no_usb_probe: bool,
    /// Live power data as text.
    #[arg(long)]
    monitor: bool,
    /// Live power data as NDJSON.
    #[arg(long)]
    monitor_json: bool,
    /// Full-screen dashboard (Tab switches, q quits).
    #[arg(long)]
    dashboard: bool,
    /// Open a prefilled GitHub issue for the connected E-Marker cable.
    #[arg(long)]
    report: bool,
    /// Write an anonymized capture file for bug reports and fixtures.
    #[arg(long)]
    capture: bool,
    /// Output path for --capture.
    #[arg(long, value_name = "PATH", requires = "capture")]
    output: Option<PathBuf>,
    /// Force the language.
    #[arg(long, value_parser = ["de", "en"])]
    lang: Option<String>,
    /// Start the app in tray / menu bar mode.
    #[arg(long)]
    tray: bool,
    /// Start the app in window mode.
    #[arg(long)]
    window: bool,
    /// Hide ports with nothing connected.
    #[arg(long)]
    hide_empty: bool,
    /// Do not record cables in the local history.
    #[arg(long)]
    no_history: bool,
    /// Replay a fixture instead of reading the hardware (tests, demos).
    #[arg(long, value_name = "FILE", hide = true)]
    fake: Option<PathBuf>,
}

const EXIT_UNSUPPORTED: u8 = 3;

fn main() -> ExitCode {
    // x86_64 slice of the universal macOS binary: only the Intel message.
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        let lang = system_lang();
        eprintln!("{}", I18n::new(lang).t("cli-unsupported-intel-mac", &[]));
        return ExitCode::from(EXIT_UNSUPPORTED);
    }
    #[allow(unreachable_code)]
    run()
}

fn init_logging() {
    let filter = tracing_subscriber::EnvFilter::try_from_env("LINKSPEC_LOG")
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .try_init();
}

fn source_error(e: &SourceError, i18n: &I18n) -> ExitCode {
    match e {
        SourceError::Unsupported(Unsupported::IntelMac) => {
            eprintln!("{}", i18n.t("cli-unsupported-intel-mac", &[]));
        }
        SourceError::Unsupported(u) => {
            eprintln!(
                "{}",
                i18n.t("cli-unsupported", &[("reason", u.to_string().into())])
            );
        }
        SourceError::NotReadable(r) | SourceError::Other(r) => {
            eprintln!(
                "{}",
                i18n.t("cli-unsupported", &[("reason", r.clone().into())])
            );
        }
    }
    ExitCode::from(EXIT_UNSUPPORTED)
}

fn app_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let name = if cfg!(windows) {
        "linkspec-app.exe"
    } else {
        "linkspec-app"
    };
    [dir.join(name), dir.join("../MacOS").join(name)]
        .into_iter()
        .find(|p| p.exists())
}

fn run() -> ExitCode {
    let cli = Cli::parse();
    init_logging();
    let lang = cli
        .lang
        .as_deref()
        .and_then(Lang::parse)
        .unwrap_or_else(system_lang);
    let i18n = I18n::new(lang);

    if cli.tray || cli.window {
        let Some(app) = app_path() else {
            eprintln!("{}", i18n.t("cli-app-missing", &[]));
            return ExitCode::from(EXIT_UNSUPPORTED);
        };
        let mode = if cli.tray { "--tray" } else { "--window" };
        return match std::process::Command::new(app).arg(mode).spawn() {
            Ok(_) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{e}");
                ExitCode::from(EXIT_UNSUPPORTED)
            }
        };
    }

    let mut source = match open_source(cli.fake.as_deref()) {
        Ok(s) => s,
        Err(e) => return source_error(&e, &i18n),
    };
    let opts = ReadOptions {
        usb_probe: !cli.no_usb_probe,
    };

    if cli.capture {
        let doc = match source.capture(opts) {
            Ok(d) => d,
            Err(e) => return source_error(&e, &i18n),
        };
        let path = cli.output.clone().unwrap_or_else(|| {
            let ts = linkspec_core::now_rfc3339().replace([':', '-'], "");
            PathBuf::from(format!("linkspec-capture-{}-{ts}.json", source.name()))
        });
        let text = serde_json::to_string_pretty(&doc).unwrap_or_default() + "\n";
        if let Err(e) = std::fs::write(&path, text) {
            eprintln!("{}: {e}", path.display());
            return ExitCode::from(1);
        }
        eprintln!(
            "{}",
            i18n.t(
                "cli-capture-written",
                &[("path", path.display().to_string().into())]
            )
        );
        return ExitCode::SUCCESS;
    }

    if cli.monitor || cli.monitor_json {
        return monitor(source, cli.monitor_json, &i18n);
    }

    let record =
        !cli.no_history && cli.fake.is_none() && std::env::var_os("LINKSPEC_FAKE").is_none();
    let mut engine = match Engine::new(
        source,
        EngineOptions {
            lang,
            usb_probe: opts.usb_probe,
            include_raw: cli.raw,
            record_history: record,
        },
    ) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };

    if cli.dashboard {
        return match linkspec_cli::dashboard::run(&mut engine) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{e}");
                ExitCode::from(1)
            }
        };
    }

    let result = match engine.run() {
        Ok(r) => r,
        Err(e) => return source_error(&e, &i18n),
    };

    if cli.report {
        let Some(fields) = linkspec_cli::report::fields(&result.snapshot, &result.diagnosis) else {
            eprintln!("{}", i18n.t("cli-report-no-emarker", &[]));
            return ExitCode::SUCCESS;
        };
        let url = linkspec_cli::report::url(&fields);
        if linkspec_cli::report::open_browser(&url) {
            eprintln!("{}", i18n.t("cli-report-opened", &[]));
        } else {
            println!("{}\n{url}", i18n.t("cli-report-url", &[]));
        }
        return ExitCode::SUCCESS;
    }

    let style = linkspec_cli::render::Style {
        color: !cli.json && linkspec_cli::color_enabled(),
    };
    let print = |d: &linkspec_core::Diagnosis, pretty: bool| {
        let mut out = std::io::stdout().lock();
        if cli.json {
            let s = if pretty {
                serde_json::to_string_pretty(d)
            } else {
                serde_json::to_string(d)
            }
            .unwrap_or_default();
            let _ = writeln!(out, "{s}");
        } else {
            let _ = write!(
                out,
                "{}",
                linkspec_cli::render::text(d, engine_i18n(lang), style, cli.raw, cli.hide_empty)
            );
        }
        let _ = out.flush();
    };

    if !cli.watch {
        print(&result.diagnosis, true);
        return ExitCode::SUCCESS;
    }

    // --watch: event driven; print again on every relevant change.
    print(&result.diagnosis, false);
    if !cli.json {
        eprintln!("{}", i18n.t("cli-watch-hint", &[]));
    }
    let rx = match engine.source_mut().watch() {
        Ok(rx) => rx,
        Err(e) => return source_error(&e, &i18n),
    };
    let mut last = comparable(&result.diagnosis);
    while let Ok(ev) = rx.recv() {
        if ev == ChangeEvent::Stopped {
            break;
        }
        let Ok(r) = engine.run() else { continue };
        let now = comparable(&r.diagnosis);
        if now != last {
            last = now;
            if !cli.json {
                println!("\n— {} —", r.diagnosis.generated_at);
            }
            print(&r.diagnosis, false);
        }
    }
    ExitCode::SUCCESS
}

fn engine_i18n(lang: Lang) -> &'static I18n {
    // One table per language for the lifetime of the process.
    static DE: std::sync::OnceLock<I18n> = std::sync::OnceLock::new();
    static EN: std::sync::OnceLock<I18n> = std::sync::OnceLock::new();
    match lang {
        Lang::De => DE.get_or_init(|| I18n::new(Lang::De)),
        Lang::En => EN.get_or_init(|| I18n::new(Lang::En)),
    }
}

fn monitor(
    mut source: Box<dyn linkspec_platform::PortSource>,
    json: bool,
    i18n: &I18n,
) -> ExitCode {
    let caps = source.monitor_capabilities();
    if !json {
        println!(
            "{}  [{}]",
            i18n.t("cli-monitor-header", &[]),
            caps.sources.join(", ")
        );
    }
    let mut misses = 0;
    loop {
        match source.power_sample() {
            Ok(Some(s)) => {
                misses = 0;
                if json {
                    println!("{}", serde_json::to_string(&s).unwrap_or_default());
                } else {
                    let f = |v: Option<f64>| v.map(|x| i18n.num(x)).unwrap_or_else(|| "–".into());
                    println!(
                        "{:>8.1}s  {:>7} W  {:>6} V  {:>6} A",
                        s.t,
                        f(s.watts),
                        f(s.volts),
                        f(s.amps)
                    );
                }
            }
            Ok(None) => {
                misses += 1;
                if misses == 3 {
                    eprintln!("{}", i18n.t("cli-monitor-none", &[]));
                    return ExitCode::SUCCESS;
                }
            }
            Err(e) => return source_error(&e, i18n),
        }
        let _ = std::io::stdout().flush();
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

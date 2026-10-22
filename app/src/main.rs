//! LinkSpec desktop app: tray icon / menu bar item with a flyout, and a
//! window with the extended views (power, negotiation, display, cables,
//! settings, about). The diagnosis runs in a worker thread through the
//! same engine as the CLI.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod platform;
mod worker;

use std::cell::RefCell;
use std::rc::Rc;

use linkspec_core::diagnosis::Status;
use linkspec_core::observation::FaultKind;
use linkspec_core::{I18n, Lang};
use linkspec_engine::settings::{Settings, UiMode};
use linkspec_engine::{Engine, EngineOptions, open_source};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use worker::{Command, Update};

slint::include_modules!();

fn init_logging() -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let dir = linkspec_history::data_dir()?.join("logs");
    std::fs::create_dir_all(&dir).ok()?;
    let appender = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("linkspec")
        .filename_suffix("log")
        .max_log_files(5)
        .build(&dir)
        .ok()?;
    let (w, guard) = tracing_appender::non_blocking(appender);
    let filter = tracing_subscriber::EnvFilter::try_from_env("LINKSPEC_LOG")
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(w)
        .with_ansi(false)
        .try_init();
    Some(guard)
}

fn load_icon(bytes: &[u8], template: bool) -> Option<Icon> {
    let dec = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = dec.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let mut rgba = buf[..info.buffer_size()].to_vec();
    if template {
        // macOS template image: black with alpha.
        for px in rgba.as_chunks_mut::<4>().0 {
            px[0] = 0;
            px[1] = 0;
            px[2] = 0;
        }
    }
    Icon::from_rgba(rgba, info.width, info.height).ok()
}

struct Menus {
    refresh: MenuItem,
    keep_open: CheckMenuItem,
    settings: MenuItem,
    power: MenuItem,
    negotiation: MenuItem,
    display: MenuItem,
    cables: MenuItem,
    updates: MenuItem,
    report: MenuItem,
    about: MenuItem,
    github: MenuItem,
    quit: MenuItem,
}

impl Menus {
    fn new(i18n: &I18n, keep_open: bool) -> Self {
        let t = |k: &str| i18n.t(k, &[]);
        Self {
            refresh: MenuItem::new(t("app-refresh"), true, None),
            keep_open: CheckMenuItem::new(t("menu-keep-open"), true, keep_open, None),
            settings: MenuItem::new(t("app-settings"), true, None),
            power: MenuItem::new(t("tab-power"), true, None),
            negotiation: MenuItem::new(t("tab-negotiation"), true, None),
            display: MenuItem::new(t("tab-display"), true, None),
            cables: MenuItem::new(t("tab-cables"), true, None),
            updates: MenuItem::new(t("menu-check-updates"), true, None),
            report: MenuItem::new(t("menu-report"), true, None),
            about: MenuItem::new(t("tab-about"), true, None),
            github: MenuItem::new("GitHub", true, None),
            quit: MenuItem::new(t("menu-quit"), true, None),
        }
    }

    fn relabel(&self, i18n: &I18n) {
        let t = |k: &str| i18n.t(k, &[]);
        self.refresh.set_text(t("app-refresh"));
        self.keep_open.set_text(t("menu-keep-open"));
        self.settings.set_text(t("app-settings"));
        self.power.set_text(t("tab-power"));
        self.negotiation.set_text(t("tab-negotiation"));
        self.display.set_text(t("tab-display"));
        self.cables.set_text(t("tab-cables"));
        self.updates.set_text(t("menu-check-updates"));
        self.report.set_text(t("menu-report"));
        self.about.set_text(t("tab-about"));
        self.quit.set_text(t("menu-quit"));
    }

    fn menu(&self) -> Menu {
        let m = Menu::new();
        let sep = PredefinedMenuItem::separator();
        let _ = m.append_items(&[
            &self.refresh,
            &self.keep_open,
            &self.settings,
            &sep,
            &self.power,
            &self.negotiation,
            &self.display,
            &self.cables,
            &PredefinedMenuItem::separator(),
            &self.updates,
            &self.report,
            &self.about,
            &self.github,
            &PredefinedMenuItem::separator(),
            &self.quit,
        ]);
        m
    }
}

const TAB_POWER: i32 = 1;

fn main() {
    let _guard = init_logging();
    let args: Vec<String> = std::env::args().collect();
    let mut settings = Settings::load();
    if args.iter().any(|a| a == "--window") {
        settings.mode = UiMode::Window;
    }
    if args.iter().any(|a| a == "--tray") {
        settings.mode = UiMode::Tray;
    }
    let fake = args
        .iter()
        .position(|a| a == "--fake")
        .and_then(|i| args.get(i + 1))
        .map(std::path::PathBuf::from);
    let tray_wanted = settings.mode == UiMode::Tray && platform::tray_probably_available();

    let lang = settings.lang();
    let i18n = Rc::new(RefCell::new(I18n::new(lang)));

    let main_window = MainWindow::new().expect("main window");
    let flyout = Flyout::new().expect("flyout");

    // Translations.
    for g in [main_window.global::<Tr>(), flyout.global::<Tr>()] {
        let i = i18n.clone();
        g.on_t(move |key, _rev| SharedString::from(i.borrow().t(&key, &[])));
    }

    // Initial state.
    let init_state = |s: State<'_>| {
        s.set_version(linkspec_engine::APP_VERSION.into());
        s.set_tech_details(settings.technical_details);
        s.set_hide_empty(settings.hide_empty_ports);
        s.set_start_at_login(settings.start_at_login);
        s.set_window_mode(settings.mode == UiMode::Window);
        s.set_usb_probe(settings.usb_probe);
        s.set_notifications(settings.notifications);
        s.set_update_check(settings.update_check);
        s.set_keep_open(settings.keep_window_open);
        s.set_font_scale(settings.font_scale);
        s.set_language(match settings.language {
            None => 0,
            Some(Lang::De) => 1,
            Some(Lang::En) => 2,
        });
    };
    init_state(main_window.global::<State>());
    init_state(flyout.global::<State>());

    let source = match open_source(fake.as_deref()) {
        Ok(s) => s,
        Err(e) => {
            let msg = match &e {
                linkspec_platform::SourceError::Unsupported(
                    linkspec_platform::Unsupported::IntelMac,
                ) => i18n.borrow().t("cli-unsupported-intel-mac", &[]),
                other => i18n
                    .borrow()
                    .t("cli-unsupported", &[("reason", other.to_string().into())]),
            };
            for s in [main_window.global::<State>(), flyout.global::<State>()] {
                s.set_no_ports_text(msg.clone().into());
            }
            tracing::error!("{e}");
            let _ = main_window.show();
            let _ = slint::run_event_loop();
            return;
        }
    };
    let engine = Engine::new(
        source,
        EngineOptions {
            lang,
            usb_probe: settings.usb_probe,
            include_raw: false,
            record_history: true,
        },
    )
    .expect("engine");
    let vendor_line = {
        let info = engine.catalog().info();
        i18n.borrow().t(
            "set-vendor-db",
            &[
                ("version", info.usb_ids_version.into()),
                ("license", info.usb_ids_license.into()),
                ("count", info.vendor_count.into()),
            ],
        )
    };
    for s in [main_window.global::<State>(), flyout.global::<State>()] {
        s.set_vendor_db(vendor_line.clone().into());
        s.set_no_ports_text(i18n.borrow().t("cli-no-ports", &[]).into());
    }

    // Worker → UI.
    let mw_weak = main_window.as_weak();
    let fo_weak = flyout.as_weak();
    let notifications_on = Rc::new(RefCell::new(settings.notifications));
    let hide_empty = Rc::new(RefCell::new(settings.hide_empty_ports));
    let (status_tx, status_rx) = std::sync::mpsc::channel::<Status>();
    let notif_flag =
        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(settings.notifications));
    let notif_flag_w = notif_flag.clone();
    let hide_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
        settings.hide_empty_ports,
    ));
    let hide_flag_w = hide_flag.clone();
    let cmd = worker::spawn(engine, move |u| {
        let mw = mw_weak.clone();
        let fo = fo_weak.clone();
        let status_tx = status_tx.clone();
        let notif = notif_flag_w.load(std::sync::atomic::Ordering::Relaxed);
        let hide = hide_flag_w.load(std::sync::atomic::Ordering::Relaxed);
        if let Update::Fault { title, body } = &u {
            if notif {
                platform::notify(title, body);
            }
            return;
        }
        if let Update::Report(url) = &u {
            if let Some(url) = url {
                platform::open_url(url);
            }
            return;
        }
        let _ = slint::invoke_from_event_loop(move || {
            let (Some(mw), Some(fo)) = (mw.upgrade(), fo.upgrade()) else {
                return;
            };
            apply(&mw.global::<State>(), &u, hide);
            apply(&fo.global::<State>(), &u, hide);
            if let Update::Data { worst, widget, .. } = &u {
                let _ = status_tx.send(*worst);
                platform::write_widget_snapshot(widget);
            }
        });
    });
    let _ = (notifications_on, hide_empty);

    // UI → worker.
    let wire = |s: State<'_>| {
        let c = cmd.clone();
        s.on_refresh(move || {
            let _ = c.send(Command::Refresh);
        });
        let c = cmd.clone();
        s.on_dismiss(move |id, kind| {
            let k = if kind == "overcurrent" {
                FaultKind::Overcurrent
            } else {
                FaultKind::DropReconnect
            };
            let _ = c.send(Command::Dismiss(id.to_string(), k));
        });
        let c = cmd.clone();
        s.on_rename_cable(move |fp, name| {
            let _ = c.send(Command::Rename(fp.to_string(), name.to_string()));
        });
        let c = cmd.clone();
        s.on_forget_cable(move |fp| {
            let _ = c.send(Command::Forget(fp.to_string()));
        });
        let c = cmd.clone();
        s.on_report(move || {
            let _ = c.send(Command::Report);
        });
        s.on_open_url(|u| platform::open_url(&u));
        let c = cmd.clone();
        s.on_monitor_visible(move |on| {
            let _ = c.send(Command::Monitor(on));
        });
    };
    wire(main_window.global::<State>());
    wire(flyout.global::<State>());

    // Open the window at a tab (from flyout or menu).
    let open_tab = {
        let mw = main_window.as_weak();
        let fo = flyout.as_weak();
        let c = cmd.clone();
        Rc::new(move |tab: i32| {
            if let Some(mw) = mw.upgrade() {
                mw.global::<State>().set_current_tab(tab);
                let _ = c.send(Command::Monitor(tab == TAB_POWER));
                platform::set_dock_icon(true);
                let _ = mw.show();
            }
            if let Some(fo) = fo.upgrade()
                && !fo.global::<State>().get_keep_open()
            {
                let _ = fo.hide();
            }
        })
    };
    {
        let o = open_tab.clone();
        flyout.global::<State>().on_open_window(move |t| o(t));
        let o = open_tab.clone();
        main_window.global::<State>().on_open_window(move |t| o(t));
    }
    // Monitor only while the power tab is visible.
    {
        let c = cmd.clone();
        let mw = main_window.as_weak();
        let last_tab = Rc::new(RefCell::new(-1));
        let timer = slint::Timer::default();
        timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(300),
            move || {
                if let Some(mw) = mw.upgrade() {
                    let visible = mw.window().is_visible();
                    let tab = if visible {
                        mw.global::<State>().get_current_tab()
                    } else {
                        -1
                    };
                    if *last_tab.borrow() != tab {
                        *last_tab.borrow_mut() = tab;
                        let _ = c.send(Command::Monitor(tab == TAB_POWER));
                    }
                }
            },
        );
        std::mem::forget(timer);
    }

    // Settings.
    let settings_rc = Rc::new(RefCell::new(settings.clone()));
    let on_setting = {
        let mw = main_window.as_weak();
        let fo = flyout.as_weak();
        let c = cmd.clone();
        let i18n = i18n.clone();
        let st = settings_rc.clone();
        let notif_flag = notif_flag.clone();
        let hide_flag = hide_flag.clone();
        Rc::new(move |source_is_main: bool, key: SharedString| {
            let (Some(mw), Some(fo)) = (mw.upgrade(), fo.upgrade()) else {
                return;
            };
            let src = if source_is_main {
                mw.global::<State>()
            } else {
                fo.global::<State>()
            };
            let mut s = st.borrow_mut();
            s.hide_empty_ports = src.get_hide_empty();
            s.start_at_login = src.get_start_at_login();
            s.mode = if src.get_window_mode() {
                UiMode::Window
            } else {
                UiMode::Tray
            };
            s.technical_details = src.get_tech_details();
            s.usb_probe = src.get_usb_probe();
            s.notifications = src.get_notifications();
            s.update_check = src.get_update_check();
            s.keep_window_open = src.get_keep_open();
            s.font_scale = src.get_font_scale();
            s.language = match src.get_language() {
                1 => Some(Lang::De),
                2 => Some(Lang::En),
                _ => None,
            };
            for other in [mw.global::<State>(), fo.global::<State>()] {
                other.set_hide_empty(s.hide_empty_ports);
                other.set_start_at_login(s.start_at_login);
                other.set_window_mode(s.mode == UiMode::Window);
                other.set_tech_details(s.technical_details);
                other.set_usb_probe(s.usb_probe);
                other.set_notifications(s.notifications);
                other.set_update_check(s.update_check);
                other.set_font_scale(s.font_scale);
                other.set_language(src.get_language());
            }
            notif_flag.store(s.notifications, std::sync::atomic::Ordering::Relaxed);
            hide_flag.store(s.hide_empty_ports, std::sync::atomic::Ordering::Relaxed);
            match key.as_str() {
                "start_at_login" => {
                    if let Err(e) = platform::set_autostart(s.start_at_login) {
                        tracing::warn!("autostart: {e}");
                    }
                }
                "usb_probe" => {
                    let _ = c.send(Command::SetProbe(s.usb_probe));
                }
                "hide_empty" => {
                    let _ = c.send(Command::Refresh);
                }
                "language" => {
                    let l = s.lang();
                    *i18n.borrow_mut() = I18n::new(l);
                    for t in [mw.global::<Tr>(), fo.global::<Tr>()] {
                        t.set_rev(t.get_rev() + 1);
                    }
                    let _ = c.send(Command::SetLang(l));
                }
                _ => {}
            }
            if let Err(e) = s.save() {
                tracing::warn!("settings: {e}");
            }
        })
    };
    {
        let f = on_setting.clone();
        main_window
            .global::<State>()
            .on_setting_changed(move |k| f(true, k));
        let f = on_setting.clone();
        flyout
            .global::<State>()
            .on_setting_changed(move |k| f(false, k));
    }

    // Update check (at most every 6 hours, never for package manager installs).
    let update_text = {
        let mw = main_window.as_weak();
        let fo = flyout.as_weak();
        let i18n = i18n.clone();
        Rc::new(move |force: bool| {
            if platform::packaged() {
                return;
            }
            let mw = mw.clone();
            let fo = fo.clone();
            let t_none = i18n.borrow().t("update-none", &[]);
            let t_new = i18n.borrow().t("update-available", &[]);
            let t_err = i18n.borrow().t("update-error", &[]);
            std::thread::spawn(move || {
                let text = match platform::check_update(force) {
                    Ok(Some((tag, url))) => format!("{t_new} {tag} – {url}"),
                    Ok(None) if force => t_none,
                    Ok(None) => return,
                    Err(e) => {
                        tracing::info!("update check: {e}");
                        if force { t_err } else { return }
                    }
                };
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(w) = mw.upgrade() {
                        w.global::<State>().set_update_text(text.clone().into());
                    }
                    if let Some(f) = fo.upgrade() {
                        f.global::<State>().set_update_text(text.into());
                    }
                });
            });
        })
    };
    {
        let u = update_text.clone();
        main_window
            .global::<State>()
            .on_check_updates(move || u(true));
        let u = update_text.clone();
        flyout.global::<State>().on_check_updates(move || u(true));
    }
    if settings.update_check {
        update_text(false);
    }

    // Quit.
    {
        let c = cmd.clone();
        main_window.global::<State>().on_quit(move || {
            let _ = c.send(Command::Quit);
            let _ = slint::quit_event_loop();
        });
    }

    // Tray.
    let tray: Rc<RefCell<Option<(TrayIcon, Menus)>>> = Rc::new(RefCell::new(None));
    if tray_wanted {
        let tray = tray.clone();
        let i18n = i18n.clone();
        let keep_open = settings.keep_window_open;
        slint::Timer::single_shot(std::time::Duration::from_millis(0), move || {
            let menus = Menus::new(&i18n.borrow(), keep_open);
            let template = cfg!(target_os = "macos");
            let icon = load_icon(
                include_bytes!("../../packaging/icons/tray-32.png"),
                template,
            );
            let mut b = TrayIconBuilder::new()
                .with_menu(Box::new(menus.menu()))
                .with_tooltip("LinkSpec")
                .with_menu_on_left_click(false);
            if let Some(i) = icon {
                // macOS draws the menu bar icon as a template image.
                #[cfg(target_os = "macos")]
                {
                    b = b.with_icon_templated(i);
                }
                #[cfg(not(target_os = "macos"))]
                {
                    b = b.with_icon(i);
                }
            }
            match b.build() {
                Ok(t) => *tray.borrow_mut() = Some((t, menus)),
                Err(e) => tracing::warn!("tray: {e}"),
            }
        });
        platform::set_dock_icon(false);
    } else {
        let _ = main_window.show();
    }

    // Tray and menu events (polled on the UI thread).
    {
        let tray = tray.clone();
        let fo = flyout.as_weak();
        let mw = main_window.as_weak();
        let open_tab = open_tab.clone();
        let c = cmd.clone();
        let i18n = i18n.clone();
        let st = settings_rc.clone();
        let update_text = update_text.clone();
        let last_lang = RefCell::new(lang);
        let timer = slint::Timer::default();
        timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(100),
            move || {
                // Tray icon tooltip follows the worst status.
                while let Ok(worst) = status_rx.try_recv() {
                    if let Some((t, _)) = tray.borrow().as_ref() {
                        let tip = match worst {
                            Status::Ok => "LinkSpec ✓",
                            Status::Warn => "LinkSpec !",
                            Status::Fault => "LinkSpec ✗",
                        };
                        let _ = t.set_tooltip(Some(tip));
                    }
                }
                let cur = st.borrow().lang();
                if *last_lang.borrow() != cur {
                    *last_lang.borrow_mut() = cur;
                    if let Some((_, m)) = tray.borrow().as_ref() {
                        m.relabel(&i18n.borrow());
                    }
                }
                while let Ok(ev) = TrayIconEvent::receiver().try_recv() {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        rect,
                        ..
                    } = ev
                        && let Some(fo) = fo.upgrade()
                    {
                        if fo.window().is_visible() {
                            let _ = fo.hide();
                        } else {
                            let size = fo.window().size();
                            let x = rect.position.x as i32 + (rect.size.width as i32 / 2)
                                - (size.width as i32 / 2);
                            let y = if cfg!(windows) {
                                rect.position.y as i32 - size.height as i32 - 8
                            } else {
                                rect.position.y as i32 + rect.size.height as i32 + 4
                            };
                            fo.window()
                                .set_position(slint::PhysicalPosition::new(x.max(0), y.max(0)));
                            let _ = fo.show();
                        }
                    }
                }
                while let Ok(ev) = MenuEvent::receiver().try_recv() {
                    let tb = tray.borrow();
                    let Some((_, menus)) = tb.as_ref() else {
                        continue;
                    };
                    let id = ev.id;
                    if id == *menus.refresh.id() {
                        let _ = c.send(Command::Refresh);
                    } else if id == *menus.keep_open.id() {
                        let on = menus.keep_open.is_checked();
                        if let Some(fo) = fo.upgrade() {
                            fo.global::<State>().set_keep_open(on);
                        }
                        if let Some(mw) = mw.upgrade() {
                            mw.global::<State>().set_keep_open(on);
                        }
                        st.borrow_mut().keep_window_open = on;
                        let _ = st.borrow().save();
                    } else if id == *menus.settings.id() {
                        open_tab(5);
                    } else if id == *menus.power.id() {
                        open_tab(1);
                    } else if id == *menus.negotiation.id() {
                        open_tab(2);
                    } else if id == *menus.display.id() {
                        open_tab(3);
                    } else if id == *menus.cables.id() {
                        open_tab(4);
                    } else if id == *menus.about.id() {
                        open_tab(6);
                    } else if id == *menus.updates.id() {
                        open_tab(6);
                        update_text(true);
                    } else if id == *menus.report.id() {
                        let _ = c.send(Command::Report);
                    } else if id == *menus.github.id() {
                        platform::open_url("https://github.com/itsh-neumeier/linkspec");
                    } else if id == *menus.quit.id() {
                        let _ = c.send(Command::Quit);
                        let _ = slint::quit_event_loop();
                    }
                }
            },
        );
        std::mem::forget(timer);
    }

    // Hide the flyout when it loses focus (unless "keep open").
    {
        let fo = flyout.as_weak();
        flyout.window().on_close_requested(move || {
            if let Some(fo) = fo.upgrade() {
                let _ = fo.hide();
            }
            slint::CloseRequestResponse::HideWindow
        });
    }
    {
        let tray_mode = tray_wanted;
        main_window.window().on_close_requested(move || {
            if !tray_mode {
                let _ = slint::quit_event_loop();
            } else {
                platform::set_dock_icon(false);
            }
            slint::CloseRequestResponse::HideWindow
        });
    }

    // Start tab (screenshots, deep links): --tab N.
    if let Some(tab) = args
        .iter()
        .position(|a| a == "--tab")
        .and_then(|i| args.get(i + 1))
        .and_then(|t| t.parse::<i32>().ok())
    {
        main_window.global::<State>().set_current_tab(tab);
    }

    // Performance measurement: quit after the first rendered frame.
    if std::env::var_os("LINKSPEC_PERF_EXIT_AFTER_FIRST_FRAME").is_some() {
        let _ = main_window.show();
        let _ = main_window.window().set_rendering_notifier(|state, _| {
            if matches!(state, slint::RenderingState::AfterRendering) {
                let _ = slint::quit_event_loop();
            }
        });
    }

    let _ = slint::run_event_loop_until_quit();
    let _ = cmd.send(Command::Quit);
}

fn apply(s: &State<'_>, u: &Update, hide_empty: bool) {
    match u {
        Update::Data {
            ports,
            negotiation,
            displays,
            cables,
            service,
            ..
        } => {
            let ports: Vec<Port> = ports
                .iter()
                .filter(|p| !(hide_empty && p.empty))
                .map(|p| Port {
                    id: p.id.clone().into(),
                    label: p.label.clone().into(),
                    location: p.location.clone().into(),
                    headline: p.headline.clone().into(),
                    summary: p.summary.clone().into(),
                    status: p.status,
                    empty: p.empty,
                    cable: p.cable.clone().into(),
                    power: p.power.clone().into(),
                    verdicts: ModelRc::new(VecModel::from(
                        p.verdicts
                            .iter()
                            .map(|v| Verdict {
                                domain: v.domain.clone().into(),
                                text: v.text.clone().into(),
                                warn: v.warn,
                            })
                            .collect::<Vec<_>>(),
                    )),
                    banners: ModelRc::new(VecModel::from(
                        p.banners
                            .iter()
                            .map(|(k, t)| Banner {
                                kind: k.clone().into(),
                                text: t.clone().into(),
                            })
                            .collect::<Vec<_>>(),
                    )),
                    devices: ModelRc::new(VecModel::from(
                        p.devices
                            .iter()
                            .map(|(t, d)| Device {
                                text: t.clone().into(),
                                depth: *d,
                            })
                            .collect::<Vec<_>>(),
                    )),
                    notes: ModelRc::new(VecModel::from(
                        p.notes
                            .iter()
                            .map(|n| SharedString::from(n.as_str()))
                            .collect::<Vec<_>>(),
                    )),
                    details: ModelRc::new(VecModel::from(
                        p.details
                            .iter()
                            .map(|n| SharedString::from(n.as_str()))
                            .collect::<Vec<_>>(),
                    )),
                })
                .collect();
            s.set_ports(ModelRc::new(VecModel::from(ports)));
            s.set_negotiation(ModelRc::new(VecModel::from(
                negotiation
                    .iter()
                    .map(|n| NegRow {
                        port_label: n.port_label.clone().into(),
                        domain: n.domain.clone().into(),
                        port: n.port.clone().into(),
                        cable: n.cable.clone().into(),
                        device: n.device.clone().into(),
                        negotiated: n.negotiated.clone().into(),
                        weakest: n.weakest,
                    })
                    .collect::<Vec<_>>(),
            )));
            s.set_displays(ModelRc::new(VecModel::from(
                displays
                    .iter()
                    .map(|d| DisplayRow {
                        port_label: d.port_label.clone().into(),
                        name: d.name.clone().into(),
                        current: d.current.clone().into(),
                        max: d.max.clone().into(),
                        link: d.link.clone().into(),
                        verdict: d.verdict.clone().into(),
                    })
                    .collect::<Vec<_>>(),
            )));
            s.set_cables(ModelRc::new(VecModel::from(
                cables
                    .iter()
                    .map(|c| CableRow {
                        fingerprint: c.fingerprint.clone().into(),
                        name: c.name.clone().unwrap_or_default().into(),
                        summary: c.summary.clone().into(),
                        vidpid: format!("{:04x}:{:04x}", c.vid, c.pid).into(),
                        uses: c.uses as i32,
                        faults: c.faults as i32,
                        first_seen: c.first_seen.get(..10).unwrap_or(&c.first_seen).into(),
                        last_seen: c.last_seen.get(..10).unwrap_or(&c.last_seen).into(),
                        ok: c.faults == 0,
                    })
                    .collect::<Vec<_>>(),
            )));
            s.set_service_status(service.clone().into());
            let _ = s.get_ports().row_count();
        }
        Update::Monitor { now, path, note } => {
            s.set_monitor_now(now.clone().into());
            s.set_monitor_path(path.clone().into());
            s.set_monitor_note(note.clone().into());
        }
        Update::Fault { .. } | Update::Report(_) => {}
    }
}

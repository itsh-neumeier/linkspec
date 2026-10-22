//! `--dashboard`: full-screen TUI with overview, negotiation and power.
//! Tab switches the view, `q` quits.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use linkspec_core::diagnosis::Domain;
use linkspec_core::negotiation;
use linkspec_engine::{Engine, RunResult};
use linkspec_platform::ChangeEvent;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Row, Sparkline, Table, Tabs, Wrap};

use crate::render;

pub fn run(engine: &mut Engine) -> std::io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;
    let res = run_loop(&mut terminal, engine);
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    res
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    engine: &mut Engine,
) -> std::io::Result<()> {
    let watch = engine.source_mut().watch().ok();
    let mut tab = 0usize;
    let mut result: Option<RunResult> = engine.run().ok();
    let mut samples: VecDeque<(f64, Option<f64>, Option<f64>)> = VecDeque::new();
    let mut last_sample = Instant::now() - Duration::from_secs(2);
    let mut scroll: u16 = 0;
    loop {
        if let Some(rx) = &watch {
            let mut changed = false;
            while let Ok(ev) = rx.try_recv() {
                changed |= ev == ChangeEvent::Changed;
            }
            if changed && let Ok(r) = engine.run() {
                result = Some(r);
            }
        }
        if tab == 2 && last_sample.elapsed() >= Duration::from_secs(1) {
            last_sample = Instant::now();
            if let Ok(Some(s)) = engine.source_mut().power_sample()
                && let Some(w) = s.watts
            {
                samples.push_back((w, s.volts, s.amps));
                if samples.len() > 300 {
                    samples.pop_front();
                }
            }
        }
        let i18n = engine.i18n();
        let titles = [
            i18n.t("tui-tab-overview", &[]),
            i18n.t("tui-tab-negotiation", &[]),
            i18n.t("tui-tab-power", &[]),
        ];
        let help = i18n.t("tui-help", &[]);
        terminal.draw(|f| {
            let [head, body, foot] = Layout::vertical([
                Constraint::Length(3),
                Constraint::Min(3),
                Constraint::Length(1),
            ])
            .areas(f.area());
            let tabs = Tabs::new(titles.iter().map(|t| Line::from(t.as_str())))
                .select(tab)
                .block(Block::default().borders(Borders::ALL).title(" LinkSpec "))
                .highlight_style(
                    Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED),
                );
            f.render_widget(tabs, head);
            f.render_widget(Paragraph::new(help.as_str()), foot);
            let Some(r) = &result else {
                return;
            };
            match tab {
                0 => {
                    let text = render::text(
                        &r.diagnosis,
                        i18n,
                        render::Style { color: false },
                        false,
                        false,
                    );
                    let p = Paragraph::new(text)
                        .wrap(Wrap { trim: false })
                        .scroll((scroll, 0))
                        .block(Block::default().borders(Borders::ALL));
                    f.render_widget(p, body);
                }
                1 => {
                    let mut rows = Vec::new();
                    for (obs, d) in r.snapshot.ports.iter().zip(&r.diagnosis.ports) {
                        let neg = negotiation::rows(obs, d, i18n);
                        if neg.is_empty() {
                            continue;
                        }
                        rows.push(
                            Row::new(vec![d.label.clone()])
                                .style(Style::default().add_modifier(Modifier::BOLD)),
                        );
                        for n in neg {
                            let dom = i18n.t(
                                match n.domain {
                                    Domain::Power => "cli-verdict-power",
                                    Domain::Data => "cli-verdict-data",
                                    Domain::Display => "cli-verdict-display",
                                },
                                &[],
                            );
                            let cell = |col: &str, v: Option<String>| {
                                let s = v.unwrap_or_else(|| "–".into());
                                if n.weakest == Some(col) {
                                    Span::styled(
                                        format!("▶ {s}"),
                                        Style::default()
                                            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                                    )
                                } else {
                                    Span::raw(s)
                                }
                            };
                            rows.push(Row::new(vec![
                                Line::from(format!("  {dom}")),
                                Line::from(cell("port", n.port)),
                                Line::from(cell("cable", n.cable)),
                                Line::from(cell("device", n.device)),
                                Line::from(n.negotiated.unwrap_or_else(|| "–".into())),
                            ]));
                        }
                    }
                    let header = Row::new(vec![
                        String::new(),
                        i18n.t("neg-port", &[]),
                        i18n.t("neg-cable", &[]),
                        i18n.t("neg-device", &[]),
                        i18n.t("neg-negotiated", &[]),
                    ])
                    .style(Style::default().add_modifier(Modifier::BOLD));
                    let t = Table::new(rows, [Constraint::Percentage(20); 5])
                        .header(header)
                        .block(Block::default().borders(Borders::ALL));
                    f.render_widget(t, body);
                }
                _ => {
                    let [top, graph] =
                        Layout::vertical([Constraint::Length(3), Constraint::Min(3)]).areas(body);
                    let line = match samples.back() {
                        Some((w, v, a)) => format!(
                            "{} W   {} V   {} A",
                            i18n.num(*w),
                            v.map(|x| i18n.num(x)).unwrap_or_else(|| "–".into()),
                            a.map(|x| i18n.num(x)).unwrap_or_else(|| "–".into())
                        ),
                        None => i18n.t("cli-monitor-none", &[]),
                    };
                    f.render_widget(
                        Paragraph::new(line).block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title(i18n.t("cli-monitor-header", &[])),
                        ),
                        top,
                    );
                    let data: Vec<u64> = samples
                        .iter()
                        .map(|(w, _, _)| (w.abs() * 10.0) as u64)
                        .collect();
                    f.render_widget(
                        Sparkline::default()
                            .data(&data)
                            .block(Block::default().borders(Borders::ALL)),
                        graph,
                    );
                }
            }
        })?;
        if event::poll(Duration::from_millis(250))?
            && let Event::Key(k) = event::read()?
            && k.kind == KeyEventKind::Press
        {
            match k.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => return Ok(()),
                KeyCode::Tab | KeyCode::Right => {
                    tab = (tab + 1) % 3;
                    scroll = 0;
                }
                KeyCode::BackTab | KeyCode::Left => {
                    tab = (tab + 2) % 3;
                    scroll = 0;
                }
                KeyCode::Down => scroll = scroll.saturating_add(1),
                KeyCode::Up => scroll = scroll.saturating_sub(1),
                KeyCode::Char('r') => {
                    if let Ok(r) = engine.run() {
                        result = Some(r);
                    }
                }
                _ => {}
            }
        }
    }
}

use clap::Args;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode,
        KeyEventKind,
    },
    execute,
    terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode,
    },
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{
        Axis, BarChart, Block, Borders, Chart, Dataset, GraphType, List,
        ListItem, Paragraph, Tabs,
    },
};
use std::{
    fs,
    io::{self, Stdout},
    path::{Path, PathBuf},
    rc::Rc,
    time::Duration,
};

// ─── CLI ─────────────────────────────────────────────────────────────────────

#[derive(Args)]
/// Interactive TUI stats viewer for JSON score files
pub(crate) struct SubArgs {
    /// Directory containing the JSON files
    #[arg(long)]
    dir: PathBuf,
    /// Maximum depth for recursive file search (0 = current dir only)
    #[arg(short, long, default_value_t = 0)]
    depth: usize,
    /// JSON field name to read as the score
    #[arg(short, long, default_value = "score")]
    field: String,
    /// Number of histogram buckets
    #[arg(short = 'b', long, default_value_t = 10)]
    buckets: usize,
}

// ─── DATA ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct Stats {
    scores: Vec<f64>,
    filenames: Vec<String>,
    count: usize,
    min: f64,
    max: f64,
    range: f64,
    mean: f64,
    median: f64,
    std_dev: f64,
    variance: f64,
    p25: f64,
    p75: f64,
    iqr: f64,
    histogram: Vec<(String, u64)>,
}

fn extract_score(value: &serde_json::Value, field: &str) -> Option<f64> {
    value.get(field)?.as_f64()
}

fn collect_json_files(
    root: &Path,
    current_depth: usize,
    max_depth: usize,
) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return files;
    };
    for entry in entries.flatten() {
        let path: PathBuf = entry.path();
        if path.is_dir() && current_depth < max_depth {
            files.extend(collect_json_files(
                &path,
                current_depth + 1,
                max_depth,
            ));
        } else if path.extension().is_some_and(|e| e == "json") {
            files.push(path);
        }
    }
    files
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx: f64 = p / 100.0 * (sorted.len() - 1) as f64;
    let lo: usize = idx.floor() as usize;
    let hi: usize = idx.ceil() as usize;
    sorted[lo] * (1.0 - (idx - lo as f64)) + sorted[hi] * (idx - lo as f64)
}

fn build_histogram(
    scores: &[f64],
    min: f64,
    max: f64,
    buckets: usize,
) -> Vec<(String, u64)> {
    let mut counts: Vec<u64> = vec![0u64; buckets];
    let range: f64 = if (max - min).abs() < f64::EPSILON {
        1.0
    } else {
        max - min
    };
    for &s in scores {
        let idx: usize = ((s - min) / range * buckets as f64).floor() as usize;
        counts[idx.min(buckets - 1)] += 1;
    }
    let bsize: f64 = range / buckets as f64;
    counts
        .into_iter()
        .enumerate()
        .map(|(i, c)| {
            let lo = min + i as f64 * bsize;
            (format!("{lo:.1}-{:.1}", lo + bsize), c)
        })
        .collect()
}

fn compute_stats(
    dir: &Path,
    field: &str,
    max_depth: usize,
    buckets: usize,
) -> anyhow::Result<Stats> {
    let files: Vec<PathBuf> = collect_json_files(dir, 0, max_depth);
    let mut pairs: Vec<(String, f64)> = files
        .iter()
        .filter_map(|path| {
            let text: String = fs::read_to_string(path).ok()?;
            let json: serde_json::Value = serde_json::from_str(&text).ok()?;
            let score: f64 = extract_score(&json, field)?;
            let name: String = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            Some((name, score))
        })
        .collect();

    pairs.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    let count: usize = pairs.len();
    if count == 0 {
        return Err(anyhow::anyhow!(
            "No JSON files with a valid '{}' field found in: {}",
            field,
            dir.display()
        ));
    }

    let filenames: Vec<String> = pairs.iter().map(|(n, _)| n.clone()).collect();
    let scores: Vec<f64> = pairs.iter().map(|(_, s)| *s).collect();
    let min: f64 = scores[0];
    let max: f64 = scores[count - 1];
    let range: f64 = max - min;
    let mean: f64 = scores.iter().sum::<f64>() / count as f64;
    let median: f64 = if count.is_multiple_of(2) {
        (scores[count / 2 - 1] + scores[count / 2]) / 2.0
    } else {
        scores[count / 2]
    };
    let variance: f64 =
        scores.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / count as f64;
    let std_dev: f64 = variance.sqrt();
    let p25: f64 = percentile(&scores, 25.0);
    let p75: f64 = percentile(&scores, 75.0);
    let iqr: f64 = p75 - p25;
    let histogram: Vec<(String, u64)> =
        build_histogram(&scores, min, max, buckets);

    Ok(Stats {
        scores,
        filenames,
        count,
        min,
        max,
        range,
        mean,
        median,
        std_dev,
        variance,
        p25,
        p75,
        iqr,
        histogram,
    })
}

// ─── TUI APP ──────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Summary = 0,
    Histogram = 1,
    Distribution = 2,
    Files = 3,
}

impl Tab {
    fn next(self) -> Self {
        match self {
            Tab::Summary => Tab::Histogram,
            Tab::Histogram => Tab::Distribution,
            Tab::Distribution => Tab::Files,
            Tab::Files => Tab::Summary,
        }
    }
    fn prev(self) -> Self {
        match self {
            Tab::Summary => Tab::Files,
            Tab::Histogram => Tab::Summary,
            Tab::Distribution => Tab::Histogram,
            Tab::Files => Tab::Distribution,
        }
    }
}

struct App {
    stats: Stats,
    active_tab: Tab,
    file_scroll: usize,
    field: String,
    dir: String,
}

impl App {
    fn new(stats: Stats, field: String, dir: String) -> Self {
        Self {
            stats,
            active_tab: Tab::Summary,
            file_scroll: 0,
            field,
            dir,
        }
    }
    fn scroll_down(&mut self) {
        if self.active_tab == Tab::Files
            && self.file_scroll < self.stats.count.saturating_sub(1)
        {
            self.file_scroll += 1;
        }
    }
    fn scroll_up(&mut self) {
        if self.active_tab == Tab::Files && self.file_scroll > 0 {
            self.file_scroll -= 1;
        }
    }
}

fn ui(frame: &mut Frame, app: &App) {
    let area: Rect = frame.area();
    let root: Rc<[Rect]> = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);
    draw_title(frame, root[0], app);
    draw_tabs(frame, root[1], app);
    draw_help(frame, root[3]);
    match app.active_tab {
        Tab::Summary => draw_summary(frame, root[2], app),
        Tab::Histogram => draw_histogram(frame, root[2], app),
        Tab::Distribution => draw_distribution(frame, root[2], app),
        Tab::Files => draw_files(frame, root[2], app),
    }
}

fn draw_title(frame: &mut Frame, area: Rect, app: &App) {
    let title: Paragraph<'_> = Paragraph::new(Line::from(vec![
        Span::styled(
            " score_stats ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  {}  ", app.dir),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            format!("[field: {}]", app.field),
            Style::default().fg(Color::Yellow),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(title, area);
}

fn draw_tabs(frame: &mut Frame, area: Rect, app: &App) {
    let titles: Vec<Line> = [
        "  Summary  ",
        "  Histogram  ",
        "  Distribution  ",
        "  Files  ",
    ]
    .iter()
    .map(|t| Line::from(*t))
    .collect();
    let tabs = Tabs::new(titles)
        .select(app.active_tab as usize)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::UNDERLINED),
        )
        .divider(symbols::DOT);
    frame.render_widget(tabs, area);
}

fn draw_help(frame: &mut Frame, area: Rect) {
    let help: Paragraph<'_> = Paragraph::new(Line::from(vec![
        Span::styled(" ← →  ", Style::default().fg(Color::Yellow)),
        Span::raw("switch tab   "),
        Span::styled("↑ ↓  ", Style::default().fg(Color::Yellow)),
        Span::raw("scroll (Files tab)   "),
        Span::styled("q / Esc  ", Style::default().fg(Color::Yellow)),
        Span::raw("quit"),
    ]))
    .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, area);
}

fn stat_row(label: &str, value: &str) -> ListItem<'static> {
    ListItem::new(Line::from(vec![
        Span::styled(
            format!("  {:<14}", label),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            value.to_string(),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
    ]))
}

fn fmt_f(v: f64) -> String {
    format!("{:.4}", v)
}

fn draw_summary(frame: &mut Frame, area: Rect, app: &App) {
    let s: &Stats = &app.stats;
    let cols: Rc<[Rect]> = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let left: List<'_> = List::new(vec![
        stat_row("Count", &format!("{}", s.count)),
        stat_row("Mean", &fmt_f(s.mean)),
        stat_row("Median", &fmt_f(s.median)),
        stat_row("Std Dev", &fmt_f(s.std_dev)),
        stat_row("Variance", &fmt_f(s.variance)),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Central Tendency ")
            .title_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(left, cols[0]);

    let right: List<'_> = List::new(vec![
        stat_row("Min", &fmt_f(s.min)),
        stat_row("Max", &fmt_f(s.max)),
        stat_row("Range", &fmt_f(s.range)),
        stat_row("P25", &fmt_f(s.p25)),
        stat_row("P75", &fmt_f(s.p75)),
        stat_row("IQR", &fmt_f(s.iqr)),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Spread / Percentiles ")
            .title_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(right, cols[1]);
}

fn draw_histogram(frame: &mut Frame, area: Rect, app: &App) {
    let s: &Stats = &app.stats;
    let data: Vec<(&str, u64)> =
        s.histogram.iter().map(|(l, c)| (l.as_str(), *c)).collect();
    let max_count = s.histogram.iter().map(|(_, c)| *c).max().unwrap_or(1);
    let bar: BarChart<'_> = BarChart::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Histogram — {} buckets ", s.histogram.len()))
                .title_style(Style::default().fg(Color::Cyan)),
        )
        .data(&data)
        .bar_width(12)
        .bar_gap(1)
        .bar_style(Style::default().fg(Color::LightGreen))
        .value_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        )
        .label_style(Style::default().fg(Color::Yellow))
        .max(max_count);
    frame.render_widget(bar, area);
}

fn draw_distribution(frame: &mut Frame, area: Rect, app: &App) {
    let s: &Stats = &app.stats;
    let data: Vec<(f64, f64)> = s
        .scores
        .iter()
        .enumerate()
        .map(|(i, &v)| (i as f64, v))
        .collect();
    let mean_data: Vec<(f64, f64)> =
        vec![(0.0, s.mean), ((s.count.saturating_sub(1)) as f64, s.mean)];
    let median_data: Vec<(f64, f64)> = vec![
        (0.0, s.median),
        ((s.count.saturating_sub(1)) as f64, s.median),
    ];
    let x_max: f64 = (s.count as f64 - 1.0).max(1.0);
    let y_margin: f64 = s.range * 0.05 + 0.001;

    let chart: Chart<'_> = Chart::new(vec![
        Dataset::default()
            .name("scores (sorted)")
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::LightCyan))
            .data(&data),
        Dataset::default()
            .name(format!("mean ({:.4})", s.mean))
            .marker(symbols::Marker::Dot)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::Yellow))
            .data(&mean_data),
        Dataset::default()
            .name(format!("median ({:.4})", s.median))
            .marker(symbols::Marker::Dot)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::Magenta))
            .data(&median_data),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Distribution (sorted scores) ")
            .title_style(Style::default().fg(Color::Cyan)),
    )
    .x_axis(
        Axis::default()
            .title("file index (sorted)")
            .style(Style::default().fg(Color::DarkGray))
            .bounds([0.0, x_max])
            .labels(vec![
                Span::raw("0"),
                Span::raw(format!("{}", s.count / 2)),
                Span::raw(format!("{}", s.count)),
            ]),
    )
    .y_axis(
        Axis::default()
            .title("score")
            .style(Style::default().fg(Color::DarkGray))
            .bounds([s.min - y_margin, s.max + y_margin])
            .labels(vec![
                Span::raw(fmt_f(s.min)),
                Span::raw(fmt_f(s.mean)),
                Span::raw(fmt_f(s.max)),
            ]),
    );
    frame.render_widget(chart, area);
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max - 1])
    }
}

fn draw_files(frame: &mut Frame, area: Rect, app: &App) {
    let s: &Stats = &app.stats;
    let visible: usize = area.height.saturating_sub(2) as usize;
    let items: Vec<ListItem> = s
        .scores
        .iter()
        .enumerate()
        .skip(app.file_scroll)
        .take(visible)
        .map(|(i, score)| {
            let name = &s.filenames[i];
            let pct = if s.range.abs() < f64::EPSILON {
                0.5
            } else {
                (score - s.min) / s.range
            };
            let bar_len: usize = (pct * 20.0).round() as usize;
            let bar: String = "█".repeat(bar_len) + &"░".repeat(20 - bar_len);
            let color: Color = if pct < 0.33 {
                Color::Red
            } else if pct < 0.66 {
                Color::Yellow
            } else {
                Color::Green
            };
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{:>4}. ", i + 1),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("{:<30}", truncate(name, 30)),
                    Style::default().fg(Color::White),
                ),
                Span::styled(
                    format!("  {:>10.4}  ", score),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(bar, Style::default().fg(color)),
            ]))
        })
        .collect();
    let list: List<'_> = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" Files ({} total) — ↑↓ scroll ", s.count))
            .title_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(list, area);
}

// ─── MAIN ─────────────────────────────────────────────────────────────────────

pub(crate) fn run(args: SubArgs) -> anyhow::Result<()> {
    let dir: PathBuf = args.dir.canonicalize().unwrap_or(args.dir.clone());
    let dir_str: String = dir.to_string_lossy().into_owned();
    let stats: Stats =
        compute_stats(&dir, &args.field, args.depth, args.buckets.max(1))?;

    enable_raw_mode()?;
    let mut stdout: Stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend: CrosstermBackend<Stdout> = CrosstermBackend::new(stdout);
    let mut terminal: Terminal<CrosstermBackend<Stdout>> =
        Terminal::new(backend)?;
    let mut app: App = App::new(stats, args.field, dir_str);

    loop {
        terminal.draw(|f| ui(f, &app))?;
        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
        {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Right | KeyCode::Tab => {
                    app.active_tab = app.active_tab.next()
                }
                KeyCode::Left | KeyCode::BackTab => {
                    app.active_tab = app.active_tab.prev()
                }
                KeyCode::Down | KeyCode::Char('j') => app.scroll_down(),
                KeyCode::Up | KeyCode::Char('k') => app.scroll_up(),
                _ => {}
            }
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    let s: &Stats = &app.stats;
    println!("\n── score_stats ──────────────────────────────");
    println!("  Directory : {}", app.dir);
    println!("  Field     : {}", app.field);
    println!("  Files     : {}", s.count);
    println!("  Mean      : {:.4}", s.mean);
    println!("  Median    : {:.4}", s.median);
    println!("  Std Dev   : {:.4}", s.std_dev);
    println!("  Min / Max : {:.4} / {:.4}", s.min, s.max);
    println!("─────────────────────────────────────────────\n");
    Ok(())
}

//! Terminal renderer - draws UI to terminal.

use crate::application::{ActivePane, AppState, PanelMode, PanelState, QuickViewMode};
use crate::domain::FileEntry;
use crossterm::style::{Color, Print, StyledContent, Stylize};
use crossterm::terminal::{ClearType, enable_raw_mode};
use crossterm::{cursor, queue, terminal};
use std::io::Write;

pub const HEADER_ROWS: u16 = 2;
pub const FOOTER_ROWS: u16 = 3;

/// Embedded SVG logo bytes (loaded at compile time)
const LOGO_SVG: &[u8] = include_bytes!("../../../Fishez_logo.svg");

pub struct TerminalRenderer {
    pub columns: u16,
    pub rows: u16,
    pub stdout: std::io::Stdout,
    help_entries: Vec<(String, String)>,
    logo_png: Option<Vec<u8>>,
}

impl Default for TerminalRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalRenderer {
    pub fn new() -> Self {
        let (columns, rows) = terminal::size().expect("Error getting terminal size");
        enable_raw_mode().expect("Failed to enable raw mode");

        // Pre-render SVG logo to PNG bytes
        let logo_png = render_svg_to_png(LOGO_SVG);

        Self {
            columns,
            rows,
            stdout: std::io::stdout(),
            help_entries: default_help_entries(),
            logo_png,
        }
    }

    pub fn update_size(&mut self, cols: u16, rows: u16) {
        self.columns = cols;
        self.rows = rows;
    }
    pub fn visible_rows(&self) -> u16 {
        self.rows.saturating_sub(HEADER_ROWS + FOOTER_ROWS)
    }

    pub fn reset_terminal(&mut self) {
        let _ = queue!(
            &self.stdout,
            cursor::MoveTo(0, 0),
            cursor::Show,
            cursor::EnableBlinking,
            terminal::Clear(ClearType::All)
        );
        let _ = terminal::disable_raw_mode();
        let _ = self.stdout.flush();
    }

    pub fn draw(&mut self, state: &AppState) {
        let panel = state.active_panel();
        let _ = queue!(&self.stdout, cursor::DisableBlinking, cursor::Hide);
        self.draw_header(panel);
        self.draw_system_header(panel);
        match &panel.mode {
            PanelMode::Normal | PanelMode::Filter => self.draw_files_list(panel),
            PanelMode::QuickView(qv) => self.draw_quick_view(qv),
        }
        self.draw_footer(panel);
        self.draw_footer_actions(&panel.mode);
        if state.show_help {
            self.draw_help_overlay();
        }
        let _ = self.stdout.flush();
    }

    pub fn draw_two_panes(&mut self, state: &AppState) {
        let _ = queue!(&self.stdout, cursor::DisableBlinking, cursor::Hide);
        if matches!(state.active_panel().mode, PanelMode::QuickView(_)) {
            self.draw(state);
            return;
        }
        self.draw_dual_header(&state.left_panel, &state.right_panel, state.active_pane);
        self.draw_system_header(state.active_panel());
        self.draw_files_two_panes(&state.left_panel, &state.right_panel, state.active_pane);
        self.draw_footer(state.active_panel());
        self.draw_footer_actions(&state.active_panel().mode);
        if state.show_help {
            self.draw_help_overlay();
        }
        let _ = self.stdout.flush();
    }

    fn draw_header(&self, panel: &PanelState) {
        let left = if matches!(panel.mode, PanelMode::QuickView(_)) {
            if panel.cursor < panel.entries.len() {
                format!("Viewing: {}", &panel.entries[panel.cursor].name).with(Color::Cyan)
            } else {
                "Viewing".to_string().with(Color::Cyan)
            }
        } else {
            format!("PWD: {}", panel.current_path.display()).with(Color::Cyan)
        };
        let _ = queue!(
            &self.stdout,
            cursor::MoveTo(0, 0),
            Print(left),
            terminal::Clear(ClearType::UntilNewLine)
        );
    }

    fn draw_dual_header(&self, left: &PanelState, right: &PanelState, active: ActivePane) {
        let _ = queue!(
            &self.stdout,
            cursor::MoveTo(0, 0),
            terminal::Clear(ClearType::UntilNewLine)
        );
        let pw = self.columns.saturating_sub(1) / 2;
        let (ll, rl) = match active {
            ActivePane::Left => ("*L", " R"),
            ActivePane::Right => (" L", "*R"),
        };
        let lt = truncate(
            &format!("{}: {}", ll, left.current_path.display()),
            pw as usize,
        );
        let rt = truncate(
            &format!("{}: {}", rl, right.current_path.display()),
            pw as usize,
        );
        let _ = queue!(
            &self.stdout,
            cursor::MoveTo(0, 0),
            Print(lt.with(Color::Cyan)),
            cursor::MoveTo(pw + 1, 0),
            Print(rt.with(Color::Cyan))
        );
    }

    fn draw_system_header(&self, panel: &PanelState) {
        let mut right = panel
            .notification
            .clone()
            .map(|n| n.on(Color::DarkMagenta))
            .unwrap_or_else(|| "FISHEZ".to_string().with(Color::Cyan));
        let max_width = self.columns as usize;
        if right.content().len() > max_width {
            let truncated: String = right.content().chars().take(max_width).collect();
            right = right.style().apply(truncated);
        }
        let _ = queue!(
            &self.stdout,
            cursor::MoveTo(self.columns.saturating_sub(right.content().len() as u16), 0),
            Print(right)
        );
        self.draw_line(1);
    }

    fn draw_files_list(&mut self, panel: &PanelState) {
        let rows = self.visible_rows();
        let _ = queue!(&self.stdout, cursor::MoveTo(0, HEADER_ROWS));
        let start = panel.scroll.min(panel.entries.len());
        for (i, entry) in panel.entries[start..]
            .iter()
            .take(rows as usize)
            .enumerate()
        {
            let name = style_entry(entry);
            let styled = if i == panel.cursor - panel.scroll {
                name.negative()
            } else {
                name
            };
            let final_name = if panel.is_multi_selected(i + panel.scroll) {
                styled.on(Color::Blue)
            } else {
                styled
            };
            let _ = queue!(
                &self.stdout,
                Print(final_name),
                terminal::Clear(ClearType::UntilNewLine),
                cursor::MoveToNextLine(1)
            );
        }
        for _ in 0..(rows as i16 - panel.entries.len() as i16).max(0) {
            let _ = queue!(
                &self.stdout,
                terminal::Clear(ClearType::UntilNewLine),
                cursor::MoveToNextLine(1)
            );
        }
        self.draw_scrollbar(panel, rows);
    }

    fn draw_files_two_panes(&mut self, left: &PanelState, right: &PanelState, active: ActivePane) {
        let rows = self.visible_rows();
        let pw = self.columns.saturating_sub(1) / 2;
        for row in 0..rows {
            let _ = queue!(
                &self.stdout,
                cursor::MoveTo(0, HEADER_ROWS + row),
                terminal::Clear(ClearType::UntilNewLine)
            );
            let li = left.scroll + row as usize;
            if li < left.entries.len() {
                let _ = queue!(
                    &self.stdout,
                    cursor::MoveTo(0, HEADER_ROWS + row),
                    Print(trunc_styled(styled_name(left, li), pw))
                );
            }
            let sep = if matches!(active, ActivePane::Left) {
                Color::Blue
            } else {
                Color::Grey
            };
            let _ = queue!(
                &self.stdout,
                cursor::MoveTo(pw, HEADER_ROWS + row),
                Print("│".with(sep))
            );
            let ri = right.scroll + row as usize;
            if ri < right.entries.len() {
                let _ = queue!(
                    &self.stdout,
                    cursor::MoveTo(pw + 1, HEADER_ROWS + row),
                    Print(trunc_styled(styled_name(right, ri), pw - 1))
                );
            }
        }
    }

    fn draw_scrollbar(&self, panel: &PanelState, rows: u16) {
        let total = panel.entries.len();
        if total > rows as usize {
            let h = ((rows as f32 / total as f32 * rows as f32).round().max(1.0)) as u16;
            let p = (panel.scroll as f32 / total as f32 * rows as f32)
                .round()
                .max(0.0) as u16;
            for i in p..(p + h).min(rows) {
                let _ = queue!(
                    &self.stdout,
                    cursor::MoveTo(self.columns - 1, HEADER_ROWS + i),
                    Print("|".dark_blue())
                );
            }
        }
    }

    fn draw_quick_view(&mut self, qv: &QuickViewMode) {
        let rows = self.visible_rows();
        match qv {
            QuickViewMode::Text { lines, start, .. } => self.draw_text(lines, *start, rows),
            QuickViewMode::Image(bytes) => {
                let enc = iterm2img::from_bytes(bytes.to_vec())
                    .width(self.columns as u64)
                    .height(rows as u64)
                    .width_auto()
                    .preserve_aspect_ratio(true)
                    .inline(true)
                    .build();
                let _ = queue!(
                    &self.stdout,
                    cursor::MoveTo(0, HEADER_ROWS),
                    Print(enc),
                    terminal::Clear(ClearType::UntilNewLine)
                );
            }
            QuickViewMode::Directory { lines } => self.draw_text(lines, 0, rows),
            QuickViewMode::NotSupported => self.draw_text(&["".to_string()], 0, rows),
        }
    }

    fn draw_text(&self, content: &[String], start: usize, rows: u16) {
        let _ = queue!(&self.stdout, cursor::MoveTo(0, HEADER_ROWS));
        for line in content[start..].iter().take(rows as usize) {
            let _ = queue!(
                &self.stdout,
                Print(line),
                terminal::Clear(ClearType::UntilNewLine),
                cursor::MoveToNextLine(1)
            );
        }
        for _ in 0..(rows as i16 - content.len() as i16).max(0) {
            let _ = queue!(
                &self.stdout,
                terminal::Clear(ClearType::UntilNewLine),
                cursor::MoveToNextLine(1)
            );
        }
    }

    fn draw_footer(&self, panel: &PanelState) {
        self.draw_line(self.rows - FOOTER_ROWS);
        let text = if panel.mode == PanelMode::Filter {
            format!("Filter: {}", panel.filter_string).with(Color::Green)
        } else if matches!(panel.mode, PanelMode::QuickView(_)) {
            format!("File {} / {}", panel.cursor + 1, panel.entries.len()).with(Color::Green)
        } else if panel.multi_selected_count() > 0 {
            format!("Selected: {}", panel.multi_selected_count()).with(Color::Yellow)
        } else {
            let has_parent = panel
                .entries
                .first()
                .map(|e| e.name == "..")
                .unwrap_or(false);
            let dirs = panel
                .entries
                .iter()
                .filter(|e| e.is_dir() && e.name != "..")
                .count();
            format!(
                "{} dirs, {} files",
                dirs,
                panel
                    .entries
                    .len()
                    .saturating_sub(dirs + if has_parent { 1 } else { 0 })
            )
            .with(Color::Green)
        };
        let _ = queue!(
            &self.stdout,
            cursor::MoveTo(0, self.rows - FOOTER_ROWS + 1),
            Print(text),
            terminal::Clear(ClearType::UntilNewLine)
        );
    }

    fn draw_footer_actions(&self, mode: &PanelMode) {
        let actions = footer_actions(mode);
        if actions.is_empty() {
            return;
        }
        let total: usize = actions.iter().map(|a| a.len()).sum();
        let space =
            (self.columns as usize).saturating_sub(total) / actions.len().saturating_sub(1).max(1);
        let rendered: String = actions
            .iter()
            .enumerate()
            .map(|(i, a)| {
                if i + 1 < actions.len() {
                    format!("{}{}", a, " ".repeat(space.max(1)))
                } else {
                    a.clone()
                }
            })
            .collect();
        let _ = queue!(
            &self.stdout,
            cursor::MoveTo(0, self.rows - FOOTER_ROWS + 2),
            Print(
                rendered
                    .chars()
                    .take(self.columns as usize)
                    .collect::<String>()
                    .with(Color::Green)
            ),
            terminal::Clear(ClearType::UntilNewLine)
        );
    }

    fn draw_help_overlay(&mut self) {
        let mut e = self.help_entries.clone();
        e.sort_by_key(|(k, _)| k.clone());
        let (mk, md) = (
            e.iter().map(|(k, _)| k.len()).max().unwrap_or(0),
            e.iter().map(|(_, d)| d.len()).max().unwrap_or(0),
        );
        let tw = (mk + 4 + md + 4).min(self.columns as usize);
        let sc = ((self.columns as usize).saturating_sub(tw)) / 2;
        let _ = queue!(
            &self.stdout,
            cursor::MoveTo(0, 2),
            terminal::Clear(ClearType::FromCursorDown)
        );

        // Draw SVG logo using iTerm2 image protocol, or fallback to ASCII
        let logo_height: u16 = if let Some(png_bytes) = &self.logo_png {
            let enc = iterm2img::from_bytes(png_bytes.clone())
                .height(6)
                .preserve_aspect_ratio(true)
                .inline(true)
                .build();
            let _ = queue!(&self.stdout, cursor::MoveTo(0, 2), Print(enc));
            7 // logo takes about 6-7 rows
        } else {
            // Fallback ASCII logo for non-iTerm2 terminals
            let logo = [
                r"   _____ _     _              ",
                r"  |  ___(_)___| |__   ___ ____",
                r"  | |_  | / __| '_ \ / _ \_  /",
                r"  |  _| | \__ \ | | |  __// / ",
                r"  |_|   |_|___/_| |_|\___/___|",
            ];
            let logo_width = logo.iter().map(|l| l.len()).max().unwrap_or(0);
            let logo_start = ((self.columns as usize).saturating_sub(logo_width)) / 2;
            for (i, line) in logo.iter().enumerate() {
                let _ = queue!(
                    &self.stdout,
                    cursor::MoveTo(logo_start as u16, 2 + i as u16),
                    Print(line.with(Color::Cyan))
                );
            }
            logo.len() as u16 + 1
        };

        let content_start = 2 + logo_height;
        let _ = queue!(
            &self.stdout,
            cursor::MoveTo(sc as u16, content_start),
            Print(
                "Keyboard shortcuts"
                    .with(Color::Cyan)
                    .attribute(crossterm::style::Attribute::Bold)
            )
        );
        let _ = queue!(
            &self.stdout,
            cursor::MoveTo(sc as u16, content_start + 1),
            Print("─".repeat(tw).with(Color::Blue))
        );
        for (i, (key, desc)) in e.iter().enumerate() {
            if content_start + 2 + i as u16 >= self.rows - 1 {
                break;
            }
            let _ = queue!(
                &self.stdout,
                cursor::MoveTo(sc as u16, content_start + 2 + i as u16),
                Print(format!("{:w$}", key, w = mk).with(Color::Yellow)),
                cursor::MoveTo((sc + mk + 4) as u16, content_start + 2 + i as u16),
                Print(desc.clone().with(Color::Green))
            );
        }
    }

    fn draw_line(&self, row: u16) {
        let _ = queue!(
            &self.stdout,
            cursor::MoveTo(0, row),
            Print(
                (0..self.columns)
                    .map(|_| "─")
                    .collect::<String>()
                    .with(Color::Blue)
            )
        );
    }
}

impl Drop for TerminalRenderer {
    fn drop(&mut self) {
        self.reset_terminal();
    }
}

fn default_help_entries() -> Vec<(String, String)> {
    [
        ("F1", "Toggle help"),
        ("Arrows", "Navigate"),
        ("Enter", "Open dir/file"),
        ("Backspace", "Go up"),
        ("F3", "Quick view"),
        ("Tab", "Switch pane"),
        ("Esc", "Cancel/close"),
        ("F10/Ctrl+C", "Quit"),
        ("Ctrl+W", "Delete"),
        ("F6", "Find (fd)"),
        ("F7", "RipGrep"),
        ("Ctrl+D", "Favorites"),
        ("Space", "Toggle selection"),
        ("F4", "VS Code"),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect()
}

fn footer_actions(mode: &PanelMode) -> Vec<String> {
    match mode {
        PanelMode::Normal => vec![
            "[f1]help",
            "[f3]view",
            "[f4]edit",
            "[f6]find",
            "[f7]rg",
            "[ctrl+w]del",
            "[f10]quit",
        ],
        PanelMode::Filter => vec!["[f1]help", "[esc]clear", "[f3]view", "[f10]quit"],
        PanelMode::QuickView(_) => vec![
            "[f1]help",
            "[↑↓]scroll",
            "[pgup/dn]page",
            "[←→]prev/next",
            "[f3]close",
        ],
    }
    .into_iter()
    .map(String::from)
    .collect()
}

fn style_entry(e: &FileEntry) -> StyledContent<String> {
    if e.is_dir() {
        e.name.clone().yellow()
    } else {
        e.name.clone().dark_yellow()
    }
}
fn styled_name(p: &PanelState, i: usize) -> StyledContent<String> {
    let n = style_entry(&p.entries[i]);
    let s = if i == p.cursor { n.negative() } else { n };
    if p.is_multi_selected(i) {
        s.on(Color::Blue)
    } else {
        s
    }
}
fn trunc_styled(item: StyledContent<String>, w: u16) -> StyledContent<String> {
    let t = item.content().clone();
    if t.len() as u16 > w && w > 1 {
        item.style().apply(
            t.chars()
                .take((w - 1) as usize)
                .chain(std::iter::once('…'))
                .collect(),
        )
    } else {
        item
    }
}
fn truncate(t: &str, w: usize) -> String {
    if t.len() > w && w > 1 {
        t.chars().take(w - 1).chain(std::iter::once('…')).collect()
    } else {
        t.to_string()
    }
}

/// Render SVG bytes to PNG bytes using resvg
fn render_svg_to_png(svg_data: &[u8]) -> Option<Vec<u8>> {
    use resvg::tiny_skia;
    use resvg::usvg;

    let opts = usvg::Options::default();
    let tree = usvg::Tree::from_data(svg_data, &opts).ok()?;

    let size = tree.size();
    let width = size.width() as u32;
    let height = size.height() as u32;

    let mut pixmap = tiny_skia::Pixmap::new(width, height)?;
    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());

    pixmap.encode_png().ok()
}

//! Terminal renderer - draws UI to terminal.

use crate::application::use_cases::quick_view::human_size;
use crate::application::{ActivePane, AppState, PanelMode, PanelState, QuickViewMode};
use crate::domain::FileEntry;
use crate::infrastructure::disk_free_and_total;
use base64::Engine;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::style::{Color, Print, StyledContent, Stylize};
use crossterm::terminal::{ClearType, enable_raw_mode};
use crossterm::{cursor, execute, queue, terminal};
use image::{GenericImageView, ImageFormat};
use std::collections::hash_map::DefaultHasher;
use std::fs::OpenOptions;
use std::hash::{Hash, Hasher};
use std::io::{Cursor, Read, Write};
#[cfg(unix)]
use std::os::fd::AsRawFd;
use std::time::{Duration, Instant};

const LOADING_FRAMES: [&str; 4] = ["|", "/", "-", "\\"];
const KITTY_IMAGE_ID: u32 = 1;
const KITTY_DELETE_ESCAPE: &str = "\x1b_Ga=d,d=I,i=1\x1b\\";

#[cfg(test)]
use std::cell::RefCell;
#[cfg(test)]
use std::rc::Rc;

#[cfg(test)]
pub(crate) struct TestWriter {
    buffer: Rc<RefCell<Vec<u8>>>,
}

#[cfg(test)]
impl TestWriter {
    fn new(buffer: Rc<RefCell<Vec<u8>>>) -> Self {
        Self { buffer }
    }
}

#[cfg(test)]
impl Write for TestWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.buffer.borrow_mut().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub const HEADER_ROWS: u16 = 2;
pub const FOOTER_ROWS: u16 = 3;

/// Embedded SVG logo bytes (loaded at compile time)
const LOGO_SVG: &[u8] = include_bytes!("../../../Fishez_logo.svg");

#[derive(Clone, Copy, PartialEq, Eq)]
enum ImageProtocol {
    ITerm2,
    Kitty,
    None,
}

pub(crate) enum StdoutKind {
    Real(std::io::Stdout),
    #[cfg(test)]
    Test(TestWriter),
}

impl Write for StdoutKind {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            StdoutKind::Real(stdout) => stdout.write(buf),
            #[cfg(test)]
            StdoutKind::Test(writer) => writer.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            StdoutKind::Real(stdout) => stdout.flush(),
            #[cfg(test)]
            StdoutKind::Test(writer) => writer.flush(),
        }
    }
}

pub struct TerminalRenderer {
    pub columns: u16,
    pub rows: u16,
    stdout: StdoutKind,
    help_entries: Vec<(&'static str, &'static str)>,
    logo_png: Option<Vec<u8>>,
    image_protocol: ImageProtocol,
    kitty_image_hash: Option<u64>,
    loading_frame: usize,
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
        let _ = execute!(std::io::stdout(), EnableMouseCapture);

        // Pre-render SVG logo to PNG bytes
        let logo_png = render_svg_to_png(LOGO_SVG);

        Self {
            columns,
            rows,
            stdout: StdoutKind::Real(std::io::stdout()),
            help_entries: default_help_entries(),
            logo_png,
            image_protocol: detect_image_protocol(),
            kitty_image_hash: None,
            loading_frame: 0,
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
            &mut self.stdout,
            cursor::MoveTo(0, 0),
            cursor::Show,
            cursor::EnableBlinking,
            terminal::Clear(ClearType::All)
        );
        let _ = terminal::disable_raw_mode();
        let _ = execute!(std::io::stdout(), DisableMouseCapture);
        let _ = self.stdout.flush();
    }

    pub fn draw(&mut self, state: &AppState) {
        let panel = state.active_panel();
        let _ = queue!(&mut self.stdout, cursor::DisableBlinking, cursor::Hide);
        self.sync_kitty_image_visibility(&panel.mode);
        self.draw_header(panel);
        self.draw_system_header(panel);
        match &panel.mode {
            PanelMode::Normal | PanelMode::Filter => self.draw_files_list(panel),
            PanelMode::QuickView(qv) => self.draw_quick_view(qv),
        }
        self.draw_footer(panel);
        self.draw_footer_actions(&panel.mode);
        if state.show_onboarding {
            self.draw_onboarding_banner();
        }
        if state.show_help {
            self.draw_help_overlay();
        }
        let _ = self.stdout.flush();
    }

    pub fn draw_two_panes(&mut self, state: &AppState) {
        let _ = queue!(&mut self.stdout, cursor::DisableBlinking, cursor::Hide);
        self.sync_kitty_image_visibility(&state.active_panel().mode);
        if matches!(state.active_panel().mode, PanelMode::QuickView(_)) {
            self.draw(state);
            return;
        }
        self.draw_dual_header(&state.left_panel, &state.right_panel, state.active_pane);
        self.draw_system_header(state.active_panel());
        self.draw_files_two_panes(&state.left_panel, &state.right_panel, state.active_pane);
        self.draw_footer(state.active_panel());
        self.draw_footer_actions(&state.active_panel().mode);
        if state.show_onboarding {
            self.draw_onboarding_banner();
        }
        if state.show_help {
            self.draw_help_overlay();
        }
        let _ = self.stdout.flush();
    }

    fn draw_header(&mut self, panel: &PanelState) {
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
            &mut self.stdout,
            cursor::MoveTo(0, 0),
            Print(left),
            terminal::Clear(ClearType::UntilNewLine)
        );
    }

    fn draw_dual_header(&mut self, left: &PanelState, right: &PanelState, active: ActivePane) {
        let _ = queue!(
            &mut self.stdout,
            cursor::MoveTo(0, 0),
            terminal::Clear(ClearType::UntilNewLine)
        );
        let pw = self.columns.saturating_sub(1) / 2;
        let lt = truncate(&format!("L  {}", left.current_path.display()), pw as usize);
        let rt = truncate(&format!("R  {}", right.current_path.display()), pw as usize);
        let (lstyle, rstyle) = match active {
            ActivePane::Left => (lt.bold().with(Color::White), rt.with(Color::DarkGrey)),
            ActivePane::Right => (lt.with(Color::DarkGrey), rt.bold().with(Color::White)),
        };
        let _ = queue!(
            &mut self.stdout,
            cursor::MoveTo(0, 0),
            Print(lstyle),
            cursor::MoveTo(pw + 1, 0),
            Print(rstyle)
        );
    }

    fn draw_system_header(&mut self, panel: &PanelState) {
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
            &mut self.stdout,
            cursor::MoveTo(self.columns.saturating_sub(right.content().len() as u16), 0),
            Print(right)
        );
        self.draw_line(1);
    }

    fn draw_files_list(&mut self, panel: &PanelState) {
        let rows = self.visible_rows();
        let _ = queue!(&mut self.stdout, cursor::MoveTo(0, HEADER_ROWS));
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
                &mut self.stdout,
                Print(final_name),
                terminal::Clear(ClearType::UntilNewLine),
                cursor::MoveToNextLine(1)
            );
        }
        for _ in 0..(rows as i16 - panel.entries.len() as i16).max(0) {
            let _ = queue!(
                &mut self.stdout,
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
                &mut self.stdout,
                cursor::MoveTo(0, HEADER_ROWS + row),
                terminal::Clear(ClearType::UntilNewLine)
            );
            let li = left.scroll + row as usize;
            if li < left.entries.len() {
                let _ = queue!(
                    &mut self.stdout,
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
                &mut self.stdout,
                cursor::MoveTo(pw, HEADER_ROWS + row),
                Print("│".with(sep))
            );
            let ri = right.scroll + row as usize;
            if ri < right.entries.len() {
                let _ = queue!(
                    &mut self.stdout,
                    cursor::MoveTo(pw + 1, HEADER_ROWS + row),
                    Print(trunc_styled(styled_name(right, ri), pw - 1))
                );
            }
        }
    }

    fn draw_scrollbar(&mut self, panel: &PanelState, rows: u16) {
        let total = panel.entries.len();
        if total > rows as usize {
            let h = ((rows as f32 / total as f32 * rows as f32).round().max(1.0)) as u16;
            let p = (panel.scroll as f32 / total as f32 * rows as f32)
                .round()
                .max(0.0) as u16;
            for i in p..(p + h).min(rows) {
                let _ = queue!(
                    &mut self.stdout,
                    cursor::MoveTo(self.columns - 1, HEADER_ROWS + i),
                    Print("|".dark_blue())
                );
            }
        }
    }

    fn sync_kitty_image_visibility(&mut self, mode: &PanelMode) {
        if self.image_protocol == ImageProtocol::Kitty
            && self.kitty_image_hash.is_some()
            && !matches!(mode, PanelMode::QuickView(QuickViewMode::Image(_)))
        {
            let _ = queue!(&mut self.stdout, Print(KITTY_DELETE_ESCAPE));
            self.kitty_image_hash = None;
        }
    }

    fn draw_quick_view(&mut self, qv: &QuickViewMode) {
        self.clear_quick_view_area();
        if let QuickViewMode::Loading { message } = qv {
            self.draw_loading_indicator(message);
            return;
        }
        let rows = self.visible_rows();
        match qv {
            QuickViewMode::Text { lines, start, .. } => self.draw_text(lines, *start, rows),
            QuickViewMode::Image(bytes) => match self.image_protocol {
                ImageProtocol::ITerm2 => {
                    let enc = iterm2img::from_bytes(bytes.to_vec())
                        .width(self.columns as u64)
                        .height(rows as u64)
                        .width_auto()
                        .preserve_aspect_ratio(true)
                        .inline(true)
                        .build();
                    let _ = queue!(&mut self.stdout, cursor::MoveTo(0, HEADER_ROWS), Print(enc));
                }
                ImageProtocol::Kitty => {
                    let image_hash = hash_bytes(bytes);
                    if self.kitty_image_hash == Some(image_hash) {
                        return;
                    }
                    if let Some(enc) = kitty_image_escape(bytes, self.columns, rows) {
                        let _ =
                            queue!(&mut self.stdout, cursor::MoveTo(0, HEADER_ROWS), Print(enc));
                        self.kitty_image_hash = Some(image_hash);
                    }
                }
                ImageProtocol::None => self.draw_text(
                    &["Image preview unsupported by this terminal.".to_string()],
                    0,
                    rows,
                ),
            },
            QuickViewMode::Directory { lines } => self.draw_text(lines, 0, rows),
            QuickViewMode::NotSupported => self.draw_text(&["".to_string()], 0, rows),
            QuickViewMode::Loading { .. } => {} // handled above via early return
        }
    }

    fn clear_quick_view_area(&mut self) {
        let _ = queue!(
            &mut self.stdout,
            cursor::MoveTo(0, HEADER_ROWS),
            terminal::Clear(ClearType::FromCursorDown)
        );
    }

    fn draw_loading_indicator(&mut self, message: &str) {
        let frame = LOADING_FRAMES[self.loading_frame % LOADING_FRAMES.len()];
        self.loading_frame = self.loading_frame.wrapping_add(1);
        let _ = queue!(
            &mut self.stdout,
            cursor::MoveTo(0, HEADER_ROWS),
            Print(format!("{} {}", frame, message)),
            terminal::Clear(ClearType::UntilNewLine)
        );
    }

    fn draw_text(&mut self, content: &[String], start: usize, rows: u16) {
        let _ = queue!(&mut self.stdout, cursor::MoveTo(0, HEADER_ROWS));
        for line in content[start..].iter().take(rows as usize) {
            let _ = queue!(
                &mut self.stdout,
                Print(line),
                terminal::Clear(ClearType::UntilNewLine),
                cursor::MoveToNextLine(1)
            );
        }
        for _ in 0..(rows as i16 - content.len() as i16).max(0) {
            let _ = queue!(
                &mut self.stdout,
                terminal::Clear(ClearType::UntilNewLine),
                cursor::MoveToNextLine(1)
            );
        }
    }

    fn draw_footer(&mut self, panel: &PanelState) {
        self.draw_line(self.rows - FOOTER_ROWS);
        let row = self.rows - FOOTER_ROWS + 1;

        let (left, color, size_text) = if panel.mode == PanelMode::Filter {
            (
                format!("Filter: {}", panel.filter_string),
                Color::Green,
                None,
            )
        } else if matches!(panel.mode, PanelMode::QuickView(_)) {
            (
                format!("File {} / {}", panel.cursor + 1, panel.entries.len()),
                Color::Green,
                None,
            )
        } else if panel.multi_selected_count() > 0 {
            let total: u64 = panel
                .multi_selected
                .iter()
                .filter_map(|&i| panel.entries.get(i))
                .filter(|e| !e.is_dir())
                .map(|e| e.size)
                .sum();
            (
                format!("Selected: {}", panel.multi_selected_count()),
                Color::Yellow,
                Some(human_size(total)),
            )
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
            let files = panel
                .entries
                .len()
                .saturating_sub(dirs + if has_parent { 1 } else { 0 });
            let disk_text = disk_free_and_total(&panel.current_path)
                .map(|(free, total)| format!("{} of {} free", human_size(free), human_size(total)));
            (
                format!("{} dirs, {} files", dirs, files),
                Color::Green,
                disk_text,
            )
        };

        let _ = queue!(
            &mut self.stdout,
            cursor::MoveTo(0, row),
            terminal::Clear(ClearType::UntilNewLine),
            Print(left.clone().with(color)),
        );

        if let Some(size_str) = size_text {
            let gap = 2usize;
            let needed = left.len() + gap + size_str.len();
            if needed <= self.columns as usize {
                let col = self.columns - size_str.len() as u16;
                let _ = queue!(
                    &mut self.stdout,
                    cursor::MoveTo(col, row),
                    Print(size_str.with(Color::DarkGrey)),
                );
            }
        }
    }

    fn draw_footer_actions(&mut self, mode: &PanelMode) {
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
                    a.to_string()
                }
            })
            .collect();
        let _ = queue!(
            &mut self.stdout,
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
        e.sort_by_key(|(k, _)| *k);
        let (mk, md) = (
            e.iter().map(|(k, _)| k.len()).max().unwrap_or(0),
            e.iter().map(|(_, d)| d.len()).max().unwrap_or(0),
        );
        let tw = (mk + 4 + md + 4).min(self.columns as usize);
        let sc = ((self.columns as usize).saturating_sub(tw)) / 2;
        let _ = queue!(
            &mut self.stdout,
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
            let _ = queue!(&mut self.stdout, cursor::MoveTo(0, 2), Print(enc));
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
                    &mut self.stdout,
                    cursor::MoveTo(logo_start as u16, 2 + i as u16),
                    Print(line.with(Color::Cyan))
                );
            }
            logo.len() as u16 + 1
        };

        let content_start = 2 + logo_height;
        let _ = queue!(
            &mut self.stdout,
            cursor::MoveTo(sc as u16, content_start),
            Print(
                "Keyboard shortcuts"
                    .with(Color::Cyan)
                    .attribute(crossterm::style::Attribute::Bold)
            )
        );
        let _ = queue!(
            &mut self.stdout,
            cursor::MoveTo(sc as u16, content_start + 1),
            Print("─".repeat(tw).with(Color::Blue))
        );
        for (i, (key, desc)) in e.iter().enumerate() {
            if content_start + 2 + i as u16 >= self.rows - 1 {
                break;
            }
            let _ = queue!(
                &mut self.stdout,
                cursor::MoveTo(sc as u16, content_start + 2 + i as u16),
                Print(format!("{:w$}", key, w = mk).with(Color::Yellow)),
                cursor::MoveTo((sc + mk + 4) as u16, content_start + 2 + i as u16),
                Print((*desc).with(Color::Green))
            );
        }
    }

    fn draw_onboarding_banner(&mut self) {
        // Inner width (excluding │ borders). Row layout:
        //  0  ╭──╮
        //  1  │  │  blank
        //  2  │  │  title
        //  3  │  │  subtitle
        //  4  │  │  blank
        //  5  ╞══╡
        //  6  │  │  blank
        //  7  │  │  shortcut
        //  8  │  │  shortcut
        //  9  │  │  shortcut
        // 10  │  │  shortcut
        // 11  │  │  blank
        // 12  ╞══╡
        // 13  │  │  dismiss
        // 14  │  │  blank
        // 15  ╰──╯
        const IW: usize = 56;
        const BW: u16 = IW as u16 + 2;
        const BH: u16 = 16;

        if self.columns < BW + 2 || self.rows < BH + 2 {
            return;
        }

        let col = (self.columns - BW) / 2;
        let row = (self.rows - BH) / 2;

        let border = Color::Cyan;
        let key_col = Color::Yellow;
        let desc_col = Color::Reset;
        let dim_col = Color::DarkGrey;
        let bg = Color::Black;

        let center = |s: &str| -> String {
            let pad = IW.saturating_sub(s.len());
            let l = pad / 2;
            format!("{}{}{}", " ".repeat(l), s, " ".repeat(pad - l))
        };
        let blank = |s: &mut Self, r: u16| {
            let _ = queue!(
                s.stdout,
                cursor::MoveTo(col, row + r),
                Print("│".with(border).on(bg)),
                Print(" ".repeat(IW).on(bg)),
                Print("│".with(border).on(bg))
            );
        };

        let top = format!("╭{}╮", "─".repeat(IW));
        let sep = format!("╞{}╡", "═".repeat(IW));
        let bot = format!("╰{}╯", "─".repeat(IW));

        macro_rules! mv {
            ($r:expr) => {
                cursor::MoveTo(col, row + $r)
            };
        }

        let _ = queue!(&mut self.stdout, mv!(0), Print(top.with(border).on(bg)));
        blank(self, 1);

        let title = center("f i s h e z");
        let _ = queue!(
            &mut self.stdout,
            mv!(2),
            Print("│".with(border).on(bg)),
            Print(title.bold().with(Color::White).on(bg)),
            Print("│".with(border).on(bg))
        );

        let sub = center("a fast terminal file manager");
        let _ = queue!(
            &mut self.stdout,
            mv!(3),
            Print("│".with(border).on(bg)),
            Print(sub.with(dim_col).on(bg)),
            Print("│".with(border).on(bg))
        );

        blank(self, 4);
        let _ = queue!(
            &mut self.stdout,
            mv!(5),
            Print(sep.clone().with(border).on(bg))
        );
        blank(self, 6);

        // shortcut rows: 3 + key(11) + desc(18) + key(8) + desc(14) + 2 = 56
        let shortcuts: &[(&str, &str, &str, &str)] = &[
            ("type a-z", "filter instantly", "F3", "quick view"),
            ("Enter", "open", "F4", "VS Code"),
            ("Ctrl+T", "two-pane mode", "F8", "delete"),
            ("!", "shell command", "F1", "all shortcuts"),
        ];

        for (i, (k1, d1, k2, d2)) in shortcuts.iter().enumerate() {
            let _ = queue!(
                &mut self.stdout,
                mv!(7 + i as u16),
                Print("│".with(border).on(bg)),
                Print("   ".on(bg)),
                Print(format!("{:<11}", k1).with(key_col).on(bg)),
                Print(format!("{:<18}", d1).with(desc_col).on(bg)),
                Print(format!("{:<8}", k2).with(key_col).on(bg)),
                Print(format!("{:<14}", d2).with(desc_col).on(bg)),
                Print("  ".on(bg)),
                Print("│".with(border).on(bg))
            );
        }

        blank(self, 11);
        let _ = queue!(
            &mut self.stdout,
            mv!(12),
            Print(sep.clone().with(border).on(bg))
        );

        let dismiss = center("press any key to start  ·  F10 quit");
        let _ = queue!(
            &mut self.stdout,
            mv!(13),
            Print("│".with(border).on(bg)),
            Print(dismiss.with(dim_col).on(bg)),
            Print("│".with(border).on(bg))
        );

        blank(self, 14);
        let _ = queue!(&mut self.stdout, mv!(15), Print(bot.with(border).on(bg)));

        let _ = self.stdout.flush();
    }

    fn draw_line(&mut self, row: u16) {
        let _ = queue!(
            &mut self.stdout,
            cursor::MoveTo(0, row),
            Print("─".repeat(self.columns as usize).with(Color::Blue))
        );
    }

    pub(crate) fn writer(&mut self) -> &mut StdoutKind {
        &mut self.stdout
    }
}

#[cfg(test)]
impl TerminalRenderer {
    fn with_test_writer(columns: u16, rows: u16) -> (Self, Rc<RefCell<Vec<u8>>>) {
        let buffer = Rc::new(RefCell::new(Vec::new()));
        let writer = TestWriter::new(buffer.clone());

        (
            TerminalRenderer {
                columns,
                rows,
                stdout: StdoutKind::Test(writer),
                help_entries: default_help_entries(),
                logo_png: None,
                image_protocol: ImageProtocol::ITerm2,
                kitty_image_hash: None,
                loading_frame: 0,
            },
            buffer,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::{AppState, PanelMode, QuickViewMode};

    #[test]
    fn quick_view_image_clears_previous_content() {
        let (mut renderer, buffer) = TerminalRenderer::with_test_writer(40, 20);
        renderer.draw_quick_view(&QuickViewMode::Image(vec![0xFF]));
        let res = buffer.borrow();
        const CLEAR_SEQ: &[u8] = b"\x1b[J";
        assert!(
            res.windows(CLEAR_SEQ.len())
                .any(|window| window == CLEAR_SEQ),
            "expected clear command in quick view image output"
        );
    }

    #[test]
    fn quick_view_image_does_not_clear_the_image_line_after_printing() {
        let (mut renderer, buffer) = TerminalRenderer::with_test_writer(40, 20);
        renderer.draw_quick_view(&QuickViewMode::Image(vec![0xFF]));
        let res = buffer.borrow();
        let tail = b"\x07\x1b[K";

        assert!(
            !res.windows(tail.len()).any(|window| window == tail),
            "image output should not clear the line after the OSC payload"
        );
    }

    #[test]
    fn leaving_kitty_image_quick_view_emits_delete_sequence() {
        let (mut renderer, buffer) = TerminalRenderer::with_test_writer(40, 20);
        renderer.image_protocol = ImageProtocol::Kitty;
        let mut state = AppState::new(false);
        state.left_panel.mode = PanelMode::QuickView(QuickViewMode::Image(tiny_png()));

        renderer.draw(&state);
        state.left_panel.mode = PanelMode::Normal;
        renderer.draw(&state);

        let res = buffer.borrow();
        assert!(
            res.windows(b"\x1b_Ga=d,d=I,i=1\x1b\\".len())
                .any(|window| window == b"\x1b_Ga=d,d=I,i=1\x1b\\"),
            "expected kitty delete sequence when leaving image quick view"
        );
    }

    #[test]
    fn redrawing_same_kitty_image_does_not_resend_payload() {
        let (mut renderer, buffer) = TerminalRenderer::with_test_writer(40, 20);
        renderer.image_protocol = ImageProtocol::Kitty;
        let image = tiny_png();

        renderer.draw_quick_view(&QuickViewMode::Image(image.clone()));
        renderer.draw_quick_view(&QuickViewMode::Image(image));

        let res = buffer.borrow();
        assert_eq!(
            res.windows(b"\x1b_Ga=T,f=100,i=1,".len())
                .filter(|window| *window == b"\x1b_Ga=T,f=100,i=1,")
                .count(),
            1,
            "expected kitty image payload to be sent only once for the same image"
        );
    }

    #[test]
    fn quick_view_text_clears_previous_content() {
        let (mut renderer, buffer) = TerminalRenderer::with_test_writer(40, 20);
        renderer.draw_quick_view(&QuickViewMode::Text {
            lines: vec!["first line".to_string()],
            start: 0,
        });
        let res = buffer.borrow();
        const CLEAR_SEQ: &[u8] = b"\x1b[J";
        assert!(
            res.windows(CLEAR_SEQ.len())
                .any(|window| window == CLEAR_SEQ),
            "expected clear command in quick view text output"
        );
    }

    #[test]
    fn kitty_escape_uses_kitty_graphics_prefix() {
        let png = tiny_png();
        let esc = kitty_image_escape(&png, 40, 18).expect("expected kitty escape");
        assert!(esc.starts_with("\x1b_Ga=T,f=100,i=1,"));
        assert!(esc.ends_with("\x1b\\"));
    }

    #[test]
    fn kitty_escape_uses_width_only_for_wide_images() {
        let png = test_png(400, 100);
        let esc = kitty_image_escape(&png, 40, 18).expect("expected kitty escape");
        assert!(esc.contains(",c=40"));
        assert!(!esc.contains(",r=18"));
    }

    #[test]
    fn kitty_escape_uses_height_only_for_tall_images() {
        let png = test_png(100, 200);
        let esc = kitty_image_escape(&png, 40, 18).expect("expected kitty escape");
        assert!(esc.contains(",r=18"));
        assert!(!esc.contains(",c=40"));
    }

    #[test]
    fn detects_kitty_probe_response() {
        assert!(is_kitty_probe_response(b"\x1b_Gi=31;OK\x1b\\"));
        assert!(!is_kitty_probe_response(b"\x1b[c"));
    }

    #[test]
    fn detects_iterm_probe_response() {
        assert!(is_iterm_probe_response(
            b"\x1b]1337;ReportCellSize=17.50;8.00;2.0\x07"
        ));
        assert!(!is_iterm_probe_response(b"\x1b]1337;CursorShape=1\x07"));
    }

    fn entry(name: &str, kind: crate::domain::EntryKind, size: u64) -> FileEntry {
        FileEntry::new(std::path::PathBuf::from(name), name.to_string(), kind, size)
    }

    fn file(name: &str, size: u64) -> FileEntry {
        entry(name, crate::domain::EntryKind::File, size)
    }

    fn dir(name: &str) -> FileEntry {
        entry(name, crate::domain::EntryKind::Dir, 0)
    }

    #[test]
    fn footer_shows_disk_free_and_total_while_browsing() {
        let (mut renderer, buffer) = TerminalRenderer::with_test_writer(80, 20);
        let mut panel = PanelState::new();
        panel.current_path = std::path::PathBuf::from("/");
        panel.entries = vec![file("a.txt", 1024), dir("subdir/")];
        renderer.draw_footer(&panel);
        let out = String::from_utf8_lossy(&buffer.borrow()).to_string();
        assert!(
            out.contains(" of ") && out.contains("free"),
            "expected a disk free/total figure in the footer, got: {out}"
        );
        assert!(
            out.contains("1 dirs, 1 files"),
            "left-hand dirs/files count must be unaffected, got: {out}"
        );
    }

    #[test]
    fn footer_omits_disk_usage_when_it_cannot_be_determined() {
        use std::os::unix::ffi::OsStrExt;
        let (mut renderer, buffer) = TerminalRenderer::with_test_writer(80, 20);
        let mut panel = PanelState::new();
        // A path with an interior NUL byte can't be turned into a CString, so
        // disk_free_and_total returns None; the footer must just omit the figure.
        panel.current_path = std::path::PathBuf::from(std::ffi::OsStr::from_bytes(b"/tmp\0bad"));
        renderer.draw_footer(&panel);
        let out = String::from_utf8_lossy(&buffer.borrow()).to_string();
        assert!(
            !out.contains("free"),
            "expected no disk usage figure when it can't be determined, got: {out}"
        );
    }

    #[test]
    fn footer_shows_selected_size_instead_of_total_when_multi_selecting() {
        let (mut renderer, buffer) = TerminalRenderer::with_test_writer(80, 20);
        let mut panel = PanelState::new();
        panel.entries = vec![
            file("a.txt", 1024),
            file("b.txt", 2048),
            file("c.txt", 4096),
        ];
        panel.toggle_multi_selection(0);
        panel.toggle_multi_selection(1);
        renderer.draw_footer(&panel);
        let out = String::from_utf8_lossy(&buffer.borrow()).to_string();
        assert!(
            out.contains("3.00 KB"),
            "expected selected-only total (1KB+2KB), got: {out}"
        );
        assert!(
            !out.contains("7.00 KB"),
            "whole-directory total must not be used while selecting"
        );
    }

    #[test]
    fn footer_selected_size_excludes_selected_directories() {
        let (mut renderer, buffer) = TerminalRenderer::with_test_writer(80, 20);
        let mut panel = PanelState::new();
        panel.entries = vec![file("a.txt", 1024), dir("subdir/")];
        panel.toggle_multi_selection(0);
        panel.toggle_multi_selection(1);
        renderer.draw_footer(&panel);
        let out = String::from_utf8_lossy(&buffer.borrow()).to_string();
        assert!(
            out.contains("1.00 KB"),
            "expected selected directory excluded from total, got: {out}"
        );
    }

    #[test]
    fn footer_omits_size_when_terminal_too_narrow() {
        let (mut renderer, buffer) = TerminalRenderer::with_test_writer(10, 20);
        let mut panel = PanelState::new();
        panel.current_path = std::path::PathBuf::from("/");
        panel.entries = vec![file("a.txt", 1024)];
        renderer.draw_footer(&panel);
        let out = String::from_utf8_lossy(&buffer.borrow()).to_string();
        assert!(
            out.contains("0 dirs, 1 files"),
            "expected left-hand text intact, got: {out}"
        );
        assert!(
            !out.contains("free"),
            "size must be omitted rather than corrupting a too-narrow line, got: {out}"
        );
    }

    fn tiny_png() -> Vec<u8> {
        test_png(1, 1)
    }

    fn test_png(width: u32, height: u32) -> Vec<u8> {
        let image = image::RgbImage::from_pixel(width, height, image::Rgb([255, 0, 0]));
        let mut bytes = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        bytes
    }
}

impl Drop for TerminalRenderer {
    fn drop(&mut self) {
        self.reset_terminal();
    }
}

fn default_help_entries() -> Vec<(&'static str, &'static str)> {
    vec![
        ("F1", "Toggle help"),
        ("Arrows", "Navigate"),
        ("Enter", "Open dir/file"),
        ("Backspace", "Go up"),
        ("F3", "Quick view"),
        ("Tab", "Switch pane"),
        ("Ctrl+T", "Toggle two-pane"),
        ("Esc", "Cancel/close"),
        ("F10/Ctrl+C", "Quit"),
        ("F5", "Copy to"),
        ("F6", "Move to"),
        ("Shift+F6", "Rename"),
        ("F7", "New folder"),
        ("F8", "Delete"),
        ("Ctrl+W", "Delete (alias)"),
        ("Ctrl+F", "Find files (fd)"),
        ("Ctrl+R", "Search contents (rg)"),
        ("!", "Shell command"),
        ("Ctrl+D", "Favorites"),
        ("Space", "Toggle selection"),
        ("F4", "VS Code"),
        ("a-z / 0-9", "Type to filter files"),
    ]
}

fn footer_actions(mode: &PanelMode) -> &'static [&'static str] {
    match mode {
        PanelMode::Normal => &[
            "[f1]help",
            "[f3]view",
            "[f4]edit",
            "[f5]copy",
            "[f6]move",
            "[f7]mkdir",
            "[f8]del",
            "[f10]quit",
        ],
        PanelMode::Filter => &["[f1]help", "[esc]clear", "[f3]view", "[f10]quit"],
        PanelMode::QuickView(_) => &[
            "[f1]help",
            "[↑↓]scroll",
            "[pgup/dn]page",
            "[←→]prev/next",
            "[f3]close",
        ],
    }
}

fn kitty_image_escape(bytes: &[u8], columns: u16, rows: u16) -> Option<String> {
    let image = image::load_from_memory(bytes).ok()?;
    let (width, height) = image.dimensions();
    let mut png = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .ok()?;
    let payload = base64::engine::general_purpose::STANDARD.encode(png);
    let control = kitty_size_control(width, height, columns, rows);
    Some(format!(
        "\x1b_Ga=T,f=100,i={KITTY_IMAGE_ID},{control},m=0;{payload}\x1b\\"
    ))
}

fn kitty_size_control(width: u32, height: u32, columns: u16, rows: u16) -> String {
    let box_aspect = columns as f32 / rows.max(1) as f32;
    let image_aspect = width as f32 / height.max(1) as f32;
    if image_aspect >= box_aspect {
        format!("c={columns}")
    } else {
        format!("r={rows}")
    }
}

fn hash_bytes(bytes: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

fn kitty_probe_query() -> &'static [u8] {
    b"\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\\x1b[c"
}

fn is_kitty_probe_response(bytes: &[u8]) -> bool {
    bytes
        .windows(b"\x1b_Gi=31;".len())
        .any(|window| window == b"\x1b_Gi=31;")
}

fn iterm_probe_query() -> &'static [u8] {
    b"\x1b]1337;ReportCellSize\x07"
}

fn is_iterm_probe_response(bytes: &[u8]) -> bool {
    bytes
        .windows(b"\x1b]1337;ReportCellSize=".len())
        .any(|window| window == b"\x1b]1337;ReportCellSize=")
}

fn detect_image_protocol() -> ImageProtocol {
    #[cfg(unix)]
    {
        detect_image_protocol_unix()
    }
    #[cfg(not(unix))]
    {
        ImageProtocol::None
    }
}

#[cfg(unix)]
fn detect_image_protocol_unix() -> ImageProtocol {
    let Ok(mut tty) = OpenOptions::new().read(true).write(true).open("/dev/tty") else {
        return ImageProtocol::None;
    };

    drain_probe_replies(&mut tty);
    if run_probe(&mut tty, kitty_probe_query(), is_kitty_probe_response) {
        return ImageProtocol::Kitty;
    }

    drain_probe_replies(&mut tty);
    if run_probe(&mut tty, iterm_probe_query(), is_iterm_probe_response) {
        return ImageProtocol::ITerm2;
    }

    ImageProtocol::None
}

#[cfg(unix)]
fn run_probe(tty: &mut std::fs::File, query: &[u8], matches_response: fn(&[u8]) -> bool) -> bool {
    if tty.write_all(query).is_err() || tty.flush().is_err() {
        return false;
    }

    let deadline = Instant::now() + Duration::from_millis(120);
    let mut buf = [0u8; 1024];
    let mut reply = Vec::new();

    while wait_for_tty_input(tty, deadline) {
        let Ok(read) = tty.read(&mut buf) else {
            break;
        };
        if read == 0 {
            break;
        }
        reply.extend_from_slice(&buf[..read]);
        if matches_response(&reply) {
            return true;
        }
    }

    false
}

#[cfg(unix)]
fn drain_probe_replies(tty: &mut std::fs::File) {
    let deadline = Instant::now() + Duration::from_millis(10);
    let mut buf = [0u8; 512];
    while wait_for_tty_input(tty, deadline) {
        if tty.read(&mut buf).ok().filter(|read| *read > 0).is_none() {
            break;
        }
    }
}

#[cfg(unix)]
fn wait_for_tty_input(tty: &std::fs::File, deadline: Instant) -> bool {
    if Instant::now() >= deadline {
        return false;
    }

    let remaining = deadline.saturating_duration_since(Instant::now());
    let secs = remaining.as_secs().min(i64::from(i32::MAX) as u64) as libc::time_t;
    let micros = remaining.subsec_micros() as libc::suseconds_t;
    let fd = tty.as_raw_fd();

    let mut readfds = unsafe { std::mem::zeroed::<libc::fd_set>() };
    let mut timeout = libc::timeval {
        tv_sec: secs,
        tv_usec: micros,
    };

    unsafe {
        libc::FD_ZERO(&mut readfds);
        libc::FD_SET(fd, &mut readfds);
        libc::select(
            fd + 1,
            &mut readfds,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut timeout,
        ) > 0
    }
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

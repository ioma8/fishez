//! Terminal renderer - draws UI to terminal.

use crate::application::{ActivePane, AppState, PanelMode, PanelState, QuickViewMode};
use crate::domain::FileEntry;
use base64::Engine;
use crossterm::style::{Color, Print, StyledContent, Stylize};
use crossterm::terminal::{ClearType, enable_raw_mode};
use crossterm::{cursor, queue, terminal};
use image::ImageFormat;
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
            &mut self.stdout,
            cursor::MoveTo(0, 0),
            Print(lt.with(Color::Cyan)),
            cursor::MoveTo(pw + 1, 0),
            Print(rt.with(Color::Cyan))
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
                    let _ = queue!(
                        &mut self.stdout,
                        cursor::MoveTo(0, HEADER_ROWS),
                        Print(enc)
                    );
                }
                ImageProtocol::Kitty => {
                    let image_hash = hash_bytes(bytes);
                    if self.kitty_image_hash == Some(image_hash) {
                        return;
                    }
                    if let Some(enc) = kitty_image_escape(bytes) {
                        let _ = queue!(
                            &mut self.stdout,
                            cursor::MoveTo(0, HEADER_ROWS),
                            Print(enc)
                        );
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
            &mut self.stdout,
            cursor::MoveTo(0, self.rows - FOOTER_ROWS + 1),
            Print(text),
            terminal::Clear(ClearType::UntilNewLine)
        );
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
            res.windows(b"\x1b_Ga=T,f=100,i=1,m=0;".len())
                .filter(|window| *window == b"\x1b_Ga=T,f=100,i=1,m=0;")
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
        let esc = kitty_image_escape(&png).expect("expected kitty escape");
        assert!(esc.starts_with("\x1b_Ga=T,f=100,i=1,m=0;"));
        assert!(esc.ends_with("\x1b\\"));
    }

    #[test]
    fn detects_kitty_probe_response() {
        assert!(is_kitty_probe_response(b"\x1b_Gi=31;OK\x1b\\"));
        assert!(!is_kitty_probe_response(b"\x1b[c"));
    }

    #[test]
    fn detects_iterm_probe_response() {
        assert!(is_iterm_probe_response(b"\x1b]1337;ReportCellSize=17.50;8.00;2.0\x07"));
        assert!(!is_iterm_probe_response(b"\x1b]1337;CursorShape=1\x07"));
    }

    fn tiny_png() -> Vec<u8> {
        let image = image::RgbImage::from_pixel(1, 1, image::Rgb([255, 0, 0]));
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
        ("Esc", "Cancel/close"),
        ("F10/Ctrl+C", "Quit"),
        ("Ctrl+W", "Delete"),
        ("F6", "Find (fd)"),
        ("F7", "RipGrep"),
        ("Ctrl+D", "Favorites"),
        ("Space", "Toggle selection"),
        ("F4", "VS Code"),
    ]
}

fn footer_actions(mode: &PanelMode) -> &'static [&'static str] {
    match mode {
        PanelMode::Normal => &[
            "[f1]help",
            "[f3]view",
            "[f4]edit",
            "[f6]find",
            "[f7]rg",
            "[ctrl+w]del",
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

fn kitty_image_escape(bytes: &[u8]) -> Option<String> {
    let png = png_bytes(bytes)?;
    let payload = base64::engine::general_purpose::STANDARD.encode(png);
    Some(format!(
        "\x1b_Ga=T,f=100,i={KITTY_IMAGE_ID},m=0;{payload}\x1b\\"
    ))
}

fn png_bytes(bytes: &[u8]) -> Option<Vec<u8>> {
    let image = image::load_from_memory(bytes).ok()?;
    let mut png = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .ok()?;
    Some(png)
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
    bytes.windows(b"\x1b_Gi=31;".len())
        .any(|window| window == b"\x1b_Gi=31;")
}

fn iterm_probe_query() -> &'static [u8] {
    b"\x1b]1337;ReportCellSize\x07"
}

fn is_iterm_probe_response(bytes: &[u8]) -> bool {
    bytes.windows(b"\x1b]1337;ReportCellSize=".len())
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
    let Ok(mut tty) = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
    else {
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
fn run_probe(
    tty: &mut std::fs::File,
    query: &[u8],
    matches_response: fn(&[u8]) -> bool,
) -> bool {
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

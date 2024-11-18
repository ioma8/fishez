use crossterm::{cursor, event::{self, KeyCode, KeyEvent}, queue, style::{Print, Stylize}, terminal::{self, ClearType}};

use crate::{files_view::{FilesView, FOOTER_ROWS, HEADER_ROWS}, terminal_ui::{FeatureTrait, TerminalUI}};

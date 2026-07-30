use std::cell::RefCell;
use std::collections::VecDeque;
use wxdragon::prelude::*;

const STYLE_DEFAULT: i32 = 0;
const STYLE_ERROR: i32 = 1;
const STYLE_WARN: i32 = 2;
const STYLE_INFO: i32 = 3;
const STYLE_DEBUG: i32 = 4;
const STYLE_TRACE: i32 = 5;

#[derive(Clone)]
struct LogEntry {
    level: log::Level,
    text: String,
}

impl LogEntry {
    fn style(&self) -> i32 {
        match self.level {
            log::Level::Error => STYLE_ERROR,
            log::Level::Warn => STYLE_WARN,
            log::Level::Info => STYLE_INFO,
            log::Level::Debug => STYLE_DEBUG,
            log::Level::Trace => STYLE_TRACE,
        }
    }
}

#[derive(Clone, Copy)]
pub enum LogTextCtrl {
    Styled(StyledTextCtrl),
    Plain(TextCtrl),
}

impl LogTextCtrl {
    pub fn append_text(&self, text: &str) {
        match self {
            LogTextCtrl::Styled(ctrl) => ctrl.append_text(text),
            LogTextCtrl::Plain(ctrl) => ctrl.append_text(text),
        }
    }

    pub fn clear_all(&self) {
        match self {
            LogTextCtrl::Styled(ctrl) => ctrl.clear_all(),
            LogTextCtrl::Plain(ctrl) => ctrl.clear(),
        }
    }

    pub fn get_end_position(&self) -> i64 {
        match self {
            LogTextCtrl::Styled(ctrl) => ctrl.get_length() as i64,
            LogTextCtrl::Plain(ctrl) => ctrl.get_last_position(),
        }
    }

    fn get_cursor_position(&self) -> i64 {
        match self {
            LogTextCtrl::Styled(ctrl) => ctrl.get_current_pos() as i64,
            LogTextCtrl::Plain(ctrl) => ctrl.get_insertion_point(),
        }
    }

    fn set_cursor_position(&self, position: i64) {
        match self {
            LogTextCtrl::Styled(ctrl) => ctrl.set_current_pos(position as i32),
            LogTextCtrl::Plain(ctrl) => ctrl.set_insertion_point(position),
        }
    }

    #[allow(dead_code)]
    pub fn set_selection_mode_typed(&self, mode: SelectionMode) {
        if let LogTextCtrl::Styled(ctrl) = self {
            ctrl.set_selection_mode_typed(mode);
        }
    }

    pub fn start_styling(&self, start: i32) {
        if let LogTextCtrl::Styled(ctrl) = self {
            ctrl.start_styling(start);
        }
    }

    pub fn set_styling(&self, length: i32, style: i32) {
        if let LogTextCtrl::Styled(ctrl) = self {
            ctrl.set_styling(length, style);
        }
    }

    pub fn goto_end(&self) {
        match self {
            LogTextCtrl::Styled(ctrl) => {
                let end = ctrl.get_length();
                ctrl.goto_pos(end);
                ctrl.ensure_caret_visible();
            }
            LogTextCtrl::Plain(ctrl) => {
                ctrl.set_insertion_point_end();
            }
        }
    }

    pub fn scroll_to_end(&self) {
        match self {
            LogTextCtrl::Styled(ctrl) => ctrl.scroll_to_end(),
            LogTextCtrl::Plain(ctrl) => ctrl.scroll_to_end(),
        }
    }
}

pub struct LogViewPanel {
    pub panel: Panel,
    pub text_ctrl: LogTextCtrl,
}

impl LogTextCtrl {
    pub fn add_to_sizer(&self, sizer: &BoxSizer, proportion: i32, flag: SizerFlag, border: i32) {
        match self {
            LogTextCtrl::Styled(ctrl) => sizer.add(ctrl, proportion, flag, border),
            LogTextCtrl::Plain(ctrl) => sizer.add(ctrl, proportion, flag, border),
        };
    }
}

impl LogViewPanel {
    fn init_log_styles(ctrl: &StyledTextCtrl) {
        ctrl.style_clear_all();

        ctrl.style_set_foreground(STYLE_DEFAULT, colours::BLACK);
        ctrl.style_set_background(STYLE_DEFAULT, colours::WHITE);
        ctrl.style_set_size(STYLE_DEFAULT, 10);

        ctrl.style_set_foreground(STYLE_ERROR, Colour::rgb(255, 0, 0));
        ctrl.style_set_background(STYLE_ERROR, colours::WHITE);
        ctrl.style_set_size(STYLE_ERROR, 10);

        ctrl.style_set_foreground(STYLE_WARN, Colour::rgb(150, 150, 20));
        ctrl.style_set_background(STYLE_WARN, colours::WHITE);
        ctrl.style_set_size(STYLE_WARN, 10);

        ctrl.style_set_foreground(STYLE_INFO, Colour::rgb(50, 150, 100));
        ctrl.style_set_background(STYLE_INFO, colours::WHITE);
        ctrl.style_set_size(STYLE_INFO, 10);

        ctrl.style_set_foreground(STYLE_DEBUG, Colour::rgb(10, 80, 150));
        ctrl.style_set_background(STYLE_DEBUG, colours::WHITE);
        ctrl.style_set_size(STYLE_DEBUG, 10);

        ctrl.style_set_foreground(STYLE_TRACE, colours::GRAY);
        ctrl.style_set_background(STYLE_TRACE, colours::WHITE);
        ctrl.style_set_size(STYLE_TRACE, 10);
    }

    pub fn new(parent: &Panel, use_color: bool) -> Self {
        let panel = Panel::builder(parent).build();
        let sizer = BoxSizer::builder(Orientation::Vertical).build();
        let text_ctrl = if use_color {
            let ctrl = StyledTextCtrl::builder(&panel).with_size(Size::new(-1, 200)).build();
            ctrl.set_undo_collection(false);
            ctrl.set_min_size(Size::new(-1, 200));
            ctrl.set_selection_mode_typed(SelectionMode::Stream);
            Self::init_log_styles(&ctrl);
            LogTextCtrl::Styled(ctrl)
        } else {
            let ctrl = TextCtrl::builder(&panel)
                .with_size(Size::new(-1, 200))
                .with_style(TextCtrlStyle::MultiLine | TextCtrlStyle::ReadOnly)
                .build();
            ctrl.set_min_size(Size::new(-1, 200));
            LogTextCtrl::Plain(ctrl)
        };

        text_ctrl.add_to_sizer(&sizer, 1, SizerFlag::Expand | SizerFlag::All, crate::settings::WIDGET_MARGIN);
        panel.set_sizer(sizer, true);
        Self { panel, text_ctrl }
    }
}

// UI-thread local storage for the log control. This avoids Send/Sync issues
// by ensuring the control is only ever accessed on the UI thread.
thread_local! {
    pub static LOG_TEXT_CTRL: RefCell<Option<LogTextCtrl>> = const { RefCell::new(None) };
    // A UI-thread log ring buffer; we render from here instead of reading back from the control
    pub static LOG_RING: RefCell<VecDeque<LogEntry>> = const { RefCell::new(VecDeque::new()) };
}

fn append_text_entry(ctrl: &LogTextCtrl, entry: &LogEntry) {
    let start = ctrl.get_end_position();
    ctrl.append_text(&entry.text);
    let end = ctrl.get_end_position();
    let length = end.saturating_sub(start) as i32;
    if length > 0 {
        ctrl.start_styling(start as i32);
        ctrl.set_styling(length, entry.style());
    }
}

fn rebuild_log_text(ctrl: &LogTextCtrl, ring: &VecDeque<LogEntry>) {
    ctrl.clear_all();
    for entry in ring.iter() {
        append_text_entry(ctrl, entry);
    }
}

/// Append structured log lines to the UI-side ring buffer and render them into the
/// StyledTextCtrl.
pub fn ui_append_logs(lines: Vec<(log::Level, String)>, max_lines: usize, auto_scroll: bool) {
    let new_entries: Vec<LogEntry> = lines
        .into_iter()
        .map(|(level, line_text)| LogEntry { level, text: line_text })
        .collect();

    LOG_RING.with(|ring_cell| {
        let mut ring = ring_cell.borrow_mut();
        let mut needs_rebuild = false;

        for entry in new_entries.iter().cloned() {
            ring.push_back(entry);
            if ring.len() > max_lines {
                ring.pop_front();
                needs_rebuild = true;
            }
        }

        LOG_TEXT_CTRL.with(|cell| {
            if let Some(ctrl) = cell.borrow().as_ref() {
                let cursor_position = (!auto_scroll).then(|| ctrl.get_cursor_position());

                if needs_rebuild {
                    rebuild_log_text(ctrl, &ring);
                } else {
                    for entry in new_entries.iter() {
                        append_text_entry(ctrl, entry);
                    }
                }

                if auto_scroll {
                    ctrl.goto_end();
                    ctrl.scroll_to_end();
                } else if let Some(cursor_position) = cursor_position {
                    ctrl.set_cursor_position(cursor_position.min(ctrl.get_end_position()));
                }
            }
        });
    });
}

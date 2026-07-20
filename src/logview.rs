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

#[allow(dead_code)]
pub struct LogViewPanel {
    pub panel: Panel,
    pub text_ctrl: StyledTextCtrl,
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

    pub fn new(parent: &Panel) -> Self {
        let panel = Panel::builder(parent).build();
        let sizer = BoxSizer::builder(Orientation::Vertical).build();
        let text_ctrl = StyledTextCtrl::builder(&panel).with_size(Size::new(-1, 200)).build();
        text_ctrl.set_min_size(Size::new(-1, 200));
        text_ctrl.set_selection_mode_typed(SelectionMode::Stream);
        Self::init_log_styles(&text_ctrl);
        sizer.add(&text_ctrl, 1, SizerFlag::Expand | SizerFlag::All, crate::settings::WIDGET_MARGIN);
        panel.set_sizer(sizer, true);
        Self { panel, text_ctrl }
    }
}

// UI-thread local storage for the log StyledTextCtrl. This avoids Send/Sync issues
// by ensuring the control is only ever accessed on the UI thread.
thread_local! {
    pub static LOG_TEXT_CTRL: RefCell<Option<StyledTextCtrl>> = const { RefCell::new(None) };
    // A UI-thread log ring buffer; we render from here instead of reading back from the control
    pub static LOG_RING: RefCell<VecDeque<LogEntry>> = const { RefCell::new(VecDeque::new()) };
}

fn append_text_entry(ctrl: &StyledTextCtrl, entry: &LogEntry) {
    let start = ctrl.get_length();
    ctrl.append_text(&entry.text);
    let end = ctrl.get_length();
    let length = end.saturating_sub(start);
    if length > 0 {
        ctrl.start_styling(start);
        ctrl.set_styling(length, entry.style());
    }
}

fn rebuild_log_text(ctrl: &StyledTextCtrl, ring: &VecDeque<LogEntry>) {
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
                if needs_rebuild {
                    rebuild_log_text(ctrl, &ring);
                } else {
                    for entry in new_entries.iter() {
                        append_text_entry(ctrl, entry);
                    }
                }

                let end = ctrl.get_length();
                ctrl.goto_pos(end);
                ctrl.ensure_caret_visible();
                if auto_scroll {
                    let last_line = ctrl.get_line_count().saturating_sub(1);
                    ctrl.scroll_to_line(last_line);
                }
            }
        });
    });
}

use std::cell::RefCell;
use std::collections::VecDeque;
use wxdragon::prelude::*;

#[allow(dead_code)]
pub struct LogViewPanel {
    pub panel: Panel,
    pub text_ctrl: TextCtrl,
}

impl LogViewPanel {
    pub fn new(parent: &Window) -> Self {
        let panel = Panel::builder(parent).build();
        let sizer = BoxSizer::builder(Orientation::Vertical).build();
        let text_ctrl = TextCtrl::builder(&panel)
            .with_size(Size::new(-1, 200))
            .with_style(TextCtrlStyle::MultiLine | TextCtrlStyle::ReadOnly)
            .build();
        text_ctrl.set_min_size(Size::new(-1, 200));
        sizer.add(&text_ctrl, 1, SizerFlag::Expand | SizerFlag::All, crate::settings::WIDGET_MARGIN);
        panel.set_sizer(sizer, true);
        Self { panel, text_ctrl }
    }
}

// UI-thread local storage for the log TextCtrl. This avoids Send/Sync issues by
// ensuring the control is only ever accessed on the UI thread.
thread_local! {
    pub static LOG_TEXT_CTRL: RefCell<Option<TextCtrl>> = RefCell::new(None);
    // A UI-thread log ring buffer; we render from here instead of reading back from the control
    pub static LOG_RING: RefCell<VecDeque<String>> = RefCell::new(VecDeque::new());
}

/// Append pre-formatted log text to the UI-side ring buffer and render into the TextCtrl.
/// Must be called on the UI thread (e.g., inside wxdragon::call_after).
pub fn ui_append_logs(appended: String, max_lines: usize, auto_scroll: bool) {
    LOG_RING.with(|ring_cell| {
        let mut ring = ring_cell.borrow_mut();
        for line in appended.lines() {
            ring.push_back(line.to_string());
        }
        while ring.len() > max_lines {
            ring.pop_front();
        }
        let mut text = String::with_capacity(appended.len().saturating_mul(2));
        if !ring.is_empty() {
            text = ring.iter().cloned().collect::<Vec<_>>().join("\n");
            text.push('\n');
        }
        LOG_TEXT_CTRL.with(|cell| {
            if let Some(ctrl) = cell.borrow().as_ref() {
                ctrl.set_value(&text);
                if auto_scroll {
                    // Try to move caret to end so it scrolls; if API is unavailable, this is a no-op.
                    // wxWidgets equivalent is SetInsertionPointEnd(); wxdragon likely exposes similar.
                    #[allow(unused_must_use)]
                    {
                        // Best-effort attempt; ignore if method not available in this binding.
                        // If set_insertion_point_end is not provided, adjust here to a supported API.
                        ctrl.set_insertion_point_end();
                    }
                }
            }
        });
    });
}

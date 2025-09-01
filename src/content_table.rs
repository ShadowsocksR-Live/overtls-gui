use crate::{LOG_HEIGHT, MENUBAR_HEIGHT, OverTlsNode, OverTlsNodeReceivers, node_details_dialog::show_node_details};
use fltk::{
    enums::{Align, Color, Event, FrameType, Shortcut},
    menu::MenuFlag,
    prelude::{GroupExt, MenuExt, TableExt, WidgetBase, WidgetExt},
    table::{Table, TableContext},
    window::Window,
};
use std::cell::RefCell;
use std::rc::Rc;

const HEADERS: [&str; 3] = ["Server Host", "Server Port", "Tunnel Path"];
const ROW_HEADER_WIDTH: i32 = 150;

pub fn create_table(
    selected_row: &Rc<RefCell<Option<usize>>>,
    nodes: &Rc<RefCell<Vec<OverTlsNode>>>,
    win: &Window,
    node_details_receivers: OverTlsNodeReceivers,
) -> Table {
    let mut table = Table::new(0, MENUBAR_HEIGHT, win.w(), win.h() - MENUBAR_HEIGHT - LOG_HEIGHT, "");
    table.set_cols(HEADERS.len() as i32);
    table.set_col_header(true);
    table.set_row_header(true);

    let configs_rc = nodes.clone();
    // To highlight the selected row
    let selected_row_handle = selected_row.clone();
    let win_clone = win.clone();
    let mut dnd = false;
    let mut released = false;
    table.handle(move |table, ev| {
        // Handle drag-and-drop events
        match ev {
            Event::DndEnter => {
                dnd = true;
                return true;
            }
            Event::DndDrag => return true,
            Event::DndRelease => {
                released = true;
                return true;
            }
            Event::Paste => {
                if dnd && released {
                    let event_text = fltk::app::event_text();
                    let nodes_clone = configs_rc.clone();
                    let mut table = table.clone();

                    // we use a timeout to avoid pasting the path into the buffer
                    fltk::app::add_timeout3(0.0, {
                        move |_| {
                            let mut successful = false;
                            // Process each line as a potential file path
                            for line in event_text.lines() {
                                log::debug!("Pasting file: {line}");
                                let path: std::path::PathBuf = line.trim().replace("file://", "").into();
                                match crate::paste_operations::process_inputed_file(&path) {
                                    Ok(node) => {
                                        nodes_clone.borrow_mut().push(node);
                                        successful = true;
                                    }
                                    Err(e) => {
                                        log::warn!("Failed to load file: {e}");
                                    }
                                }
                            }

                            // Update table if any files were loaded
                            if successful {
                                table.set_rows(nodes_clone.borrow().len() as i32);
                                table.redraw();
                            }
                        }
                    });

                    dnd = false;
                    released = false;
                    return true;
                } else {
                    return false;
                }
            }
            Event::DndLeave => {
                dnd = false;
                released = false;
                return true;
            }
            _ => {}
        }

        // Only respond to left mouse button
        if ev == Event::Released && fltk::app::event_button() == fltk::app::MouseButton::Left as i32 {
            let table_context = table.callback_context();
            let row = table.callback_row();
            #[cfg(debug_assertions)]
            match row % 5 {
                0 => log::error!("Table context: {table_context:?}, row: {row}"),
                1 => log::warn!("Table context: {table_context:?}, row: {row}"),
                2 => log::info!("Table context: {table_context:?}, row: {row}"),
                3 => log::debug!("Table context: {table_context:?}, row: {row}"),
                4 => log::trace!("Table context: {table_context:?}, row: {row}"),
                _ => unreachable!(),
            }

            if (table_context == TableContext::Cell || table_context == TableContext::RowHeader) && row >= 0 {
                let cols = table.cols();
                // First, clear all selections
                table.set_selection(-1, -1, -1, -1);
                // Then select the current row
                *selected_row_handle.borrow_mut() = Some(row as usize);
                table.set_selection(row, 0, row, cols - 1);
                table.redraw();
                return true;
            }
            if table_context == TableContext::ColHeader || table_context == TableContext::None || table_context == TableContext::Table {
                // Clear selection when clicking on column header or table empty area
                table.set_selection(-1, -1, -1, -1);
                *selected_row_handle.borrow_mut() = None;
                table.redraw();
                return true;
            }
        }
        // Right-click context menu
        if ev == Event::Released && fltk::app::event_button() == fltk::app::MouseButton::Right as i32 {
            let table_context = table.callback_context();
            let row = table.callback_row();
            let col = table.callback_col();
            let mut menu_btn = fltk::menu::MenuButton::new(fltk::app::event_x(), fltk::app::event_y(), 1, 1, "");

            let count = configs_rc.borrow().len();
            log::debug!("Right-click context menu, items count = {count}, table context = {table_context:?}, row = {row}, col = {col}");

            if (table_context == TableContext::Cell || table_context == TableContext::RowHeader) && 0 <= row && row < count as i32 {
                let configs_clone = configs_rc.clone();
                let win = win_clone.clone();
                let node_details_receivers = node_details_receivers.clone();
                menu_btn.add("View details", Shortcut::None, MenuFlag::Normal, move |_m| {
                    let cfg = configs_clone.borrow().get(row as usize).cloned();
                    if let Some(cfg) = cfg {
                        let (tx, rx) = std::sync::mpsc::channel();
                        show_node_details(&win, Some(cfg), tx);
                        node_details_receivers.lock().unwrap().push((Some(row as usize), rx));
                    }
                });

                // Export Node menu item
                let configs_clone = configs_rc.clone();
                menu_btn.add("Export Node", Shortcut::None, MenuFlag::Normal, move |_m| {
                    let Some(cfg) = configs_clone.borrow().get(row as usize).cloned() else {
                        return;
                    };
                    let Some(path) = crate::util::file_chooser_save_file("Export Node as JSON", None, "JSON File", &["json"]) else {
                        return;
                    };
                    match serde_json::to_string_pretty(&cfg) {
                        Ok(json_str) => {
                            if std::fs::write(&path, json_str).is_ok() {
                                log::debug!("Node exported to: {}", path.display());
                            } else {
                                rfd::MessageDialog::new()
                                    .set_title("Error")
                                    .set_description("Failed to write node file.")
                                    .set_level(rfd::MessageLevel::Error)
                                    .show();
                            }
                        }
                        Err(e) => {
                            rfd::MessageDialog::new()
                                .set_title("Error")
                                .set_description(format!("Failed to serialize node: {e}"))
                                .set_level(rfd::MessageLevel::Error)
                                .show();
                        }
                    }
                });

                // Show QR Code menu item
                let win = win_clone.clone();
                let configs_clone = configs_rc.clone();
                menu_btn.add("Show QR Code", Shortcut::None, MenuFlag::MenuDivider, move |_m| {
                    if let Some(cfg) = configs_clone.borrow().get(row as usize)
                        && let Ok(ssr_url) = cfg.generate_ssr_url()
                    {
                        let name = cfg.remarks.clone().unwrap_or_default();
                        let title = if name.is_empty() {
                            "Node QR Code".to_string()
                        } else {
                            format!("Node QR Code - '{name}'")
                        };
                        if let Err(e) = crate::qr_code_dialog::qr_code_dialog(&win, &title, &ssr_url) {
                            rfd::MessageDialog::new()
                                .set_title("Error")
                                .set_description(format!("Failed to show QR code: {e}"))
                                .set_level(rfd::MessageLevel::Error)
                                .show();
                        }
                    }
                });

                let configs_clone = configs_rc.clone();
                let mut table_clone = table.clone();
                let selected_row_clone = selected_row_handle.clone();
                menu_btn.add("Delete", Shortcut::None, MenuFlag::MenuDivider, move |_| {
                    let title = configs_clone
                        .borrow()
                        .get(row as usize)
                        .map(|c| c.remarks.clone().unwrap_or_default())
                        .unwrap_or_default();
                    let confirm = rfd::MessageDialog::new()
                        .set_title("Confirm Deletion")
                        .set_description(format!("Are you sure you want to delete node: '{title}'?"))
                        .set_buttons(rfd::MessageButtons::OkCancel)
                        .set_level(rfd::MessageLevel::Warning)
                        .show();
                    if confirm == rfd::MessageDialogResult::Ok {
                        configs_clone.borrow_mut().remove(row as usize);
                        table_clone.set_rows(configs_clone.borrow().len() as i32);
                        table_clone.set_selection(-1, -1, -1, -1);
                        *selected_row_clone.borrow_mut() = None;
                        table_clone.redraw();
                    }
                });
            }
            let win = win_clone.clone();
            let node_details_receivers = node_details_receivers.clone();
            menu_btn.add("New", Shortcut::None, MenuFlag::Normal, move |_m| {
                let (tx, rx) = std::sync::mpsc::channel();
                show_node_details(&win, None, tx);
                node_details_receivers.lock().unwrap().push((None, rx));
            });
            menu_btn.popup();

            return false;
        }

        // Double-click logic
        if ev == Event::Push && fltk::app::event_clicks() {
            // respond to double-clicks on cells
            let table_context = table.callback_context();
            if table_context == TableContext::Cell || table_context == TableContext::RowHeader {
                let row = table.callback_row();
                if row >= 0 && (row as usize) < configs_rc.borrow().len() {
                    let cfg = configs_rc.borrow().get(row as usize).cloned();
                    if let Some(cfg) = cfg {
                        let (tx, rx) = std::sync::mpsc::channel();
                        show_node_details(&win_clone, Some(cfg), tx);
                        node_details_receivers.lock().unwrap().push((Some(row as usize), rx));
                    }
                }
                return true;
            }
        }
        false
    });

    let configs_for_draw = nodes.clone();
    let selected_row_draw = selected_row.clone();
    table.draw_cell(move |_t, ctx, row, col, x, y, w, h| {
        // Set font and size for Table cell explicitly
        // This is necessary because without it, the font might be inconsistent
        fltk::draw::set_font(fltk::enums::Font::Helvetica, 14);

        match ctx {
            TableContext::ColHeader => {
                fltk::draw::draw_box(FrameType::ThinUpBox, x, y, w, h, Color::FrameDefault);
                fltk::draw::set_draw_color(Color::Black);
                let text = HEADERS[col as usize];
                fltk::draw::draw_text2(text, x, y, w, h, Align::Center);
            }
            TableContext::RowHeader => {
                fltk::draw::draw_box(FrameType::ThinUpBox, x, y, w, h, Color::FrameDefault);
                fltk::draw::set_draw_color(Color::Black);
                let configs = configs_for_draw.borrow();
                let text = configs.get(row as usize).and_then(|cfg| cfg.remarks.as_deref()).unwrap_or("");
                let is_selected = selected_row_draw.borrow().is_some_and(|sel| sel as i32 == row);
                let check = if cfg!(target_os = "linux") { "✔  " } else { "✔ " };
                let display_text = format!("{}{text}", if is_selected { check } else { "     " });
                fltk::draw::draw_text2(&display_text, x, y, w, h, Align::Left);
            }
            TableContext::Cell => {
                // Only highlight the selected row
                let highlight = selected_row_draw.borrow().is_some_and(|sel| sel as i32 == row);
                let bg = if highlight { Color::Yellow } else { Color::White };
                fltk::draw::draw_box(FrameType::ThinUpBox, x, y, w, h, bg);
                fltk::draw::set_draw_color(Color::Black);
                let configs = configs_for_draw.borrow();
                if let Some(cfg) = configs.get(row as usize) {
                    let tunnel_path_str = cfg.tunnel_path.to_string();
                    let (host, port) = if let Some(client) = &cfg.client {
                        (client.server_host.as_str(), client.server_port.to_string())
                    } else {
                        ("Not a client config", 0.to_string())
                    };
                    let text = match col {
                        0 => host,
                        1 => port.as_str(),
                        2 => tunnel_path_str.as_str(),
                        _ => "",
                    };
                    fltk::draw::draw_text2(text, x, y, w, h, Align::Left);
                }
            }
            _ => (),
        }
    });

    table
}

/// Helper function to refresh the table after config changes
pub fn refresh_table(table: &mut Table, win: &mut Window, row_count: usize) {
    win.resizable(table);
    table.set_rows(row_count as i32);
    update_table_size(table, win.width(), win.height() - MENUBAR_HEIGHT - LOG_HEIGHT);
}

pub fn update_table_size(table: &mut Table, width: i32, height: i32) {
    table.set_size(width, height);
    table.set_row_header_width(ROW_HEADER_WIDTH);
    let col_count = HEADERS.len() as i32;
    table.set_col_width_all((width - ROW_HEADER_WIDTH) / col_count - 1);
    table.set_row_height_all(30);
}

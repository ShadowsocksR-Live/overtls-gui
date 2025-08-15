use crate::{COMMON_DLG_H, COMMON_DLG_W, LOG_HEIGHT, states_manager::WindowState};
use fltk::{
    enums::{Align, Color, Event, FrameType, Shortcut},
    menu::MenuFlag,
    prelude::{GroupExt, MenuExt, TableExt, WidgetBase, WidgetExt},
    table::{Table, TableContext},
    window::Window,
};
use overtls::Config;
use std::cell::RefCell;
use std::rc::Rc;

const HEADERS: [&str; 3] = ["Server Host", "Server Port", "Tunnel Path"];
const ROW_HEADER_WIDTH: i32 = 150;

pub fn create_table(
    top_offset: i32,
    state: &Rc<RefCell<WindowState>>,
    selected_row: &Rc<RefCell<Option<usize>>>,
    nodes: &Rc<RefCell<Vec<Config>>>,
    win: &Window,
) -> Table {
    let mut table = Table::new(0, top_offset, state.borrow().w, state.borrow().h - top_offset - LOG_HEIGHT, "");
    table.set_cols(HEADERS.len() as i32);
    table.set_col_header(true);
    table.set_row_header(true);

    let configs_rc = nodes.clone();
    let state_rc = state.clone();
    // To highlight the selected row
    let selected_row_handle = selected_row.clone();
    let win_clone = win.clone();
    table.handle(move |table, ev| {
        if ev == Event::Released {
            let table_context = table.callback_context();
            let row = table.callback_row();
            // Only respond to left mouse button
            if fltk::app::event_button() == fltk::app::MouseButton::Left as i32 {
                log::debug!("Table context: {:?}, row: {}", table_context, row);
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
                let ws_clone = state_rc.clone();
                let mut table_clone = table.clone();
                menu_btn.add("View details", Shortcut::None, MenuFlag::Normal, move |_m| {
                    let cfg = configs_clone.borrow().get(row as usize).cloned();
                    if let Some(cfg) = cfg
                        && let Some(details) = crate::node_details_dialog::show_node_details(&ws_clone, Some(cfg))
                    {
                        configs_clone.borrow_mut()[row as usize] = details;
                        table_clone.redraw();
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
                            let x = win.x() + (win.w() - COMMON_DLG_W) / 2;
                            let y = win.y() + (win.h() - COMMON_DLG_H) / 2;
                            fltk::dialog::alert(x, y, &format!("Failed to show QR code: {e}"));
                        }
                    }
                });

                let configs_clone = configs_rc.clone();
                let mut table_clone = table.clone();
                let ws_clone = state_rc.clone();
                let selected_row_clone = selected_row_handle.clone();
                menu_btn.add("Delete", Shortcut::None, MenuFlag::MenuDivider, move |_| {
                    let title = configs_clone
                        .borrow()
                        .get(row as usize)
                        .map(|c| c.remarks.clone().unwrap_or_default())
                        .unwrap_or_default();
                    let confirm = fltk::dialog::choice2(
                        ws_clone.borrow().x + fltk::app::event_x(),
                        ws_clone.borrow().y + fltk::app::event_y(),
                        &format!("Are you sure you want to delete node: '{title}'?"),
                        "Yes",
                        "No",
                        "",
                    );
                    if confirm == Some(0) {
                        configs_clone.borrow_mut().remove(row as usize);
                        table_clone.set_rows(configs_clone.borrow().len() as i32);
                        table_clone.set_selection(-1, -1, -1, -1);
                        *selected_row_clone.borrow_mut() = None;
                        table_clone.redraw();
                    }
                });
            }
            let ws_clone = state_rc.clone();
            let configs_clone = configs_rc.clone();
            let mut table_clone = table.clone();
            menu_btn.add("New", Shortcut::None, MenuFlag::Normal, move |_| {
                if let Some(new_cfg) = crate::node_details_dialog::show_node_details(&ws_clone, None) {
                    configs_clone.borrow_mut().push(new_cfg);
                    table_clone.set_rows(configs_clone.borrow().len() as i32);
                    table_clone.redraw();
                }
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
                    if let Some(cfg) = cfg
                        && let Some(details) = crate::node_details_dialog::show_node_details(&state_rc, Some(cfg))
                    {
                        configs_rc.borrow_mut()[row as usize] = details;
                        table.redraw();
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
                let display_text = format!("{}{text}", if is_selected { "✔ " } else { "     " });
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
pub fn refresh_table(
    table: &mut Table,
    win: &mut Window,
    menubar_height: i32,
    state: &Rc<RefCell<crate::states_manager::WindowState>>,
    nodes: &Rc<RefCell<Vec<overtls::Config>>>,
) {
    win.resizable(table);
    table.set_rows(nodes.borrow().len() as i32);
    update_table_size(table, state.borrow().w, state.borrow().h - menubar_height - LOG_HEIGHT);
}

pub fn update_table_size(table: &mut Table, width: i32, height: i32) {
    table.set_size(width, height);
    table.set_row_header_width(ROW_HEADER_WIDTH);
    let col_count = HEADERS.len() as i32;
    table.set_col_width_all((width - ROW_HEADER_WIDTH) / col_count - 1);
    table.set_row_height_all(30);
}

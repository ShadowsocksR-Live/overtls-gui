use crate::selection_ctx;
use crate::{MenuId, ServerNode, about_dlg, details_dlg, model::ServerList, settings_dlg, show_qrcode_dlg};
use std::path::PathBuf;
use std::{cell::RefCell, rc::Rc};
use wxdragon::prelude::*;

/// Dispatch a menu command ID to the same logic used by Frame::on_menu.
/// This allows other UI elements (e.g., double-click on DataView) to reuse menu actions.
pub fn handle_menu_command(parent: &dyn WxWidget, model: &CustomDataViewTreeModel, id: i32) {
    let Ok(menu_id) = MenuId::try_from(id) else {
        log::warn!("Received unknown Menu ID: {id}");
        return;
    };
    match menu_id {
        MenuId::Quit => {
            log::info!("Menu/Toolbar: Quit clicked!");
            parent.close(true);
        }
        MenuId::About => {
            about_dlg::show_about_dialog(parent);
        }
        MenuId::Settings => {
            log::info!("Menu/Toolbar: Settings clicked!");
            settings_dlg::settings_dlg(parent);
        }
        MenuId::ViewDetails => {
            log::info!("Menu/Toolbar: View Details clicked!");
            // If a pending selection has been provided (e.g., by a double-click), use it to prefill
            if let Some(weak) = selection_ctx::get_pending_details() {
                if let Some(rc) = weak.upgrade() {
                    // Scope the immutable borrow just for prefill usage
                    let updated = {
                        let init_borrow = rc.borrow();
                        details_dlg::details_dlg(parent, Some(&*init_borrow))
                    };
                    if let Some(updated_node) = updated {
                        // Commit the changes back to the model
                        {
                            let mut m = rc.borrow_mut();
                            *m = updated_node;
                        }
                        // Notify the view that this item changed
                        let ptr: *const ServerNode = {
                            let b = rc.borrow();
                            &*b as *const _
                        };
                        model.item_changed::<ServerNode>(ptr);
                    }
                } else {
                    // Node no longer exists; open dialog without prefill (no commit target)
                    let _ = details_dlg::details_dlg(parent, None);
                }
            } else {
                // No pending selection; treat as read-only view (or nothing to edit)
                let _ = details_dlg::details_dlg(parent, None);
            }
        }
        MenuId::New => {
            log::info!("Menu/Toolbar: New clicked!");
            if let Some(node) = details_dlg::details_dlg(parent, None) {
                let added = model.with_userdata_mut::<Rc<RefCell<ServerList>>, Option<*const ServerNode>>(|list_rc| {
                    let rc = Rc::new(RefCell::new(node));
                    let ptr: *const ServerNode = {
                        let b = rc.borrow();
                        &*b as *const _
                    };
                    list_rc.borrow_mut().nodes.push(rc);
                    Some(ptr)
                });
                if let Some(Some(ptr)) = added {
                    model.item_added::<ServerNode>(None, ptr);
                }
            }
        }
        MenuId::Delete => {
            log::info!("Menu/Toolbar: Delete clicked!");
            if let Some(weak) = selection_ctx::get_pending_details() {
                if let Some(rc) = weak.upgrade() {
                    // Capture raw pointer for model notification before removal
                    let child_ptr: *const ServerNode = {
                        let b = rc.borrow();
                        &*b as *const _
                    };

                    // Remove from underlying data
                    let removed = model.with_userdata_mut::<Rc<RefCell<ServerList>>, bool>(|list_rc| {
                        let mut list = list_rc.borrow_mut();
                        if let Some(idx) = list.nodes.iter().position(|n| Rc::ptr_eq(n, &rc)) {
                            list.nodes.remove(idx);
                            true
                        } else {
                            false
                        }
                    });

                    if let Some(true) = removed {
                        // Notify view
                        model.item_deleted::<ServerNode>(None, child_ptr);
                        // Clear selection context as the item is gone
                        selection_ctx::set_pending_details(None);
                    } else {
                        log::warn!("Delete requested, but selected node was not found in model.");
                    }
                } else {
                    log::warn!("Delete requested, but the selected item no longer exists.");
                }
            } else {
                log::info!("No selection to delete.");
            }
        }
        MenuId::ShowQrCode => {
            log::info!("Menu/Toolbar: Show QR Code clicked!");
            if let Some(weak) = selection_ctx::get_pending_details()
                && let Some(rc) = weak.upgrade()
            {
                let b = rc.borrow();
                if let Err(e) = show_qrcode_dlg::show_qrcode_dlg(parent, &b) {
                    log::error!("Failed to show QR code dialog: {e}");
                }
            }
        }
        MenuId::ExportNode => {
            log::info!("Menu/Toolbar: Export Node clicked!");
            if let Some(weak) = selection_ctx::get_pending_details()
                && let Some(rc) = weak.upgrade()
            {
                let node = rc.borrow();
                if let Ok(json_str) = serde_json::to_string_pretty(&*node) {
                    let root = dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).to_string_lossy().to_string();
                    let dialog = FileDialog::builder(parent)
                        .with_message("Save as")
                        .with_style(FileDialogStyle::Save | FileDialogStyle::OverwritePrompt)
                        .with_default_dir(&root)
                        .with_default_file("exported_node.json")
                        .with_wildcard("JSON files (*.json)|*.json|All files (*.*)|*.*")
                        .build();
                    if dialog.show_modal() == wxdragon::id::ID_OK {
                        if let Some(path_option) = dialog.get_path() {
                            if std::fs::write(&path_option, json_str).is_ok() {
                                log::debug!("Node exported to: {}", path_option);
                            }
                        }
                    } else {
                        log::info!("File Dialog: Save cancelled.");
                    }
                    dialog.destroy();
                }
            }
        }

        _ => {
            log::warn!("Unhandled Menu ID: {menu_id:?}");
        }
    }
}

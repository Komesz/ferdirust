use cef::*;

use crate::app::SharedServiceManager;

// Per-service context menu actions, encoded into the menu command id as
// menu_base() + service_index * MENU_IDS_PER_SERVICE + action.
const MENU_ACTION_RELOAD: u32 = 0;
const MENU_ACTION_HARD_RELOAD: u32 = 1;
const MENU_ACTION_REPAIR: u32 = 2;
const MENU_ACTION_RESET: u32 = 3;
// Submenu containers need their own (never-dispatched) command ids
const MENU_SUBMENU_REPAIR: u32 = 4;
const MENU_SUBMENU_RESET: u32 = 5;
const MENU_IDS_PER_SERVICE: u32 = 8;

fn menu_base() -> u32 {
    MenuId::USER_FIRST.get_raw()
}

// ContextMenuHandler for the sidebar: right-clicking a service icon (an
// anchor with href "ferdisvc:<id>") shows Reload / Hard Reload / Repair /
// Reset. The destructive entries sit in confirm submenus so a stray click
// can't trigger them.
wrap_context_menu_handler! {
    struct SidebarContextMenuHandler {
        service_manager: SharedServiceManager,
    }

    impl ContextMenuHandler {
        fn on_before_context_menu(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            params: Option<&mut ContextMenuParams>,
            model: Option<&mut MenuModel>,
        ) {
            let Some(model) = model else { return };
            // Never show the default (inspect/copy) menu inside the sidebar;
            // an empty model means no menu appears at all.
            model.clear();

            let Some(params) = params else { return };
            let link_userfree = params.unfiltered_link_url();
            let link = CefString::from(&link_userfree).to_string();
            let Some(service_id) = link.strip_prefix("ferdisvc:") else { return };

            let (index, name) = {
                let Ok(guard) = self.service_manager.lock() else { return };
                let Some(mgr) = guard.as_ref() else { return };
                let Some((index, instance)) = mgr
                    .services
                    .iter()
                    .enumerate()
                    .find(|(_, s)| s.config.id == service_id)
                else {
                    return;
                };
                (index as u32, instance.config.name.clone())
            };

            let base = menu_base() + index * MENU_IDS_PER_SERVICE;
            model.add_item(
                (base + MENU_ACTION_RELOAD) as i32,
                Some(&CefString::from(format!("Reload {name}").as_str())),
            );
            model.add_item(
                (base + MENU_ACTION_HARD_RELOAD) as i32,
                Some(&CefString::from("Hard Reload (bypass cache)")),
            );
            model.add_separator();
            if let Some(repair) = model.add_sub_menu(
                (base + MENU_SUBMENU_REPAIR) as i32,
                Some(&CefString::from("Repair (keeps login)")),
            ) {
                repair.add_item(
                    (base + MENU_ACTION_REPAIR) as i32,
                    Some(&CefString::from("Clear caches and local data")),
                );
            }
            if let Some(reset) = model.add_sub_menu(
                (base + MENU_SUBMENU_RESET) as i32,
                Some(&CefString::from("Reset (logs out!)")),
            ) {
                reset.add_item(
                    (base + MENU_ACTION_RESET) as i32,
                    Some(&CefString::from(
                        format!("Yes, wipe all data for {name}").as_str(),
                    )),
                );
            }
        }

        fn on_context_menu_command(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _params: Option<&mut ContextMenuParams>,
            command_id: ::std::os::raw::c_int,
            _event_flags: EventFlags,
        ) -> ::std::os::raw::c_int {
            let base = menu_base();
            let cmd = command_id as u32;
            if cmd < base {
                return 0;
            }
            let index = ((cmd - base) / MENU_IDS_PER_SERVICE) as usize;
            let action = (cmd - base) % MENU_IDS_PER_SERVICE;

            let Ok(mut guard) = self.service_manager.lock() else { return 0 };
            let Some(mgr) = guard.as_mut() else { return 0 };
            let Some(service_id) = mgr.services.get(index).map(|s| s.config.id.clone())
            else {
                return 0;
            };

            match action {
                MENU_ACTION_RELOAD => mgr.reload_service(&service_id),
                MENU_ACTION_HARD_RELOAD => mgr.hard_reload_service(&service_id),
                MENU_ACTION_REPAIR => mgr.repair_service(&service_id),
                MENU_ACTION_RESET => mgr.reset_service(&service_id),
                _ => return 0,
            }
            1
        }
    }
}

// DisplayHandler that intercepts "ferdirust:switch:{id}" title changes
wrap_display_handler! {
    struct SidebarDisplayHandler {
        service_manager: SharedServiceManager,
    }

    impl DisplayHandler {
        fn on_title_change(&self, _browser: Option<&mut Browser>, title: Option<&CefString>) {
            if let Some(title) = title {
                let title_str = title.to_string();
                if let Some(id) = title_str.strip_prefix("ferdirust:switch:") {
                    if let Ok(mut guard) = self.service_manager.lock() {
                        if let Some(mgr) = guard.as_mut() {
                            mgr.switch_to_id(id);
                        }
                    }
                }
                // Same actions the context menu offers, addressable from the
                // sidebar page (and DevTools) as "ferdirust:action:<verb>:<id>"
                if let Some(rest) = title_str.strip_prefix("ferdirust:action:") {
                    if let Some((verb, id)) = rest.split_once(':') {
                        if let Ok(mut guard) = self.service_manager.lock() {
                            if let Some(mgr) = guard.as_mut() {
                                match verb {
                                    "reload" => mgr.reload_service(id),
                                    "hardreload" => mgr.hard_reload_service(id),
                                    "repair" => mgr.repair_service(id),
                                    "reset" => mgr.reset_service(id),
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// Client for the sidebar BrowserView
wrap_client! {
    pub struct SidebarClient {
        pub service_manager: SharedServiceManager,
    }

    impl Client {
        fn display_handler(&self) -> Option<DisplayHandler> {
            Some(SidebarDisplayHandler::new(self.service_manager.clone()))
        }

        fn context_menu_handler(&self) -> Option<ContextMenuHandler> {
            Some(SidebarContextMenuHandler::new(self.service_manager.clone()))
        }
    }
}

impl SidebarClient {
    pub fn create(service_manager: SharedServiceManager) -> Client {
        SidebarClient::new(service_manager)
    }
}

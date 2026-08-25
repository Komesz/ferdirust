use cef::*;

use crate::app::SharedServiceManager;

wrap_window_delegate! {
    pub struct FerdiWindowDelegate {
        pub service_manager: SharedServiceManager,
    }

    impl ViewDelegate {}

    impl PanelDelegate {}

    impl WindowDelegate {
        fn window_runtime_style(&self) -> RuntimeStyle {
            RuntimeStyle::ALLOY
        }

        fn on_window_created(&self, window: Option<&mut Window>) {
            let Some(window) = window else { return };

            window.set_title(Some(&CefString::from("Ferdirust")));

            // Set up layout inside on_window_created (critical for CEF Views)
            if let Ok(mut guard) = self.service_manager.lock() {
                if let Some(mgr) = guard.as_mut() {
                    mgr.setup_window(window, &self.service_manager);
                }
            }

            window.show();
            window.center_window(Some(&Size {
                width: 1280,
                height: 800,
            }));
        }

        fn on_window_bounds_changed(&self, _window: Option<&mut Window>, new_bounds: Option<&Rect>) {
            if let Some(bounds) = new_bounds {
                if let Ok(guard) = self.service_manager.lock() {
                    if let Some(mgr) = guard.as_ref() {
                        mgr.layout_views(bounds.width, bounds.height);
                    }
                }
            }
        }

        fn on_window_destroyed(&self, _window: Option<&mut Window>) {
            quit_message_loop();
        }

        fn initial_bounds(&self, _window: Option<&mut Window>) -> Rect {
            Rect {
                x: 0,
                y: 0,
                width: 1280,
                height: 800,
            }
        }

        fn can_resize(&self, _window: Option<&mut Window>) -> ::std::os::raw::c_int {
            1
        }

        fn can_maximize(&self, _window: Option<&mut Window>) -> ::std::os::raw::c_int {
            1
        }

        fn can_minimize(&self, _window: Option<&mut Window>) -> ::std::os::raw::c_int {
            1
        }

        fn can_close(&self, _window: Option<&mut Window>) -> ::std::os::raw::c_int {
            1
        }

        fn on_key_event(
            &self,
            _window: Option<&mut Window>,
            event: Option<&KeyEvent>,
        ) -> ::std::os::raw::c_int {
            if let Some(event) = event {
                // Only handle key-up to avoid double-firing
                if event.type_ != KeyEventType::from(cef::sys::cef_key_event_type_t::KEYEVENT_KEYUP) {
                    return 0;
                }

                let ctrl = event.modifiers & (1 << 2) != 0; // EVENTFLAG_CONTROL_DOWN
                let shift = event.modifiers & (1 << 1) != 0; // EVENTFLAG_SHIFT_DOWN

                if ctrl {
                    // Ctrl+1 through Ctrl+9 to switch services
                    let key_num = match event.windows_key_code {
                        0x31..=0x39 => Some((event.windows_key_code - 0x31) as usize), // '1'-'9'
                        _ => None,
                    };

                    if let Some(index) = key_num {
                        if let Ok(mut guard) = self.service_manager.lock() {
                            if let Some(mgr) = guard.as_mut() {
                                mgr.switch_to_index(index);
                                return 1;
                            }
                        }
                    }

                    // Ctrl+R to reload, Ctrl+Shift+R to hard reload
                    // (bypasses the HTTP cache and the service worker)
                    if event.windows_key_code == 0x52 {
                        if let Ok(guard) = self.service_manager.lock() {
                            if let Some(mgr) = guard.as_ref() {
                                if shift {
                                    mgr.hard_reload_active();
                                } else {
                                    mgr.reload_active();
                                }
                            }
                        }
                        return 1;
                    }
                }
            }
            0
        }
    }
}

impl FerdiWindowDelegate {
    pub fn create(service_manager: SharedServiceManager) -> WindowDelegate {
        FerdiWindowDelegate::new(service_manager)
    }
}

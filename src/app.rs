use cef::*;
use std::sync::{Arc, Mutex};

use crate::browser_process::FerdiBrowserProcessHandler;
use crate::service::config::AppConfig;
use crate::service::manager::ServiceManager;

pub type SharedServiceManager = Arc<Mutex<Option<ServiceManager>>>;

wrap_app! {
    pub struct FerdiApp {
        pub service_manager: SharedServiceManager,
        pub config: Arc<AppConfig>,
    }

    impl App {
        fn on_before_command_line_processing(
            &self,
            _process_type: Option<&CefString>,
            command_line: Option<&mut CommandLine>,
        ) {
            if let Some(cmd) = command_line {
                // Wayland native support
                cmd.append_switch_with_value(
                    Some(&CefString::from("ozone-platform-hint")),
                    Some(&CefString::from("auto")),
                );
                cmd.append_switch(Some(&CefString::from("use-views")));

                // Enable WebRTC / media stream
                cmd.append_switch(Some(&CefString::from("enable-media-stream")));

                // System-audio loopback for screenshare goes through PulseAudio and
                // segfaults the audio service against pipewire-pulse (crash in
                // media::PulseLoopbackAudioStream). Disable it; screen video still
                // works via the PipeWire portal capturer.
                cmd.append_switch_with_value(
                    Some(&CefString::from("disable-features")),
                    Some(&CefString::from("PulseaudioLoopbackForScreenShare")),
                );

                // GPU acceleration
                cmd.append_switch(Some(&CefString::from("enable-gpu")));

                // Allow running insecure content (some services need it)
                cmd.append_switch(Some(&CefString::from("allow-running-insecure-content")));
            }
        }

        fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
            Some(FerdiBrowserProcessHandler::create(
                self.service_manager.clone(),
                self.config.clone(),
            ))
        }
    }
}

pub fn create_app() -> App {
    let config = Arc::new(AppConfig::load());
    let service_manager = Arc::new(Mutex::new(None));
    FerdiApp::new(service_manager, config)
}

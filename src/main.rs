mod app;
mod browser_process;
mod client;
mod handlers;
mod ipc;
mod service;
mod sidebar;
mod window_delegate;

use cef::*;

fn main() {
    let args = args::Args::new();

    // Configure the CEF API version before any other CEF calls.
    // On Linux, libcef_dll_wrapper is not linked, so we must call this manually.
    api_hash(sys::CEF_API_VERSION_LAST, 0);

    let mut app = app::create_app();

    // All processes (browser + subprocesses) start here.
    // execute_process returns -1 for browser process, >= 0 for subprocesses.
    let exit_code = execute_process(Some(args.as_main_args()), Some(&mut app), std::ptr::null_mut());
    if exit_code >= 0 {
        std::process::exit(exit_code);
    }

    // Browser process continues here

    // Delete service partitions flagged by "Reset" — must happen before CEF
    // opens any profile files.
    service::partition::sweep_marked_partitions();

    let mut settings = Settings::default();
    settings.no_sandbox = 1;
    settings.root_cache_path = CefString::from(
        dirs::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("~/.local/share"))
            .join("ferdirust")
            .join("cef_cache")
            .to_str()
            .unwrap_or("/tmp/ferdirust_cache"),
    );
    settings.log_severity = LogSeverity::from(cef::sys::cef_log_severity_t::LOGSEVERITY_WARNING);

    let result = initialize(
        Some(args.as_main_args()),
        Some(&settings),
        Some(&mut app),
        std::ptr::null_mut(),
    );
    if result == 0 {
        eprintln!("CEF initialization failed");
        std::process::exit(1);
    }

    run_message_loop();
    shutdown();
}

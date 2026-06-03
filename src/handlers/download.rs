use cef::*;
use std::path::PathBuf;
use std::process::Command;

wrap_download_handler! {
    pub struct FerdiDownloadHandler {
        pub download_dir: String,
    }

    impl DownloadHandler {
        fn on_before_download(
            &self,
            _browser: Option<&mut Browser>,
            download_item: Option<&mut DownloadItem>,
            _suggested_name: Option<&CefString>,
            callback: Option<&mut BeforeDownloadCallback>,
        ) -> ::std::os::raw::c_int {
            if let (Some(item), Some(cb)) = (download_item, callback) {
                let filename = item.suggested_file_name();
                let filename_str = CefString::from(&filename).to_string();
                let save_path = PathBuf::from(&self.download_dir).join(&filename_str);

                let path = CefString::from(save_path.to_str().unwrap_or(&filename_str));
                // show_dialog = 0 means auto-save without dialog
                cb.cont(Some(&path), 0);
            }
            0
        }

        fn on_download_updated(
            &self,
            _browser: Option<&mut Browser>,
            download_item: Option<&mut DownloadItem>,
            _callback: Option<&mut DownloadItemCallback>,
        ) {
            if let Some(item) = download_item {
                if item.is_complete() != 0 {
                    let filename = item.suggested_file_name();
                    let filename_str = CefString::from(&filename).to_string();
                    // Send desktop notification
                    let body = format!("{} saved to {}", filename_str, self.download_dir);
                    let _ = Command::new("notify-send")
                        .arg("--app-name=Ferdirust")
                        .arg("Download Complete")
                        .arg(&body)
                        .spawn();
                }
            }
        }
    }
}

impl FerdiDownloadHandler {
    pub fn create(download_dir: String) -> DownloadHandler {
        FerdiDownloadHandler::new(download_dir)
    }
}

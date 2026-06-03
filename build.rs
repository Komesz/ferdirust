use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Set RPATH to $ORIGIN so the binary finds libcef.so in its own directory
    println!("cargo::rustc-link-arg=-Wl,-rpath,$ORIGIN");

    if let Ok(cef_dir) = env::var("DEP_CEF_CEF_DIR") {
        let marker = out_dir.join("cef_dir.txt");
        fs::write(&marker, &cef_dir).unwrap();
        println!("cargo::rustc-env=CEF_DIR={cef_dir}");
    }
}

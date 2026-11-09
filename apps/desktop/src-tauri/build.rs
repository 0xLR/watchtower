fn main() {
    println!(
        "cargo:rustc-env=WATCHTOWER_TARGET={}",
        std::env::var("TARGET").expect("Cargo supplies TARGET")
    );
    tauri_build::build();
}

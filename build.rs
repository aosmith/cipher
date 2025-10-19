fn main() {
    // Link SystemConfiguration framework on iOS for libp2p dependencies
    #[cfg(target_os = "ios")]
    {
        println!("cargo:rustc-link-lib=framework=SystemConfiguration");
    }

    tauri_build::build()
}

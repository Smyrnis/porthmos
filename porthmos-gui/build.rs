use embed_manifest::{embed_manifest, manifest::Setting, new_manifest};

const ICON: &str = "../assets/icon/porthmos.ico";

fn main() {
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        embed_manifest(new_manifest("Porthmos GUI").long_path_aware(Setting::Enabled))
            .expect("unable to embed the Windows manifest");
        winresource::WindowsResource::new()
            .set_icon(ICON)
            .set("ProductName", "Porthmos")
            .set("FileDescription", "Porthmos")
            .compile()
            .expect("unable to embed the Windows icon and version information");
    }
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={ICON}");
}

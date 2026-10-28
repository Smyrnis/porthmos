use embed_manifest::{embed_manifest, manifest::Setting, new_manifest};

fn main() {
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        embed_manifest(new_manifest("Porthmos").long_path_aware(Setting::Enabled))
            .expect("unable to embed the Windows manifest");
    }
    println!("cargo:rerun-if-changed=build.rs");
}

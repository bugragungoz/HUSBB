//! Embeds the HUSBB icon and version information into the Windows executable.

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico")
            .set("ProductName", "HUSBB")
            .set("FileDescription", "HUSBB - Hantek DSO2D15 USB Bridge")
            .set("LegalCopyright", "MIT License");
        res.compile().expect("compile Windows resources (needs rc.exe from the Windows SDK)");
    }
}

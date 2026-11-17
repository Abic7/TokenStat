fn main() {
    println!("cargo:rerun-if-changed=assets/tokenstat.rc");
    println!("cargo:rerun-if-changed=assets/tokenstat.ico");
    // Embed the exe icon only when building on Windows for Windows (needs rc.exe from the MSVC build tools).
    if cfg!(windows) && std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("assets/tokenstat.rc", embed_resource::NONE).manifest_optional().unwrap();
    }
}

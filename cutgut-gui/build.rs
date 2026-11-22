fn main() {
    println!("cargo:rerun-if-changed=assets/cutgut.rc");
    println!("cargo:rerun-if-changed=assets/cutgut.ico");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("assets/cutgut.rc", embed_resource::NONE)
            .manifest_required()
            .expect("failed to embed the CutGut icon");
    }
}

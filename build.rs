fn main() {
    #[cfg(windows)]
    {
        let icon = "assets/logo/fastview.ico";
        println!("cargo:rerun-if-changed=assets/fastview.rc");
        println!("cargo:rerun-if-changed={icon}");
        // 首次生成图标前允许跳过，避免生成器自身无法编译。
        if std::path::Path::new(icon).exists() {
            embed_resource::compile("assets/fastview.rc", embed_resource::NONE)
                .manifest_optional()
                .expect("embed icon resource");
        }
    }
}

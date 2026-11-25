//! 为独立桌面适配测试提供 Windows 系统控件清单。
fn main() {
    println!("cargo:rerun-if-changed=windows-tests.manifest");

    // 按目标平台判断，避免交叉编译时把 MSVC 链接参数传给其他平台。
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows")
        || std::env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc")
    {
        return;
    }

    let manifest = std::path::PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("缺少桌面适配包目录"),
    )
    .join("windows-tests.manifest");

    // 本包只构建 rlib 和测试程序；链接参数覆盖 --lib 单元测试，且不会传递给宿主。
    // TaskDialogIndirect 需要 Common Controls v6，必须在 Windows 加载测试程序前声明。
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
}

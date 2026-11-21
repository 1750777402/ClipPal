//! 构建时处理版本配置，再交给 Tauri 生成权限和打包元数据。
/// 只在编译期间执行；生成的配置由薄启动壳嵌入，不打印其中的密钥。
fn main() {
    println!("cargo:rerun-if-changed=config.json");
    // 按版本嵌入配置，避免单机产物携带未使用的云端地址和部署令牌。
    let source = std::fs::read_to_string("config.json").expect("无法读取打包配置");
    let mut config: serde_json::Value = serde_json::from_str(&source).expect("打包配置格式无效");
    if std::env::var_os("CARGO_FEATURE_LOCAL").is_some() {
        // 单机版只需要本地加密配置，不把未启用的服务配置写进可执行文件。
        let local = config.get("app_secret").cloned().expect("缺少本地加密配置");
        config = serde_json::json!({"app_secret": local});
    }
    // 写 Cargo 专属输出目录，不覆盖开发者的原始 config.json。
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("缺少构建输出目录"));
    std::fs::write(output.join("packaged-config.json"), serde_json::to_vec(&config).expect("打包配置编码失败"))
        .expect("写入版本配置失败");
    tauri_build::build()
}

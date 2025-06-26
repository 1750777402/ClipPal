use log::LevelFilter;
use log4rs::{
    Config,
    append::{console::ConsoleAppender, file::FileAppender},
    config::{Appender, Root},
    encode::pattern::PatternEncoder,
};

/// 初始化日志系统，将日志文件放到统一的logs目录下
fn init_logging() {
    // 获取logs目录路径
    let log_file_path = match get_logs_dir() {
        Some(mut logs_dir) => {
            logs_dir.push("clip_pal.log");
            logs_dir
        }
        None => {
            eprintln!("无法获取logs目录，使用当前目录");
            std::path::PathBuf::from("clip_pal.log")
        }
    };

    // 创建控制台输出器
    let stdout = ConsoleAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            "[{d(%Y-%m-%d %H:%M:%S)}] [{h({l})}] {m}{n}",
        )))
        .build();

    // 创建文件输出器
    let logfile = match FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            "[{d(%Y-%m-%d %H:%M:%S)}] [{l}] [{M}] {m}{n}",
        )))
        .build(&log_file_path)
    {
        Ok(appender) => appender,
        Err(e) => {
            eprintln!("创建日志文件失败: {}, 路径: {:?}", e, log_file_path);
            // 使用env_logger作为后备
            env_logger::init();
            return;
        }
    };

    // 构建log4rs配置
    let config = match Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        .appender(Appender::builder().build("logfile", Box::new(logfile)))
        .build(
            Root::builder()
                .appender("stdout")
                .appender("logfile")
                .build(LevelFilter::Info),
        ) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("构建日志配置失败: {}", e);
            env_logger::init();
            return;
        }
    };

    // 初始化log4rs
    if let Err(e) = log4rs::init_config(config) {
        eprintln!("初始化log4rs失败: {}", e);
        env_logger::init();
    } else {
        println!("日志系统初始化成功，日志文件: {:?}", log_file_path);
    }
}

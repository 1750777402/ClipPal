use log::LevelFilter;
use log4rs::{
    Config,
    append::{
        console::ConsoleAppender,
        rolling_file::{
            RollingFileAppender,
            policy::compound::{
                CompoundPolicy, roll::fixed_window::FixedWindowRoller, trigger::size::SizeTrigger,
            },
        },
    },
    config::{Appender, Root},
    encode::pattern::PatternEncoder,
};

use crate::utils::file_dir::get_logs_dir;

/// 初始化日志系统，将日志文件放到统一的logs目录下
/// 自动管理日志文件轮转和清理，无需外部干预
pub fn init_logging(level: LevelFilter) {
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

    // 配置日志轮转策略
    // 单文件最大12MB，保证5个文件不超过60MB总限制
    let trigger = SizeTrigger::new(12 * 1024 * 1024); // 12MB触发轮转

    // 配置固定窗口滚动策略，最多保留4个历史文件（加上当前文件总共5个）
    let roller = match get_logs_dir() {
        Some(logs_dir) => {
            let pattern = logs_dir
                .join("clip_pal.{}.log")
                .to_string_lossy()
                .to_string();
            FixedWindowRoller::builder()
                .build(&pattern, 4) // 保留4个历史文件，加上当前文件总共5个
                .expect("Failed to create fixed window roller")
        }
        None => FixedWindowRoller::builder()
            .build("clip_pal.{}.log", 4)
            .expect("Failed to create fixed window roller"),
    };

    // 创建复合策略，结合大小触发和固定窗口滚动
    let policy = CompoundPolicy::new(Box::new(trigger), Box::new(roller));

    // 创建滚动文件输出器
    let logfile = match RollingFileAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            "[{d(%Y-%m-%d %H:%M:%S)}] [{l}] [{M}] {m}{n}",
        )))
        .build(&log_file_path, Box::new(policy))
    {
        Ok(appender) => appender,
        Err(e) => {
            eprintln!("创建滚动日志文件失败: {}, 路径: {:?}", e, log_file_path);
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
                .build(level),
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
        eprintln!("日志系统初始化失败: {}", e);
        env_logger::init();
    } else {
        log::info!("日志系统初始化成功，日志文件: {:?}", log_file_path);
    }
}

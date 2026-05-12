use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, fmt};
///
/// 日志配置初始化
///
pub fn init_log(
    crate_name: &str,
    file_name: &str,
) -> Vec<tracing_appender::non_blocking::WorkerGuard> {
    // // 消费log门面日志 转为 tracing Event日志
    // LogTracer::builder()
    //     // .with_max_level(log::LevelFilter::Error)
    //     .init()
    //     .expect("[PEAR] LogTracer 初始化失败");

    let stdenv = EnvFilter::new(format!("{}=warn", crate_name));
    //使用同步的即可，不需要进行改变，non_blocking会开单独线程进行处理
    let stderr = std::io::stderr();
    let (non_blocking, stderr_guard) = tracing_appender::non_blocking(stderr);
    // 标准控制台输出layer(同步)
    let fmt_layer = fmt::layer()
        //显示等级(info warn等)
        .with_level(true)
        // 指定标准控制台输出
        .with_writer(non_blocking)
        // 日志等级过滤
        .with_filter(stdenv);

    let file_env = EnvFilter::new(format!("{}=info", crate_name));
    // 文件 appender 指定日志文件输出目录和文件名前缀
    // daily 指定生成文件名日期到年月日
    // 如： test-log.2023-08-30
    let file_appender = tracing_appender::rolling::daily("logs/", file_name);
    // 生成非阻塞写入器
    let (non_blocking, file_log_guard) = tracing_appender::non_blocking(file_appender);
    // 文件输出层
    let file_layer = fmt::layer()
        // 移除输出内容中的 颜色或其它格式相关转义字符
        .with_ansi(false)
        .with_writer(non_blocking)
        // 日志等级过滤
        .with_filter(file_env);

    // 生成注册中心 Registry 绑定多个输出层
    let collector = tracing_subscriber::registry()
        .with(file_layer)
        .with(fmt_layer);

    // 订阅者全局注册
    tracing::subscriber::set_global_default(collector).expect("Tracing collect error");

    vec![stderr_guard, file_log_guard]
}

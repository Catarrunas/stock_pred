use dotenv::from_filename;
use std::env;
use tracing_subscriber;


/// Initialize tracing
pub fn init_tracing(
    stdout: bool,
    filter: tracing::Level,
) -> tracing_appender::non_blocking::WorkerGuard {
     // Load environment variables from vars.env.
     let _ = from_filename("vars.env");
     // Read log file settings from the environment.
     let log_dir = env::var("LOG_DIR").unwrap_or_else(|_| "log".to_string());
     let log_file = env::var("LOG_FILE").unwrap_or_else(|_| "app.log".to_string());

     // Decide which output should be used
    let (writer, guard) = if stdout {
        let (writer, guard) = tracing_appender::non_blocking(std::io::stdout());
        (writer, guard)
    } else {
        let file_appender = tracing_appender::rolling::daily(&log_dir, &log_file);
        let (writer, guard) = tracing_appender::non_blocking(file_appender);
        (writer, guard)
    };

    // Initialize tracing instance
    tracing_subscriber::fmt()
        .with_writer(writer)
        .with_max_level(filter)
        .with_ansi(stdout)
        .with_target(false)
        .with_file(false)
        .with_thread_ids(true)
        .with_thread_names(true)
        .init();

    guard
}

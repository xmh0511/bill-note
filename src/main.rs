use config_file::FromConfigFile;
use salvo::prelude::*;
use serde::Deserialize;
use tracing_appender::non_blocking::WorkerGuard;
mod error;
mod orm;

#[derive(Deserialize)]
struct Config {
    host: String,
    database_url: String,
}

#[handler]
async fn hello_zh() -> error::JsonResult<&'static str> {
    Ok("你好，世界！")
}

#[tokio::main]
async fn main() {
    let config = Config::from_config_file("./config.toml").unwrap();
    let _trracing_guard = init_log();
    //orm::init_dao(config.database_url).await;
    let acceptor = TcpListener::new(config.host).bind().await;
    let router = Router::new().push(Router::with_path("hello").get(hello_zh));

    Server::new(acceptor).serve(router).await;
}

fn init_log() -> Option<WorkerGuard> {
    use time::{UtcOffset, macros::format_description};
    use tracing_subscriber::fmt::time::OffsetTime;

    let local_time = OffsetTime::new(
        UtcOffset::from_hms(8, 0, 0).unwrap(),
        format_description!("[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:3]"),
    );
    if cfg!(debug_assertions) {
        tracing_subscriber::fmt().with_timer(local_time).init();
        None
    } else {
        let file_appender = tracing_appender::rolling::hourly("./logs", "bill.log");
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
        tracing_subscriber::fmt()
            .with_timer(local_time)
            .with_max_level(tracing::Level::INFO)
            .with_writer(non_blocking)
            .init();
        Some(guard)
    }
}

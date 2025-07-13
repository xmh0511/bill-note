use config_file::FromConfigFile;
use salvo::jwt_auth::{ConstDecoder, HeaderFinder, QueryFinder};
use salvo::prelude::*;
use serde::Deserialize;
use tracing_appender::non_blocking::WorkerGuard;
mod auth;
mod bill;
mod error;
mod orm;
use auth::{Authority, JwtClaims};

#[derive(Deserialize)]
struct Config {
    host: String,
    database_url: String,
    secret_key: String,
    base_path: String,
}

#[handler]
async fn hello_zh() -> error::JsonResult<&'static str> {
    Ok("你好，世界！")
}

#[tokio::main]
async fn main() {
    let config = Config::from_config_file("./config.toml").unwrap();
    let _trracing_guard = init_log();
    orm::init_dao(config.database_url).await;
    let acceptor = TcpListener::new(config.host).bind().await;

    let auth_handler: JwtAuth<JwtClaims, _> =
        JwtAuth::new(ConstDecoder::from_secret(config.secret_key.as_bytes()))
            .finders(vec![
                Box::new(HeaderFinder::new()),
                Box::new(QueryFinder::new("token")),
                // Box::new(CookieFinder::new("jwt_token")),
            ])
            .force_passed(true);

    let authority = Authority::new(config.secret_key);

    let router = if config.base_path.is_empty() {
        Router::new()
    } else {
        Router::with_path(config.base_path)
    }
    .hoop(authority);
    let router = router.push(Router::with_path("login").post(bill::login));
    let router = router.push(Router::with_path("reg").post(bill::registry));

    let bill_router = Router::with_path("bill");
    let bill_router = bill_router.push(Router::with_path("list").get(bill::bill_list));
    let bill_router = bill_router.push(Router::with_path("add").post(bill::bill_add));
    let bill_router = bill_router.push(Router::with_path("del").post(bill::del_bill));

    let tag_router = Router::with_path("tag");
    let tag_router = tag_router.push(Router::with_path("add").post(bill::add_tag));
    let tag_router = tag_router.push(Router::with_path("list").post(bill::tag_list));
    let tag_router = tag_router.push(Router::with_path("del").post(bill::del_tag));

    let auth_router = Router::with_hoop(auth_handler)
        .hoop(auth::check_auth_id)
        .push(bill_router)
        .push(tag_router);

    let router = router.push(auth_router);

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

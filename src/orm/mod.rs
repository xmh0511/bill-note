use crate::error::{JsonErr, JsonResult};
use sea_orm::{Database, DatabaseConnection};
use serde_json::json;
use std::sync::OnceLock;

pub mod model;

static DAO: OnceLock<DatabaseConnection> = OnceLock::new();

pub fn get_dao() -> JsonResult<&'static DatabaseConnection> {
    DAO.get().ok_or(JsonErr::from_value(json!({
        "status":"error",
        "code":500,
        "msg":"database is unusable"
    })))
}

pub async fn init_dao(database_url: String) {
    let db: DatabaseConnection = Database::connect(database_url)
        .await
        .expect("database init error");
    DAO.set(db)
        .expect("not possible that other threads can init database connection");
}

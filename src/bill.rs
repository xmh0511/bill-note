use crate::{auth::Authority, error::*, orm};
use anyhow::anyhow;
use chrono::NaiveDate;
use salvo::prelude::*;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, Statement, Value};
use serde_json::json;

use crate::orm::model::{prelude::*, *};

#[handler]
pub async fn login(req: &mut Request, res: &mut Response, depot: &mut Depot) -> JsonResult<()> {
    let account = req
        .form::<String>("account")
        .await
        .ok_or(JsonErr::from_error(400, anyhow!("account is required")))?;
    let pass = req
        .form::<String>("password")
        .await
        .ok_or(JsonErr::from_error(400, anyhow!("password is required")))?;

    let salt_pass = format!("{:x}", md5::compute(pass));
    let db = orm::get_dao()?;
    let info = UserTb::find()
        .filter(user_tb::Column::Account.eq(account))
        .filter(user_tb::Column::Pass.eq(salt_pass))
        .one(db)
        .await
        .map_err(|e| JsonErr::from_error(500, anyhow!(e)))?
        .ok_or(JsonErr::from_error(400, anyhow!("账户或密码错误")))?;
    let authority = depot
        .obtain::<Authority>()
        .map_err(|e| JsonErr::from_error(500, anyhow!("签名错误 {e:?}")))?;
    let token = authority.sign(info.id, 30 * 24 * 3600)?;
    res.render(Text::Json(
        json!({
            "status":"success",
            "code":200,
            "msg":{
                "data":token
            }
        })
        .to_string(),
    ));
    Ok(())
}

#[handler]
pub async fn bill_list(req: &mut Request, res: &mut Response, depot: &mut Depot) -> JsonResult<()> {
    let user_id = *depot
        .get::<i32>("user_id")
        .map_err(|_e| JsonErr::from_error(403, anyhow!("unknown users")))?;
    let start_date = req
        .query::<String>("begin")
        .ok_or(JsonErr::from_error(400, anyhow!("未获取到起始日期")))?;
    let begin = NaiveDate::parse_from_str(&start_date, "%Y-%m-%d")
        .map_err(|e| JsonErr::from_error(403, anyhow!("起始日期解析错误：{}", e)))?;

    let end_date = req
        .query::<String>("end")
        .ok_or(JsonErr::from_error(400, anyhow!("未获取到结束日期")))?;
    let end = NaiveDate::parse_from_str(&end_date, "%Y-%m-%d")
        .map_err(|e| JsonErr::from_error(403, anyhow!("结束日期解析错误：{}", e)))?;

    let delta_time = end.signed_duration_since(begin);

    if delta_time.num_seconds() < 0 {
        res.render(Text::Json(
            json!({
                "status":"error",
                "code":400,
                "msg":"无效的日期范围"
            })
            .to_string(),
        ));
        return Ok(());
    }

    let sql = "SELECT
	bill_tb.*,
	tag_tb.`name` AS tagName 
FROM
	bill_tb
	LEFT JOIN tag_tb ON tag_tb.id = bill_tb.tag_id 
WHERE
	bill_tb.user_id = ?
	AND bill_tb.transaction_date <= ? AND bill_tb.transaction_date >= ?";
    let db = orm::get_dao()?;

    let result = BillTb::find()
        .from_raw_sql(Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::MySql,
            sql,
            [
                Value::Int(Some(user_id)),
                Value::ChronoDate(Some(Box::new(end))),
                Value::ChronoDate(Some(Box::new(begin))),
            ],
        ))
        .into_json()
        .all(db)
        .await
        .map_err(|e| JsonErr::from_error(400, anyhow!(e)))?;

    let sql2 = "SELECT
	SUM(pay) as pay_amount
FROM
	bill_tb 
WHERE
	bill_tb.user_id = ?
	AND bill_tb.transaction_date <= ? AND bill_tb.transaction_date >= ?";
    let pay_amount = BillTb::find()
        .from_raw_sql(Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::MySql,
            sql2,
            [
                Value::Int(Some(user_id)),
                Value::ChronoDate(Some(Box::new(end))),
                Value::ChronoDate(Some(Box::new(begin))),
            ],
        ))
        .into_json()
        .one(db)
        .await
        .map_err(|e| JsonErr::from_error(400, anyhow!(e)))?
        .unwrap_or(json!({}))
        .get("pay_amount")
        .unwrap_or(&serde_json::Value::Null)
        .as_str()
        .map(|x| x.to_owned());
    res.render(Text::Json(
        json!({
            "status":"success",
            "code":200,
            "msg":{
                "data":{
                    "list":result,
                    "pay_amount":pay_amount
                }
            }
        })
        .to_string(),
    ));
    Ok(())
}

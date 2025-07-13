use crate::{auth::Authority, error::*, orm};
use anyhow::anyhow;
use chrono::{Local, NaiveDate};
use rust_decimal::prelude::*;
use salvo::prelude::*;
use sea_orm::{
    ActiveModelBehavior, ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait,
    IntoActiveModel, PaginatorTrait, QueryFilter, Statement, Value,
};
use serde_json::json;

use crate::error::IntoJsonError;
use crate::orm::model::{prelude::*, *};
use rust_decimal::Decimal;

#[handler]
pub async fn registry(req: &mut Request, res: &mut Response) -> JsonResult<()> {
    let account = req
        .form::<String>("account")
        .await
        .filter(|s| !s.is_empty())
        .ok_or(JsonErr::from_error(400, anyhow!("未获取到有效账号")))?;
    let pass = req
        .form::<String>("password")
        .await
        .filter(|s| !s.is_empty())
        .ok_or(JsonErr::from_error(400, anyhow!("未获取到有效密码")))?;
    if pass.chars().count() < 6 {
        res_error(400, anyhow!("密码至少6位"))?;
        return Ok(());
    }
    let db = orm::get_dao()?;
    if UserTb::find()
        .filter(user_tb::Column::Account.eq(&account))
        .count(db)
        .await
        .json_err()?
        != 0
    {
        res_error(400, anyhow!("账号已存在"))?;
        return Ok(());
    }
    let salt_pass = format!("{:x}", md5::compute(pass));
    let mut user = user_tb::ActiveModel::new();
    user.pass = Set(salt_pass);
    user.account = Set(account);
    let now = Local::now().naive_local();
    user.created_time = Set(now);
    user.updated_time = Set(now);
    user.insert(db).await.json_err()?;
    res.render(Text::Json(
        json!({
            "status":"success",
            "code":200,
            "msg":"注册成功"
        })
        .to_string(),
    ));
    Ok(())
}
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
        .map_err(|_e| JsonErr::from_error(401, anyhow!("unknown user")))?;
    let start_date = req
        .query::<String>("begin")
        .ok_or(JsonErr::from_error(400, anyhow!("未获取到起始日期")))?;
    let begin = NaiveDate::parse_from_str(&start_date, "%Y-%m-%d")
        .map_err(|e| JsonErr::from_error(400, anyhow!("起始日期解析错误：{}", e)))?;

    let end_date = req
        .query::<String>("end")
        .ok_or(JsonErr::from_error(400, anyhow!("未获取到结束日期")))?;
    let end = NaiveDate::parse_from_str(&end_date, "%Y-%m-%d")
        .map_err(|e| JsonErr::from_error(400, anyhow!("结束日期解析错误：{}", e)))?;

    let delta_time = end.signed_duration_since(begin);

    if delta_time.num_seconds() < 0 {
        res_error(400, anyhow!("无效的日期范围"))?;
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

    let mut pay_amount = rust_decimal::Decimal::new(0, 2);
    for bill in &result {
        _ = bill.as_object().inspect(|v| {
            v.get("pay").inspect(|v| {
                v.as_str().inspect(|v| {
                    if let Ok(v) = Decimal::from_str(v) {
                        pay_amount += v;
                    }
                });
            });
        });
        // if let Some(v) = bill.as_object() {
        //     if let Some(v) = v.get("pay") {
        //         if let Some(v) = v.as_str() {
        //             if let Ok(v) = Decimal::from_str(v) {
        //                 pay_amount += v;
        //             }
        //         }
        //     }
        // }
    }
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

#[handler]
pub async fn bill_add(req: &mut Request, res: &mut Response, depot: &mut Depot) -> JsonResult<()> {
    let user_id = *depot
        .get::<i32>("user_id")
        .map_err(|_e| JsonErr::from_error(401, anyhow!("unknown user")))?;
    let pay = req
        .form::<String>("pay")
        .await
        .ok_or(JsonErr::from_error(400, anyhow!("未获取到支出金额")))?;
    let pay = Decimal::from_str(&pay)
        .map_err(|e| JsonErr::from_error(400, anyhow!("无效的支出金额 {e}")))?;
    let pay_method = req
        .form::<String>("pay_method")
        .await
        .ok_or(JsonErr::from_error(400, anyhow!("未获取到支付方式")))?;
    let comment = req
        .form::<String>("comment")
        .await
        .ok_or(JsonErr::from_error(400, anyhow!("未获取到备注")))?;
    let transaction_date = req
        .form::<String>("transaction_date")
        .await
        .ok_or(JsonErr::from_error(400, anyhow!("未获取到交易日期")))?;

    let transaction_date = NaiveDate::parse_from_str(&transaction_date, "%Y-%m-%d")
        .map_err(|e| JsonErr::from_error(400, anyhow!("交易日期解析错误：{}", e)))?;

    let tag_id = req
        .form::<i32>("tag_id")
        .await
        .ok_or(JsonErr::from_error(400, anyhow!("未获取到交易标签")))?;

    let db = orm::get_dao()?;
    if TagTb::find()
        .filter(tag_tb::Column::Id.eq(tag_id))
        .filter(tag_tb::Column::UserId.eq(user_id))
        .one(db)
        .await
        .json_err()?
        .is_none()
    {
        res_error(400, anyhow!("无效的标签"))?;
        return Ok(());
    }

    let mut info = bill_tb::ActiveModel::new();
    info.comment = Set(Some(comment));
    info.pay = Set(Some(pay));
    info.pay_method = Set(pay_method);
    info.transaction_date = Set(transaction_date);
    info.user_id = Set(user_id);
    info.tag_id = Set(tag_id);
    let now = Local::now().naive_local();
    info.created_time = Set(now);
    info.updated_time = Set(now);
    info.insert(db).await.json_err()?;
    res.render(Text::Json(
        json!({
            "status":"success",
            "code":200,
            "msg":"新增成功"
        })
        .to_string(),
    ));
    Ok(())
}

#[handler]
pub async fn del_bill(req: &mut Request, res: &mut Response, depot: &mut Depot) -> JsonResult<()> {
    let user_id = *depot
        .get::<i32>("user_id")
        .map_err(|_e| JsonErr::from_error(401, anyhow!("unknown user")))?;
    let bill_id = req
        .form::<i32>("id")
        .await
        .ok_or(JsonErr::from_error(400, anyhow!("未找到有效的账单ID")))?;
    let db = orm::get_dao()?;
    if let Some(info) = BillTb::find()
        .filter(bill_tb::Column::Id.eq(bill_id))
        .filter(bill_tb::Column::UserId.eq(user_id))
        .one(db)
        .await
        .json_err()?
    {
        let info = info.into_active_model();
        info.delete(db).await.json_err()?;
        res.render(Text::Json(
            json!({
                "status":"success",
                "code":200,
                "msg":"删除成功"
            })
            .to_string(),
        ));
    } else {
        res_error(400, anyhow!("无效的账单"))?;
    }
    Ok(())
}

#[handler]
pub async fn tag_list(req: &mut Request, res: &mut Response, depot: &mut Depot) -> JsonResult<()> {
    let user_id = *depot
        .get::<i32>("user_id")
        .map_err(|_e| JsonErr::from_error(401, anyhow!("unknown user")))?;
    let db = orm::get_dao()?;
    let list = TagTb::find()
        .filter(tag_tb::Column::UserId.eq(user_id))
        .into_json()
        .all(db)
        .await
        .json_err()?;
    res.render(Text::Json(
        json!({
            "status":"success",
            "code":200,
            "msg":{
                "data":{
                    "list":list
                }
            }
        })
        .to_string(),
    ));
    Ok(())
}

#[handler]
pub async fn add_tag(req: &mut Request, res: &mut Response, depot: &mut Depot) -> JsonResult<()> {
    let user_id = *depot
        .get::<i32>("user_id")
        .map_err(|_e| JsonErr::from_error(401, anyhow!("unknown user")))?;
    let name = req
        .form::<String>("name")
        .await
        .ok_or(JsonErr::from_error(400, anyhow!("未获取到有效标签")))?;
    let db = orm::get_dao()?;
    if TagTb::find()
        .filter(tag_tb::Column::UserId.eq(user_id))
        .filter(tag_tb::Column::Name.eq(&name))
        .count(db)
        .await
        .json_err()?
        != 0
    {
        res_error(400, anyhow!("标签已存在"))?;
        return Ok(());
    }
    let mut info = tag_tb::ActiveModel::new();
    info.name = Set(name);
    info.user_id = Set(user_id);
    let now = Local::now().naive_local();
    info.created_time = Set(now);
    info.updated_time = Set(now);
    info.insert(db).await.json_err()?;
    res.render(Text::Json(
        json!({
            "status":"success",
            "code":200,
            "msg":"新增成功"
        })
        .to_string(),
    ));
    Ok(())
}

#[handler]
pub async fn del_tag(req: &mut Request, res: &mut Response, depot: &mut Depot) -> JsonResult<()> {
    let user_id = *depot
        .get::<i32>("user_id")
        .map_err(|_e| JsonErr::from_error(401, anyhow!("unknown user")))?;
    let tag_id = req
        .form::<i32>("id")
        .await
        .ok_or(JsonErr::from_error(400, anyhow!("未找到有效的账单ID")))?;
    let db = orm::get_dao()?;
    if let Some(info) = TagTb::find()
        .filter(tag_tb::Column::Id.eq(tag_id))
        .filter(tag_tb::Column::UserId.eq(user_id))
        .one(db)
        .await
        .json_err()?
    {
        let info = info.into_active_model();
        info.delete(db).await.json_err()?;
        res.render(Text::Json(
            json!({
                "status":"success",
                "code":200,
                "msg":"删除成功"
            })
            .to_string(),
        ));
    } else {
        res_error(400, anyhow!("无效的标签"))?;
    }
    Ok(())
}

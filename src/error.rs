use salvo::prelude::*;
use serde_json::{Value, json};
pub struct JsonErr(StatusCode, Value);

impl JsonErr {
    pub fn from_value(val: Value) -> Self {
        JsonErr(StatusCode::BAD_REQUEST, val)
    }
    pub fn from_error(code: i32, msg: anyhow::Error) -> Self {
        JsonErr(
            StatusCode::from_u16(code as _).unwrap_or(StatusCode::BAD_REQUEST),
            json!({
                "status":"error",
                "code":code,
                "msg":msg.to_string()
            }),
        )
    }
}

pub type JsonResult<T> = Result<T, JsonErr>;

#[async_trait]
impl Writer for JsonErr {
    async fn write(mut self, _req: &mut Request, _depot: &mut Depot, res: &mut Response) {
        res.status_code(self.0);
        res.render(Text::Json(self.1.to_string()));
    }
}

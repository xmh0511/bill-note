use salvo::prelude::*;
use serde_json::{Value, json};
pub struct JsonErr(Value);

impl JsonErr {
    pub fn from_value(val: Value) -> Self {
        JsonErr(val)
    }
    pub fn from_error(code: i32, msg: anyhow::Error) -> Self {
        JsonErr(json!({
            "status":"error",
            "code":code,
            "msg":msg.to_string()
        }))
    }
}

pub type JsonResult<T> = Result<T, JsonErr>;

#[async_trait]
impl Writer for JsonErr {
    async fn write(mut self, _req: &mut Request, _depot: &mut Depot, res: &mut Response) {
        res.render(Text::Json(self.0.to_string()));
    }
}

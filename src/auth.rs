use crate::error::{JsonErr, JsonResult};
use anyhow::anyhow;
use jsonwebtoken::{self, EncodingKey};
use salvo::prelude::*;
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};

#[derive(Clone)]
pub struct Authority {
    secret_key: String,
}

impl Authority {
    pub fn new(secret_key: String) -> Self {
        Self { secret_key }
    }

    pub fn sign(&self, id: i32, seconds: i64) -> JsonResult<String> {
        let exp = OffsetDateTime::now_utc() + Duration::seconds(seconds);
        let claim = JwtClaims {
            id,
            exp: exp.unix_timestamp(),
        };
        jsonwebtoken::encode(
            &jsonwebtoken::Header::default(),
            &claim,
            &EncodingKey::from_secret(self.secret_key.as_bytes()),
        )
        .map_err(|e| JsonErr::from_error(500, anyhow!(e)))
    }
}

#[handler]
impl Authority {
    async fn handle(&self, depot: &mut Depot) {
        depot.inject(self.clone());
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JwtClaims {
    id: i32,
    exp: i64,
}

#[handler]
pub async fn check_auth_id(
    req: &mut Request,
    res: &mut Response,
    depot: &mut Depot,
    ctrl: &mut FlowCtrl,
) -> JsonResult<()> {
    match depot.jwt_auth_state() {
        JwtAuthState::Authorized => {
            let data = depot.jwt_auth_data::<JwtClaims>().unwrap();
            depot.insert("user_id", data.claims.id);
            ctrl.call_next(req, depot, res).await;
            Ok(())
        }
        _ => {
            ctrl.skip_rest();
            return Err(JsonErr::from_error(401, anyhow!("UnAuthorized")));
        }
    }
}

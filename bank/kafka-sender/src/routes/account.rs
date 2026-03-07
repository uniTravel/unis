use super::*;
use crate::handlers::account::*;
use axum::routing::post;
use unis::{JsonFormat, RkyvFormat, route_builder};

macro_rules! account_routes {
    ($format:ty) => {
        route_builder!(account, $format, [create], [verify, limit, approve])
    };
}

pub fn rkyv_routes() -> Router<Arc<KafkaSender<AccountCommand>>> {
    account_routes!(RkyvFormat)
}

pub fn json_routes() -> Router<Arc<KafkaSender<AccountCommand>>> {
    account_routes!(JsonFormat)
}

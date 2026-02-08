use super::*;
use crate::handlers::account::*;
use axum::routing::post;
use unis_kafka::sender::{JsonFormat, RkyvFormat};

pub fn rkyv_routes() -> Router<Arc<Sender<AccountCommand>>> {
    Router::new()
        .route(
            "/account/create/:agg_id/:com_id",
            post(create::<RkyvFormat>),
        )
        .route(
            "/account/verify/:agg_id/:com_id",
            post(verify::<RkyvFormat>),
        )
}

pub fn json_routes() -> Router<Arc<Sender<AccountCommand>>> {
    Router::new()
        .route(
            "/account/create/:agg_id/:com_id",
            post(create::<JsonFormat>),
        )
        .route(
            "/account/verify/:agg_id/:com_id",
            post(verify::<JsonFormat>),
        )
}

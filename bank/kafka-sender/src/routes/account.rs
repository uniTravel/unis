use super::*;
use crate::handlers::account::*;
use axum::routing::post;
use unis_kafka::sender::{JsonFormat, RkyvFormat};

macro_rules! account_impl {
    ($format:ty, [$($op:ident), *]) => {{
        let mut router = Router::new();
        $(
            router = router.route(
                concat!("/account/", stringify!($op), "/{agg_id}/{com_id}"),
                post($op::<$format>),
            );
        )*
        router
    }};
}

macro_rules! account_routes {
    ($format:ty) => {
        account_impl!($format, [create, verify, limit, approve])
    };
}

pub fn rkyv_routes() -> Router<Arc<Sender<AccountCommand>>> {
    account_routes!(RkyvFormat)
}

pub fn json_routes() -> Router<Arc<Sender<AccountCommand>>> {
    account_routes!(JsonFormat)
}

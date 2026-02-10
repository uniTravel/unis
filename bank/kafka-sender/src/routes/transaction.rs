use super::*;
use crate::handlers::transaction::*;
use axum::routing::post;
use unis_kafka::sender::{JsonFormat, RkyvFormat};

macro_rules! transaction_impl {
    ($format:ty, [$($op:ident), *]) => {{
        let mut router = Router::new();
        $(
            router = router.route(
                concat!("/transaction/", stringify!($op), "/{agg_id}/{com_id}"),
                post($op::<$format>),
            );
        )*
        router
    }};
}

macro_rules! transaction_routes {
    ($format:ty) => {
        transaction_impl!(
            $format,
            [
                init,
                open,
                set_limit,
                change_limit,
                set_trans_limit,
                deposit,
                withdraw,
                transfer_out,
                transfer_in
            ]
        )
    };
}

pub fn rkyv_routes() -> Router<Arc<Sender<TransactionCommand>>> {
    transaction_routes!(RkyvFormat)
}

pub fn json_routes() -> Router<Arc<Sender<TransactionCommand>>> {
    transaction_routes!(JsonFormat)
}

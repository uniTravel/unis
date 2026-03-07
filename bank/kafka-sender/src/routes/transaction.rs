use super::*;
use crate::handlers::transaction::*;
use axum::routing::post;
use unis::{JsonFormat, RkyvFormat, route_builder};

macro_rules! transaction_routes {
    ($format:ty) => {
        route_builder!(
            transaction,
            $format,
            [init, open],
            [
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

pub fn rkyv_routes() -> Router<Arc<KafkaSender<TransactionCommand>>> {
    transaction_routes!(RkyvFormat)
}

pub fn json_routes() -> Router<Arc<KafkaSender<TransactionCommand>>> {
    transaction_routes!(JsonFormat)
}

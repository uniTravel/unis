use super::*;

pub fn rkyv_routes() -> Router<Arc<Sender<TransactionCommand>>> {
    Router::new()
}

pub fn json_routes() -> Router<Arc<Sender<TransactionCommand>>> {
    Router::new()
}

mod account_init;

use super::*;
use domain::account::*;

#[fixture]
async fn app(#[future(awt)] _setup: (), #[future(awt)] ctx: &'static App) -> Router {
    let svc = Arc::new(ctx.setup::<AccountCommand>().await);
    Router::new().merge(routes::account_routes().with_state(svc))
}

fn route(op: &str, agg_id: Uuid, com_id: Uuid) -> String {
    format!("/api/v1/rkyv/account/{op}/{agg_id}/{com_id}")
}

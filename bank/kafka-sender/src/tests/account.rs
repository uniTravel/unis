mod account_init;

use super::*;
use domain::account::*;

#[fixture]
async fn app(_setup: (), #[future(awt)] ctx: &'static App) -> Router {
    let svc = Arc::new(ctx.setup::<AccountCommand>().await);
    Router::new().merge(routes::account_routes().with_state(svc))
}

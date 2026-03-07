mod account_init;

use super::*;
use domain::account::*;
use unis_kafka::sender::KafkaSender;

#[fixture]
async fn app(_setup: (), ctx: &'static Context) -> Router {
    let svc = Arc::new(ctx.setup::<_, KafkaSender<AccountCommand>>().await);
    Router::new().merge(routes::account_routes().with_state(svc))
}

use axum::extract::{Path, State};
use domain::account::{self, AccountCommand};
use std::sync::Arc;
use unis_kafka::sender::{Request, Sender, UniCommand, UniKey, UniResponse};

macro_rules! account_handlers {
    ($func_name:ident, $com:ty, $variant:ident) => {
        pub async fn $func_name<F>(
            Path(UniKey { agg_id, com_id }): Path<UniKey>,
            State(svc): State<Arc<Sender<AccountCommand>>>,
            UniCommand(com, _): UniCommand<$com, F>,
        ) -> UniResponse {
            svc.send(agg_id, com_id, AccountCommand::$variant(com))
                .await
        }
    };
}

account_handlers!(create, account::CreateAccount, Create);
account_handlers!(verify, account::VerifyAccount, Verify);
account_handlers!(limit, account::LimitAccount, Limit);
account_handlers!(approve, account::ApproveAccount, Approve);

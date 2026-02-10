use axum::extract::{Path, State};
use domain::transaction::{self, TransactionCommand};
use std::sync::Arc;
use unis_kafka::sender::{Request, Sender, UniCommand, UniKey, UniResponse};

macro_rules! transaction_handlers {
    ($func_name:ident, $com:ty, $variant:ident) => {
        pub async fn $func_name<F>(
            Path(UniKey { agg_id, com_id }): Path<UniKey>,
            State(svc): State<Arc<Sender<TransactionCommand>>>,
            UniCommand(com, _): UniCommand<$com, F>,
        ) -> UniResponse {
            svc.send(agg_id, com_id, TransactionCommand::$variant(com))
                .await
        }
    };
}

transaction_handlers!(init, transaction::InitPeriod, InitPeriod);
transaction_handlers!(open, transaction::OpenPeriod, OpenPeriod);
transaction_handlers!(set_limit, transaction::SetLimit, SetLimit);
transaction_handlers!(change_limit, transaction::ChangeLimit, ChangeLimit);
transaction_handlers!(set_trans_limit, transaction::SetTransLimit, SetTransLimit);
transaction_handlers!(deposit, transaction::Deposit, Deposit);
transaction_handlers!(withdraw, transaction::Withdraw, Withdraw);
transaction_handlers!(transfer_out, transaction::TransferOut, TransferOut);
transaction_handlers!(transfer_in, transaction::TransferIn, TransferIn);

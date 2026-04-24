use axum::extract::State;
use domain::transaction::*;
use std::sync::Arc;
use unis::{UniCommand, UniKey, sender::Sender};
use unis_kafka::com_handler;

com_handler!(init, TransactionCommand, InitPeriod, InitPeriod);
com_handler!(open, TransactionCommand, OpenPeriod, OpenPeriod);

com_handler!(set_limit, TransactionCommand, SetLimit, SetLimit);
com_handler!(change_limit, TransactionCommand, ChangeLimit, ChangeLimit);
com_handler!(
    set_trans_limit,
    TransactionCommand,
    SetTransLimit,
    SetTransLimit
);
com_handler!(deposit, TransactionCommand, Deposit, Deposit);
com_handler!(withdraw, TransactionCommand, Withdraw, Withdraw);
com_handler!(transfer_out, TransactionCommand, TransferOut, TransferOut);
com_handler!(transfer_in, TransactionCommand, TransferIn, TransferIn);

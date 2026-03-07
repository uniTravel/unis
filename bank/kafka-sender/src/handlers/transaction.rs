use axum::extract::{Path, State};
use domain::transaction::*;
use std::sync::Arc;
use unis::{UniCommand, UniKey, UniResponse, sender::Sender};
use unis_kafka::{change_handler, create_handler};
use uuid::Uuid;

create_handler!(init, TransactionCommand, InitPeriod, InitPeriod);
create_handler!(open, TransactionCommand, OpenPeriod, OpenPeriod);

change_handler!(set_limit, TransactionCommand, SetLimit, SetLimit);
change_handler!(change_limit, TransactionCommand, ChangeLimit, ChangeLimit);
change_handler!(
    set_trans_limit,
    TransactionCommand,
    SetTransLimit,
    SetTransLimit
);
change_handler!(deposit, TransactionCommand, Deposit, Deposit);
change_handler!(withdraw, TransactionCommand, Withdraw, Withdraw);
change_handler!(transfer_out, TransactionCommand, TransferOut, TransferOut);
change_handler!(transfer_in, TransactionCommand, TransferIn, TransferIn);

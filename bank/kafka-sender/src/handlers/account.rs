use axum::extract::{Path, State};
use domain::account::*;
use std::sync::Arc;
use unis_kafka::{
    change_handler, create_handler,
    sender::{Request, Sender, UniCommand, UniKey, UniResponse},
};
use uuid::Uuid;

create_handler!(create, AccountCommand, CreateAccount, Create);

change_handler!(verify, AccountCommand, VerifyAccount, Verify);
change_handler!(limit, AccountCommand, LimitAccount, Limit);
change_handler!(approve, AccountCommand, ApproveAccount, Approve);

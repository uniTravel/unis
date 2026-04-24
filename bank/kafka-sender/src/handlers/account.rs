use axum::extract::State;
use domain::account::*;
use std::sync::Arc;
use unis::{UniCommand, UniKey, sender::Sender};
use unis_kafka::com_handler;

com_handler!(create, AccountCommand, CreateAccount, Create);

com_handler!(verify, AccountCommand, VerifyAccount, Verify);
com_handler!(limit, AccountCommand, LimitAccount, Limit);
com_handler!(approve, AccountCommand, ApproveAccount, Approve);

use axum::extract::{Path, State};
use domain::account::*;
use std::sync::Arc;
use unis::{UniCommand, UniKey, UniResponse, sender::Sender};
use unis_kafka::{change_handler, create_handler};
use uuid::Uuid;

create_handler!(create, AccountCommand, CreateAccount, Create);

change_handler!(verify, AccountCommand, VerifyAccount, Verify);
change_handler!(limit, AccountCommand, LimitAccount, Limit);
change_handler!(approve, AccountCommand, ApproveAccount, Approve);

pub(crate) mod account;
pub(crate) mod transaction;

use axum::{Extension, extract::State, http::StatusCode};
use std::sync::Arc;
use unis::{AxumCommand, UniKey, sender::Sender};
use unis_kafka::sender::KafkaSender;
use utoipa::OpenApi;

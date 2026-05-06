use std::sync::Arc;

pub mod app;
pub mod db;
mod dto;
mod error;
mod handlers;

use db::Database;

pub type AppState = Arc<Database>;

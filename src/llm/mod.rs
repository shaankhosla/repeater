pub mod client;
pub mod cloze;
pub mod drill_preprocessor;
pub mod prompt_user;
pub mod provider;
pub mod rephrase;
pub mod response;
pub mod secrets;

pub use client::{ensure_client, get_auth_and_store, test_configured_api_key};
pub use cloze::request_cloze;
pub use rephrase::request_question_rephrase;
pub use secrets::clear_api_key;

//! GitHub domain: the REST client, the payload mapping and the token store.
//! The Pull requests add-on and the Settings > GitHub page are the consumers;
//! nothing here touches gpui.
pub mod client;
pub mod model;
pub mod token;

pub use client::Client;
pub use model::{CheckState, Checks, FileStat, PullRequest, RepoSlug};

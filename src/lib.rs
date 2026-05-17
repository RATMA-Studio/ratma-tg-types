//! Auto-generated Rust types and async client for the Telegram Bot API +
//! WebApp.
//!
//! Façade re-exporting the two member crates:
//! - [`ratma_tg_types_core`] — pure types (always available).
//! - [`ratma_tg_types_http`] — async HTTP client and method bindings (default;
//!   disable the `http` feature for a types-only build).
//!
//! # Examples
//! ## Use webhooks to fetch updates
//! ```no_run
//! use ratma_tg_types::gen_types::FileData;
//! use ratma_tg_types::bot::Bot;
//! use ratma_tg_types::ext::{Webhook, BotUrl};
//! use std::net::{SocketAddr, Ipv4Addr, IpAddr};
//! use futures_util::StreamExt;
//! # tokio_test::block_on(async {
//! let client = Bot::new("sometoken").unwrap();
//! Webhook::new(
//!    &client,
//!    BotUrl::Host("example.com".to_owned()),
//!    false,
//!    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
//!    None,
//! )
//! .get_updates()
//! .await.unwrap()
//! .for_each_concurrent(
//!     None,
//!     |update| async move {
//!         //handle update
//!     },
//! );
//! })
//! ```

#![recursion_limit = "256"]

#[cfg(feature = "http")]
pub use ratma_tg_types_core::multipart;
pub use ratma_tg_types_core::types as gen_types;
#[cfg(feature = "http")]
pub use ratma_tg_types_http::{bot, ext, gen_methods};

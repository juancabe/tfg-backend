//! Server implementation
//!
//! # General notes
//!
//! ## Architecture
//!
//! The server is divided in modules [`interface`], [`service`], [`dao`], [`model`] (and [`helpers`]) with their
//! corresponding responsibilities. A layer based approach is intended, where [`interface`]
//! subsystems call [`service`]'s ones, [`service`] calls [`dao`] and [`dao`] uses [`model`]
//!
//! ### Implications
//!
//! - Each module contains its type interface definitions.
//! - If Module A needs to call Module B, A will use B defined types on the interface parameters and B defined types will also be returned.
//! - Exceptions may happen to occur, for example [`helpers`] Module types and (of course) [`std`] types can be returned whenever it's necessary, no need to wrap them.
//! - In order to avoid _boilerplate burden_, [`service`] layer is used as a mapper, [`interface`] defined types *can* be directly accepted and returned on the [`service`] layer.
//!
//!
pub mod dao;
pub mod helpers;
pub mod interface;
pub mod model;
pub mod service;

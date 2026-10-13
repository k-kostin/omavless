// SPDX-License-Identifier: MIT
// Tiny standalone test driver: no GPUI link, host module or native window.
#[path = "../theme_client.rs"]
#[allow(dead_code)] // Tests inject environment; normal entry reads it in clients.
mod theme_client;
#[path = "../theme_environment.rs"]
mod theme_environment;
#[path = "../theme_fixture.rs"]
mod theme_fixture;
#[path = "../theme_follow.rs"]
mod theme_follow;
#[path = "../theme_source.rs"]
mod theme_source;

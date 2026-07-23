// GPUI view composition naturally uses callback-heavy signatures and intentionally keeps some
// render helpers close to their tests. Keep Clippy strict for correctness while excluding these
// mechanical style lints from the workspace-wide `-D warnings` gate.
#![allow(
    clippy::default_constructed_unit_structs,
    clippy::derivable_impls,
    clippy::if_same_then_else,
    clippy::items_after_test_module,
    clippy::large_enum_variant,
    clippy::let_and_return,
    clippy::needless_borrow,
    clippy::ptr_arg,
    clippy::question_mark,
    clippy::redundant_closure,
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::uninlined_format_args,
    clippy::unnecessary_lazy_evaluations
)]

mod actions;
mod app;
mod assets;
mod completion;
mod components;
mod entities;
mod http;
mod icons;
mod importers;
mod theme;
mod utils;
mod views;

use app::SetuApp;

fn main() {
    SetuApp::run();
}

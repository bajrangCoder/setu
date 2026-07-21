use std::sync::Arc;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum SidebarLoadState {
    #[default]
    Loading,
    Ready,
    Error(Arc<str>),
}

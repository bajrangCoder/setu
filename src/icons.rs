//! Custom icon definitions
use gpui::{AnyElement, App, IntoElement, RenderOnce, SharedString, Window};
use gpui_component::{Icon, IconNamed};

/// All icons available in the application.
///
/// Each variant maps to an SVG file in `assets/icons/`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, IntoElement)]
pub enum IconName {
    ChevronDown,
    ChevronUp,
    CircleX,
    Close,
    FilePen,
    GripVertical,
    Plus,
    Trash,
    Check,
    Loader,
    CircleDot,
    Command,
    CopyPlus,
    Link,
    PanelLeft,
    Search,
    Send,
    Ban,
    FilePlus,
    FileUp,
    File,
    Sparkles,
    CircleCheck,
    Info,
    TriangleAlert,
    Copy,
    FileDown,
    TextWrap,
    Funnel,
    CaseSensitive,
    ChevronLeft,
    ChevronRight,
    Replace,
    History,
    Folder,
    FolderOpen,
    FolderPlus,
    Star,
    StarFilled,
    Play,
    Pause,
    Square,
    Volume2,
    VolumeX,
}

impl IconNamed for IconName {
    fn path(self) -> SharedString {
        match self {
            Self::ChevronDown => "icons/chevron-down.svg",
            Self::ChevronUp => "icons/chevron-up.svg",
            Self::CircleX => "icons/circle-x.svg",
            Self::Close => "icons/close.svg",
            Self::FilePen => "icons/file-pen.svg",
            Self::GripVertical => "icons/grip-vertical.svg",
            Self::Plus => "icons/plus.svg",
            Self::Trash => "icons/trash.svg",
            Self::Check => "icons/check.svg",
            Self::Loader => "icons/loader.svg",
            Self::CircleDot => "icons/circle-dot.svg",
            Self::Command => "icons/command.svg",
            Self::CopyPlus => "icons/copy-plus.svg",
            Self::Link => "icons/link.svg",
            Self::PanelLeft => "icons/panel-left.svg",
            Self::Search => "icons/search.svg",
            Self::Send => "icons/send.svg",
            Self::Ban => "icons/ban.svg",
            Self::FilePlus => "icons/file-plus.svg",
            Self::FileUp => "icons/file-up.svg",
            Self::File => "icons/file.svg",
            Self::Sparkles => "icons/sparkles.svg",
            Self::CircleCheck => "icons/circle-check.svg",
            Self::Info => "icons/info.svg",
            Self::TriangleAlert => "icons/triangle-alert.svg",
            Self::Copy => "icons/copy.svg",
            Self::FileDown => "icons/file-down.svg",
            Self::TextWrap => "icons/text-wrap.svg",
            Self::Funnel => "icons/funnel.svg",
            Self::CaseSensitive => "icons/case-sensitive.svg",
            Self::ChevronLeft => "icons/chevron-left.svg",
            Self::ChevronRight => "icons/chevron-right.svg",
            Self::Replace => "icons/replace.svg",
            Self::History => "icons/history.svg",
            Self::Folder => "icons/folder.svg",
            Self::FolderOpen => "icons/folder-open.svg",
            Self::FolderPlus => "icons/folder-plus.svg",
            Self::Star => "icons/star.svg",
            Self::StarFilled => "icons/star-filled.svg",
            Self::Play => "icons/play.svg",
            Self::Pause => "icons/pause.svg",
            Self::Square => "icons/square.svg",
            Self::Volume2 => "icons/volume-2.svg",
            Self::VolumeX => "icons/volume-x.svg",
        }
        .into()
    }
}

impl RenderOnce for IconName {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        Icon::new(self)
    }
}

impl From<IconName> for AnyElement {
    fn from(icon: IconName) -> Self {
        Icon::new(icon).into_any_element()
    }
}

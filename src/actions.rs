// GPUI actions for keybindings and commands

use gpui::actions;

// Define all application actions
actions!(
    setu,
    [
        // Request actions
        SendRequest,
        NewRequest,
        CancelRequest,
        DuplicateRequest,
        // Tab navigation
        NextTab,
        PreviousTab,
        CloseTab,
        CloseAllTabs,
        CloseOtherTabs,
        ReopenClosedTab,
        GoToTab1,
        GoToTab2,
        GoToTab3,
        GoToTab4,
        GoToTab5,
        GoToTab6,
        GoToTab7,
        GoToTab8,
        GoToLastTab,
        // Focus/Navigation
        FocusUrlBar,
        FocusBody,
        FocusHeaders,
        FocusParams,
        FocusAuth,
        FocusResponse,
        CycleRequestTabs,
        CycleResponseTabs,
        // Request panel tabs
        SwitchToBodyTab,
        SwitchToParamsTab,
        SwitchToHeadersTab,
        SwitchToAuthTab,
        // Response panel tabs
        SwitchToResponseBody,
        SwitchToResponseHeaders,
        // UI actions
        ToggleCommandPalette,
        ToggleSidebar,
        ZoomIn,
        ZoomOut,
        ResetZoom,
        ToggleFullscreen,
        // History actions
        ClearHistory,
        // Editing
        Copy,
        Paste,
        Cut,
        SelectAll,
        Undo,
        Redo,
        FormatDocument,
        // Application actions
        Quit,
        ShowSettings,
        ShowHelp,
        SaveRequest,
        OpenRequest,
        // Method shortcuts (quick method switching)
        SetMethodGet,
        SetMethodPost,
        SetMethodPut,
        SetMethodDelete,
        SetMethodPatch,
        SetMethodHead,
        SetMethodOptions,
    ]
);

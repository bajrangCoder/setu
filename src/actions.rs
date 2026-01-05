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
        // Navigation
        FocusUrlBar,
        FocusBody,
        FocusHeaders,
        // UI actions
        ToggleCommandPalette,
        ToggleSidebar,
        // History actions
        ClearHistory,
        // Application actions
        Quit,
        // Method shortcuts
        SetMethodGet,
        SetMethodPost,
        SetMethodPut,
        SetMethodDelete,
        SetMethodPatch,
    ]
);

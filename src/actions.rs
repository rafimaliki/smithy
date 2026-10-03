//! Every keyboard-triggered action. Bindings live in `keymap.rs`.
use gpui::actions;

actions!(
    smithy,
    [
        // editor
        MoveLeft,
        MoveRight,
        MoveUp,
        MoveDown,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        Home,
        End,
        SelectHome,
        SelectEnd,
        Backspace,
        Delete,
        Enter,
        Tab,
        Undo,
        Redo,
        Copy,
        Cut,
        Paste,
        SelectAll,
        Save,
        ToggleBlame,
        // workspace
        ToggleSidebar,
        OpenFolder,
        CloseTab,
        NextTab,
        PrevTab,
        ReopenTab,
        Quit,
        // Handled by the Search add-on; does nothing while it is off.
        SearchEverywhere,
        // Handled by the add-on that owns the tab's document view.
        TogglePreview,
    ]
);

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
        Find,
        ToggleBlame,
        // Move to the next or previous syntax error (Error highlighting add-on).
        NextError,
        PrevError,
        ToggleWrap,
        // Answered by the Language servers add-on; they do nothing while it is off.
        GoToDefinition,
        FindReferences,
        NavBack,
        NavForward,
        // workspace
        ToggleSidebar,
        OpenFolder,
        CloseTab,
        NextTab,
        PrevTab,
        ReopenTab,
        // Handled by the Split panes add-on; does nothing while it is off.
        SplitRight,
        Quit,
        // Handled by the Search add-on; does nothing while it is off.
        SearchEverywhere,
        // Handled by the Terminal add-on; does nothing while it is off.
        ToggleTerminal,
        // Handled by the add-on that owns the tab's document view.
        TogglePreview,
    ]
);

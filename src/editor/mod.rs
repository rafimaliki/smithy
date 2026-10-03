//! Core text editor: buffer, caret/selection model, highlighting, and the gpui view.
pub mod buffer;
pub mod grammars;
pub mod highlight;
pub mod lang;
pub mod layout;
pub mod load;
pub mod state;
#[allow(dead_code)] // ponytail: consumed by the Search add-on (a-search), not landed yet
pub mod symbols;
pub mod view;

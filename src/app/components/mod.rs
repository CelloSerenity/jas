// Shared UI components.

/// Prompt for destrictive actions, calls the browser's alert and
/// returns what the user chooses.
pub fn confirm(message: &str) -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|w| w.confirm_with_message(message).ok())
            .unwrap_or(false)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = message;
        false
    }
}

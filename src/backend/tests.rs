#[cfg(test)]
mod tests {
    use super::*;

    include!("tests/core.rs");
    include!("tests/models.rs");
    include!("tests/session_listing.rs");
    include!("tests/session_history.rs");
    include!("tests/message_windows.rs");
    include!("tests/chat.rs");
    include!("tests/insights.rs");
    include!("tests/subagents.rs");
    include!("tests/terminal.rs");
    include!("tests/images.rs");
}

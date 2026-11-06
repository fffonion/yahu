#[cfg(test)]
mod tests {
    use super::*;

    include!("tests/core.rs");
    include!("tests/skills.rs");
    include!("tests/sensenova_auth.rs");
    include!("tests/models.rs");
    include!("tests/session_listing.rs");
    include!("tests/session_history.rs");
    include!("tests/message_windows.rs");
    include!("tests/chat.rs");
    include!("tests/insights.rs");
    include!("tests/provider_usage.rs");
    include!("tests/provider_usage_opendesign.rs");
    include!("tests/provider_usage_opendesign_auth.rs");
    include!("tests/provider_usage_workbuddy.rs");
    include!("tests/provider_daily_quota.rs");
    include!("tests/subagents.rs");
    include!("tests/terminal.rs");
    include!("tests/images.rs");
}

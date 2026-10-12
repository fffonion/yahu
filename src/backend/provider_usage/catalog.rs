fn provider_usage_catalog(hermes_home: &Path) -> Vec<ProviderUsageProvider> {
    let commandcode_pool = !auth_json_credential_pool(hermes_home, "commandcode").is_empty();
    let codex_pool = !auth_json_credential_pool(hermes_home, "openai-codex").is_empty();
    let grok_pool = !auth_json_credential_pool(hermes_home, "xai-oauth").is_empty();
    let zed_pro_pool = !auth_json_credential_pool(hermes_home, "zed-pro").is_empty();
    let zed_session_cookie = !provider_env_value(hermes_home, "ZED_SESSION_COOKIE").is_empty();
    let zed_configured = zed_pro_pool || zed_session_cookie;
    let agentrouter_user_id = first_provider_env_value(
        hermes_home,
        &["AGENTROUTER_USER_ID", "AGENTROUTER_NEWAPI_USER_ID"],
    );
    let agentrouter_cookie = first_provider_env_value(
        hermes_home,
        &["AGENTROUTER_SESSION_COOKIE", "AGENTROUTER_COOKIE"],
    );
    let agentrouter_token = first_provider_env_value(
        hermes_home,
        &["AGENTROUTER_ACCESS_TOKEN", "AGENTROUTER_TOKEN"],
    );
    let agentrouter_configured = !agentrouter_user_id.is_empty()
        || !agentrouter_cookie.is_empty()
        || !agentrouter_token.is_empty();
    let agentrouter_query_ready = !agentrouter_user_id.is_empty()
        && (!agentrouter_cookie.is_empty() || !agentrouter_token.is_empty());
    let hcnsec_user_id = provider_env_value(hermes_home, "HCNSEC_USER_ID");
    let hcnsec_cookie = provider_env_value(hermes_home, "HCNSEC_SESSION_COOKIE");
    let hcnsec_token = provider_env_value(hermes_home, "HCNSEC_ACCESS_TOKEN");
    let hcnsec_configured = !hcnsec_user_id.is_empty() || !hcnsec_cookie.is_empty() || !hcnsec_token.is_empty();
    let hcnsec_query_ready = !hcnsec_user_id.is_empty() && (!hcnsec_cookie.is_empty() || !hcnsec_token.is_empty());
    let justwoker_user_id = provider_env_value(hermes_home, "JUSTWOKER_USER_ID");
    let justwoker_cookie = provider_env_value(hermes_home, "JUSTWOKER_SESSION_COOKIE");
    let justwoker_token = provider_env_value(hermes_home, "JUSTWOKER_ACCESS_TOKEN");
    let justwoker_configured = !justwoker_user_id.is_empty()
        || !justwoker_cookie.is_empty()
        || !justwoker_token.is_empty()
        || any_provider_env_value(hermes_home, &["HERMES_CUSTOM_API_JUSTWOKER_ICU_API_KEY"])
        || any_custom_provider_api_key(hermes_home, &["justwoker", "justwoker-icu"]);
    let minimax_cookie = provider_env_value(hermes_home, "MINIMAX_COOKIE");
    let minimax_query_ready = !extract_cookie_value(&minimax_cookie, "minimax_group_id_v2").is_empty();
    let vyceai_cookie = provider_env_value(hermes_home, "VYCEAI_COOKIE");
    let vyceai_query_ready = !extract_cookie_value(&vyceai_cookie, "session").is_empty();
    let justwoker_query_ready = !justwoker_user_id.is_empty()
        && (!justwoker_cookie.is_empty() || !justwoker_token.is_empty());
    let entries = [
        (
            "agentrouter",
            "AgentRouter 用量",
            agentrouter_configured,
            agentrouter_query_ready,
            "AGENTROUTER_SESSION_COOKIE + AGENTROUTER_USER_ID",
            "也支持 AGENTROUTER_ACCESS_TOKEN + AGENTROUTER_USER_ID；凭据写入 ~/.hermes/.env。",
        ),
        (
            "justwoker",
            "JustWoker 用量",
            justwoker_configured,
            justwoker_query_ready,
            "JUSTWOKER_ACCESS_TOKEN + JUSTWOKER_USER_ID",
            "配置账户 access token 与用户 ID；也支持 JUSTWOKER_SESSION_COOKIE + JUSTWOKER_USER_ID。推理 API key 无法查询账户用量；凭据写入 ~/.hermes/.env。",
        ),
        (
            "hcnsec",
            "hcnsec 用量",
            hcnsec_configured,
            hcnsec_query_ready,
            "HCNSEC_ACCESS_TOKEN + HCNSEC_USER_ID",
            "配置账户 access token 与用户 ID；也支持 HCNSEC_SESSION_COOKIE + HCNSEC_USER_ID。",
        ),
        (
            "vyceai",
            "Vyce AI 用量",
            !vyceai_cookie.is_empty()
                || any_provider_env_value(hermes_home, &["VYCEAI_API_KEY"])
                || any_custom_provider_api_key(hermes_home, &["vyceai"]),
            vyceai_query_ready,
            "VYCEAI_COOKIE",
            "配置 VYCEAI_COOKIE，需包含网页登录后的 session；推理 API key 无法查询账户用量。",
        ),
        (
            "openrouter",
            "OpenRouter 用量",
            any_provider_env_value(hermes_home, &["OPENROUTER_API_KEY", "OPENROUTER_MANAGEMENT_KEY"])
                || any_custom_provider_api_key(hermes_home, &["openrouter"]),
            !provider_env_value(hermes_home, "OPENROUTER_MANAGEMENT_KEY").is_empty(),
            "OPENROUTER_MANAGEMENT_KEY",
            "配置 OPENROUTER_MANAGEMENT_KEY；从 OpenRouter 控制台 Settings → Management Keys 创建。",
        ),
        (
            "deepseek",
            "DeepSeek 用量",
            any_provider_env_value(hermes_home, &["DEEPSEEK_API_KEY", "DEEPSEEK_COOKIE", "DEEPSEEK_TOKEN"])
                || any_custom_provider_api_key(hermes_home, &["deepseek"]),
            any_provider_env_value(hermes_home, &["DEEPSEEK_COOKIE"])
                && any_provider_env_value(hermes_home, &["DEEPSEEK_TOKEN"]),
            "DEEPSEEK_COOKIE + DEEPSEEK_TOKEN",
            "配置 DEEPSEEK_COOKIE 与 DEEPSEEK_TOKEN；登录 platform.deepseek.com 后，从浏览器开发者工具的已登录会话复制对应 Cookie 和认证请求字段。",
        ),
        (
            "atlascloud",
            "AtlasCloud 用量",
            any_provider_env_value(hermes_home, &["ATLASCLOUD_API_KEY"])
                || any_custom_provider_api_key(hermes_home, &["atlascloud"]),
            !provider_env_value(hermes_home, "ATLASCLOUD_API_KEY").is_empty()
                || !config_provider_api_key(hermes_home, "atlascloud").is_empty(),
            "ATLASCLOUD_API_KEY 或 config.yaml custom_providers.api_key",
            "配置 ATLASCLOUD_API_KEY，或在 config.yaml 的 custom_providers 中填写 atlascloud 的 api_key；从 AtlasCloud 控制台 API Keys 获取。",
        ),
        (
            "mimo",
            "MiMo 用量",
            any_provider_env_value(hermes_home, &["MIMO_API_KEY", "MIMO_COOKIE", "MIMO_PASSTOKEN"])
                || any_custom_provider_api_key(hermes_home, &["mimo"]),
            !provider_env_value(hermes_home, "MIMO_COOKIE").is_empty()
                || !provider_env_value(hermes_home, "MIMO_PASSTOKEN").is_empty(),
            "MiMo 认证信息",
            "配置 MiMo 平台会话，或提供 Xiaomi 账号认证种子；系统会自动换取 MiMo 会话后查询用量。",
        ),
        (
            "minimax",
            "MiniMax 用量",
            !minimax_cookie.is_empty()
                || any_provider_env_value(hermes_home, &["MINIMAX_API_KEY"])
                || any_custom_provider_api_key(hermes_home, &["minimax"]),
            minimax_query_ready,
            "MINIMAX_COOKIE",
            "配置完整的 MINIMAX_COOKIE；登录 minimaxi.com 后从浏览器开发者工具复制 Cookie，需包含 minimax_group_id_v2。",
        ),
        (
            "stepfun",
            "StepFun Step Plan 用量",
            any_provider_env_value(hermes_home, &["STEPFUN_API_KEY", "STEPFUN_COOKIE", "STEPFUN_WEB_COOKIE"]),
            any_provider_env_value(hermes_home, &["STEPFUN_COOKIE", "STEPFUN_WEB_COOKIE"]),
            "STEPFUN_COOKIE 或 STEPFUN_WEB_COOKIE",
            "Step Plan 用量接口使用平台网页 Cookie；API key 仅用于模型调用，不能查询账户用量。",
        ),
        (
            "kimi",
            "Kimi Code 用量",
            any_provider_env_value(hermes_home, &["KIMI_API_KEY", "KIMI_AUTH_TOKEN"])
                || any_custom_provider_api_key(hermes_home, &["kimi"]),
            !provider_env_value(hermes_home, "KIMI_AUTH_TOKEN").is_empty(),
            "KIMI_AUTH_TOKEN",
            "配置 KIMI_AUTH_TOKEN；登录 kimi.com 后从浏览器开发者工具复制已登录会话的认证 token。",
        ),
        (
            "sensenova",
            "SenseNova Token Plan 用量",
            sensenova_password_configured(hermes_home)
                || load_sensenova_service_auth(hermes_home).is_some()
                || any_provider_env_value(hermes_home, &["SENSENOVA_API_KEY"])
                || any_custom_provider_api_key(hermes_home, &["sensenova"]),
            sensenova_password_configured(hermes_home)
                || load_sensenova_service_auth(hermes_home).is_some_and(|auth| !auth.access.is_empty() || !auth.refresh.is_empty()),
            "SENSENOVA_USERNAME + SENSENOVA_PASSWORD",
            "配置 SenseNova 用户名和密码；临时 token 由 Yahu 私有会话缓存自动管理，推理 API key 无法查询 Token Plan 积分。",
        ),
        (
            "opencode",
            "OpenCode Go 用量",
            !provider_env_value(hermes_home, "OPENCODE_GO_API_KEY").is_empty(),
            !provider_env_value(hermes_home, "OPENCODE_GO_API_KEY").is_empty(),
            "OPENCODE_GO_API_KEY",
            "配置 OPENCODE_GO_API_KEY；该 key 需关联有效的 OpenCode Go 订阅。",
        ),
        (
            "commandcode",
            "CommandCode 用量",
            commandcode_pool
                || any_provider_env_value(hermes_home, &["COMMANDCODE_API_KEY"])
                || any_custom_provider_api_key(hermes_home, &["commandcode"]),
            commandcode_pool || !provider_env_value(hermes_home, "COMMANDCODE_API_KEY").is_empty(),
            "COMMANDCODE_API_KEY 或 auth.json credential_pool.commandcode",
            "配置 COMMANDCODE_API_KEY，或在 auth.json 的 credential_pool.commandcode 中加入账号凭据；API key 从 CommandCode 控制台创建。",
        ),
        (
            "codex",
            "Codex 用量",
            codex_pool
                || any_provider_env_value(hermes_home, &["OPENAI_API_KEY"])
                || any_custom_provider_api_key(hermes_home, &["openai-codex", "codex"]),
            codex_pool,
            "auth.json credential_pool.openai-codex",
            "配置 auth.json 的 credential_pool.openai-codex；通过 Hermes 的 Codex OAuth 登录流程获取账号凭据。",
        ),
        (
            "grok",
            "Grok 用量",
            grok_pool
                || any_provider_env_value(hermes_home, &["XAI_API_KEY"])
                || any_custom_provider_api_key(hermes_home, &["xai", "grok"]),
            grok_pool,
            "auth.json credential_pool.xai-oauth",
            "配置 auth.json 的 credential_pool.xai-oauth，保留 access_token 和 refresh_token；通过 Hermes 的 xAI OAuth 登录流程获取。",
        ),
        (
            "zed-pro",
            "Zed Pro 用量",
            zed_configured,
            zed_configured,
            "ZED_SESSION_COOKIE 或 auth.json credential_pool.zed-pro",
            "用量查询使用浏览器 Cookie 中的 zed.session；auth.json 的 zed-pro 凭据提供账期重置时间。",
        ),
    ];
    entries
        .into_iter()
        .map(|(provider, title, configured, query_ready, credential_hint, setup_hint)| {
            ProviderUsageProvider {
                provider: provider.into(),
                title: title.into(),
                configured,
                query_ready,
                credential_hint: credential_hint.into(),
                setup_hint: setup_hint.into(),
            }
        })
        .collect()
}

// Provider usage panel: query external AI provider consoles for quota/usage
// data. Credentials are read from ~/.hermes/.env (and ~/.hermes/auth.json
// credential pools). Ported from ~/workspace/api-usage/token_usage.py.


include!("provider_usage/types.rs");
include!("provider_usage/credentials.rs");
include!("provider_usage/formatting.rs");
include!("provider_usage/http.rs");
include!("provider_usage/catalog.rs");
include!("provider_usage/newapi.rs");
include!("provider_usage/openrouter.rs");
include!("provider_usage/deepseek.rs");
include!("provider_usage/atlascloud.rs");
include!("provider_usage/minimax.rs");
include!("provider_usage/stepfun.rs");
include!("provider_usage/kimi.rs");
include!("provider_usage/mimo.rs");
include!("provider_usage/opencode.rs");
include!("provider_usage/commandcode.rs");
include!("provider_usage/codex.rs");
include!("provider_usage/grok.rs");
include!("provider_usage/zed.rs");
include!("provider_usage/vyceai.rs");
include!("provider_usage/cache.rs");
include!("provider_usage/icons.rs");
include!("provider_usage/api.rs");
include!("provider_usage/tests.rs");

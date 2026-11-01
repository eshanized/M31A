//! `m31a-execpolicy`: Execution policies, egress firewall, and security decision engine.
//!
//! Enforces granular security boundaries on command executions, outbound network requests,
//! SSRF vulnerabilities, and credential access across all agent activities.

pub mod ast_decomposer;
pub mod attribution;
pub mod body_budget;
pub mod connect_policy;
pub mod credential_broker;
pub mod dns_rebinding;
pub mod domain_matcher;
pub mod path_authorization;
pub mod policy;
pub mod presets;
pub mod proxy;
pub mod tcp_tunnel;

pub use ast_decomposer::{ShellAstDecomposer, ShellDecomposition};
pub use attribution::{
    ATTRIBUTION_FRAME_MAGIC, AttributedSession, AttributionRegistry, ConnectionAttribution,
    MAX_ATTRIBUTION_TOKEN_LEN, PROXY_ATTRIBUTION_TOKEN_ENV_KEY, SocketAttributor,
    read_attribution_token, write_attribution_frame,
};
pub use body_budget::{
    BUFFER_SLOT_COUNT, BodyBudgetError, BodyBudgetManager, DEFAULT_RESERVE_TIMEOUT,
    MAX_REQUEST_BODY_BYTES, MAX_RESPONSE_BODY_BYTES, collect_bounded_body, content_length_matches,
    is_amazon_event_stream, supported_transfer_framing,
};
pub use connect_policy::{
    ConnectDecision, ConnectPolicy, is_non_public_ip, is_non_public_target, is_private_network_ip,
};
pub use credential_broker::{BrokeredCredential, CredentialBroker};
pub use dns_rebinding::{AntiRebindingDnsResolver, DnsRebindingRisk, validate_ip_safety};
pub use domain_matcher::{DomainPattern, NetworkSecurityFilter};
pub use path_authorization::{PathAuthDecision, PathAuthorizationRule, PathAuthorizer};
pub use policy::{
    BANNED_PREFIX_SUGGESTIONS, Decision, ExecPolicy, NetworkRule, PatternToken, PrefixRule,
    RequirementsExecPolicy, RuleMatch, blocking_append_allow_prefix_rule,
    get_banned_prefix_suggestion, host_executable,
};
pub use presets::{ApprovalPreset, ApprovalPresetKind};
pub use proxy::{NetworkProxyServer, ProxyConfig};
pub use tcp_tunnel::{TcpTunnelInfo, TcpTunnelManager};

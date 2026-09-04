//! Permission tiers for the MCP server (plan §7 "MCP 权限分层").
//!
//! - `ReadOnly` — only parse/display, never execute. `list_requests` only.
//! - `SafeWrite` — execute GET / HEAD / OPTIONS (idempotent, side-effect-free).
//! - `HighRiskWrite` — execute any method (POST/PUT/DELETE/PATCH/...).

use http_core::model::Method;

/// Permission tier selected by the host configuration (plan §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PermissionTier {
    /// Parse + list only. No request execution.
    ReadOnly,
    /// Execute idempotent, side-effect-free methods: GET / HEAD / OPTIONS.
    #[default]
    SafeWrite,
    /// Execute any HTTP method.
    HighRiskWrite,
}

impl std::fmt::Display for PermissionTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl PermissionTier {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "read_only",
            Self::SafeWrite => "safe_write",
            Self::HighRiskWrite => "high_risk_write",
        }
    }

    /// Parse a tier from a configuration string. Case-insensitive; accepts
    /// `read_only`/`safe_write`/`high_risk_write` and a few common aliases.
    pub fn from_str_lossy(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "read_only" | "readonly" | "read-only" => Some(Self::ReadOnly),
            "safe_write" | "safe-write" | "safewrite" => Some(Self::SafeWrite),
            "high_risk_write" | "high-risk-write" | "highriskwrite" | "full" => {
                Some(Self::HighRiskWrite)
            }
            _ => None,
        }
    }

    /// Whether a request with the given method may be executed under this tier.
    pub fn allows(self, method: &Method) -> bool {
        match self {
            Self::ReadOnly => false,
            Self::SafeWrite => matches!(method, Method::Get | Method::Head | Method::Options),
            Self::HighRiskWrite => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readonly_blocks_all_methods() {
        let tier = PermissionTier::ReadOnly;
        assert!(!tier.allows(&Method::Get));
        assert!(!tier.allows(&Method::Post));
        assert!(!tier.allows(&Method::Delete));
    }

    #[test]
    fn safe_write_allows_get_head_options() {
        let tier = PermissionTier::SafeWrite;
        assert!(tier.allows(&Method::Get));
        assert!(tier.allows(&Method::Head));
        assert!(tier.allows(&Method::Options));
        assert!(!tier.allows(&Method::Post));
        assert!(!tier.allows(&Method::Put));
        assert!(!tier.allows(&Method::Delete));
        assert!(!tier.allows(&Method::Patch));
        assert!(!tier.allows(&Method::Connect));
        assert!(!tier.allows(&Method::Trace));
    }

    #[test]
    fn high_risk_allows_everything() {
        let tier = PermissionTier::HighRiskWrite;
        for m in [
            Method::Get,
            Method::Head,
            Method::Post,
            Method::Put,
            Method::Delete,
            Method::Connect,
            Method::Patch,
            Method::Options,
            Method::Trace,
        ] {
            assert!(
                tier.allows(&m),
                "{m:?} should be allowed under HighRiskWrite"
            );
        }
    }

    #[test]
    fn from_str_lossy_accepts_aliases() {
        assert_eq!(
            PermissionTier::from_str_lossy("read_only"),
            Some(PermissionTier::ReadOnly)
        );
        assert_eq!(
            PermissionTier::from_str_lossy("READONLY"),
            Some(PermissionTier::ReadOnly)
        );
        assert_eq!(
            PermissionTier::from_str_lossy("safe-write"),
            Some(PermissionTier::SafeWrite)
        );
        assert_eq!(
            PermissionTier::from_str_lossy("high_risk_write"),
            Some(PermissionTier::HighRiskWrite)
        );
        assert_eq!(
            PermissionTier::from_str_lossy("full"),
            Some(PermissionTier::HighRiskWrite)
        );
        assert_eq!(PermissionTier::from_str_lossy("unknown"), None);
    }

    #[test]
    fn default_is_safe_write() {
        assert_eq!(PermissionTier::default(), PermissionTier::SafeWrite);
    }
}

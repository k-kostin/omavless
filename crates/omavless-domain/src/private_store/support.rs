// SPDX-License-Identifier: MIT
//! Allowlisted support facts. Never serialize the underlying private document.
use super::*;

impl PrivateStore {
    /// Public counts and configured preferences, not a private settings editor.
    /// Startup enablement here is stored intent, never inferred unit enablement.
    #[must_use]
    pub fn support_projection(&self) -> Value {
        serde_json::json!({
            "inventory": {
                "profiles": self.profiles.len(),
                "favorites": self.profiles.iter().filter(|p| p.favorite).count(),
                "subscriptions": self.subscriptions.len(),
                "customRules": self.custom_rules.len(),
            },
            "routing": {
                "preset": self.routing_preset,
                "configured": !self.routing_preset.is_empty(),
                "lastManualRuleUpdate": self.rules_updated_at,
            },
            "startup": {
                "configured": self.startup_configured,
                "enabled": self.startup.enabled,
                "target": self.startup.target,
                "mode": self.startup.mode,
            },
            "updates": {"latestSubscription": self.subscriptions.iter()
                .map(|s| s.updated_at).max().unwrap_or(0)},
            "onboardingComplete": self.onboarding_complete,
        })
    }
}

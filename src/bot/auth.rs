use std::collections::HashSet;

use crate::config::schema::{BotConfig, BotPlatform};

#[derive(Debug, Clone)]
pub struct BotAuth {
    allow_all: bool,
    allowed_users: HashSet<String>,
}

impl BotAuth {
    pub fn for_platform(config: &BotConfig, platform: BotPlatform) -> Self {
        let allowed_users: HashSet<String> = config
            .authorized_users
            .iter()
            .filter(|user| user.platform == platform)
            .map(|user| user.user_id.clone())
            .collect();

        let allow_all = config.allow_all_users || allowed_users.is_empty();

        Self {
            allow_all,
            allowed_users,
        }
    }

    pub fn is_authorized(&self, user_id: &str) -> bool {
        if self.allow_all {
            return true;
        }
        self.allowed_users.contains(user_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schema::{AuthorizedUser, BotConfig, BotPlatform};

    #[test]
    fn authorized_user_passes_check() {
        let config = BotConfig {
            enabled: true,
            allow_all_users: false,
            discord_application_id: None,
            discord_public_key: None,
            slack_signing_secret: None,
            authorized_users: vec![AuthorizedUser {
                platform: BotPlatform::Discord,
                user_id: "123".to_string(),
            }],
        };

        let auth = BotAuth::for_platform(&config, BotPlatform::Discord);
        assert!(auth.is_authorized("123"));
    }

    #[test]
    fn unauthorized_user_fails_check() {
        let config = BotConfig {
            enabled: true,
            allow_all_users: false,
            discord_application_id: None,
            discord_public_key: None,
            slack_signing_secret: None,
            authorized_users: vec![AuthorizedUser {
                platform: BotPlatform::Slack,
                user_id: "U123".to_string(),
            }],
        };

        let auth = BotAuth::for_platform(&config, BotPlatform::Slack);
        assert!(!auth.is_authorized("U999"));
    }

    #[test]
    fn empty_authorized_list_allows_all_when_configured() {
        let config = BotConfig {
            enabled: true,
            allow_all_users: true,
            discord_application_id: None,
            discord_public_key: None,
            slack_signing_secret: None,
            authorized_users: Vec::new(),
        };

        let auth = BotAuth::for_platform(&config, BotPlatform::Discord);
        assert!(auth.is_authorized("any"));
    }
}

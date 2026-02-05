use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BotCommand {
    Status,
    Pause,
    Resume,
    Logs { tail: usize },
    NewSession,
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BotCommandParseError {
    message: String,
}

impl BotCommandParseError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for BotCommandParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for BotCommandParseError {}

impl FromStr for BotCommand {
    type Err = BotCommandParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let tokens: Vec<&str> = input.split_whitespace().collect();
        if tokens.is_empty() {
            return Ok(BotCommand::Help);
        }

        let mut index = 0;
        let first = tokens[0];
        if first.eq_ignore_ascii_case("/palin") || first.eq_ignore_ascii_case("palin") {
            index += 1;
        }

        let command = tokens.get(index).copied().unwrap_or("help");
        match command {
            "status" => Ok(BotCommand::Status),
            "pause" => Ok(BotCommand::Pause),
            "resume" => Ok(BotCommand::Resume),
            "logs" => parse_logs_command(&tokens[(index + 1)..]),
            "new-session" | "newsession" => Ok(BotCommand::NewSession),
            "help" => Ok(BotCommand::Help),
            _ => Err(BotCommandParseError::new(format!(
                "Unknown command: {command}"
            ))),
        }
    }
}

fn parse_logs_command(args: &[&str]) -> Result<BotCommand, BotCommandParseError> {
    let mut tail = 10usize;
    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx] {
            "--tail" | "-t" => {
                let value = args
                    .get(idx + 1)
                    .ok_or_else(|| BotCommandParseError::new("Missing value for --tail"))?;
                tail = value
                    .parse::<usize>()
                    .map_err(|_| BotCommandParseError::new("Invalid value for --tail"))?;
                idx += 2;
            }
            _ => {
                idx += 1;
            }
        }
    }

    Ok(BotCommand::Logs { tail })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BotCommandResult {
    pub success: bool,
    pub title: String,
    pub body: Option<String>,
    pub fields: Vec<BotCommandField>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BotCommandField {
    pub name: String,
    pub value: String,
    pub inline: bool,
}

impl BotCommandResult {
    pub fn success(title: impl Into<String>) -> Self {
        Self {
            success: true,
            title: title.into(),
            body: None,
            fields: Vec::new(),
        }
    }

    pub fn error(title: impl Into<String>) -> Self {
        Self {
            success: false,
            title: title.into(),
            body: None,
            fields: Vec::new(),
        }
    }

    pub fn with_body(mut self, body: impl Into<String>) -> Self {
        self.body = Some(body.into());
        self
    }

    pub fn with_fields(mut self, fields: Vec<BotCommandField>) -> Self {
        self.fields = fields;
        self
    }

    pub fn to_discord_response(&self) -> serde_json::Value {
        let title = truncate(&self.title, 256);
        let description = self.body.as_ref().map(|body| truncate(body, 1800));
        let fields = if self.fields.is_empty() {
            None
        } else {
            Some(
                self.fields
                    .iter()
                    .map(|field| {
                        serde_json::json!({
                            "name": truncate(&field.name, 256),
                            "value": truncate(&field.value, 1024),
                            "inline": field.inline,
                        })
                    })
                    .collect::<Vec<_>>(),
            )
        };

        let embed = serde_json::json!({
            "title": title,
            "description": description,
            "fields": fields,
        });

        serde_json::json!({
            "type": 4,
            "data": {
                "embeds": [embed]
            }
        })
    }

    pub fn to_slack_response(&self) -> serde_json::Value {
        let title = truncate(&self.title, 150);
        let mut blocks = vec![serde_json::json!({
            "type": "header",
            "text": {
                "type": "plain_text",
                "text": title,
            }
        })];

        if let Some(body) = self.body.as_ref() {
            blocks.push(serde_json::json!({
                "type": "section",
                "fields": [{
                    "type": "mrkdwn",
                    "text": truncate(body, 2900)
                }]
            }));
        } else if !self.fields.is_empty() {
            let fields: Vec<_> = self
                .fields
                .iter()
                .map(|field| {
                    serde_json::json!({
                        "type": "mrkdwn",
                        "text": format!("*{}*\n{}", field.name, field.value)
                    })
                })
                .collect();
            blocks.push(serde_json::json!({
                "type": "section",
                "fields": fields
            }));
        }

        serde_json::json!({
            "response_type": "ephemeral",
            "blocks": blocks
        })
    }
}

fn truncate(value: &str, limit: usize) -> String {
    if value.len() <= limit {
        value.to_string()
    } else if limit <= 3 {
        value[..limit].to_string()
    } else {
        let mut trimmed = value[..(limit - 3)].to_string();
        trimmed.push_str("...");
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_status_command() {
        let cmd = BotCommand::from_str("/palin status").unwrap();
        assert_eq!(cmd, BotCommand::Status);
    }

    #[test]
    fn parses_logs_tail_command() {
        let cmd = BotCommand::from_str("/palin logs --tail 5").unwrap();
        assert_eq!(cmd, BotCommand::Logs { tail: 5 });
    }

    #[test]
    fn parses_simple_status_command() {
        let cmd = BotCommand::from_str("status").unwrap();
        assert_eq!(cmd, BotCommand::Status);
    }

    #[test]
    fn rejects_unknown_command() {
        let err = BotCommand::from_str("/palin nope").unwrap_err();
        assert!(err.to_string().contains("Unknown command"));
    }
}

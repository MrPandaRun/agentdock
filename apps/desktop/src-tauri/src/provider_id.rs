use provider_contract::ProviderId;

pub fn parse_provider_id(raw: &str) -> Result<ProviderId, String> {
    match raw {
        "claude_code" => Ok(ProviderId::ClaudeCode),
        "codex" => Ok(ProviderId::Codex),
        "opencode" => Ok(ProviderId::OpenCode),
        _ => Err(format!("Unsupported provider: {raw}")),
    }
}

#[cfg(test)]
mod tests {
    use provider_contract::ProviderId;

    use super::parse_provider_id;

    #[test]
    fn parse_provider_id_reads_supported_providers() {
        assert_eq!(parse_provider_id("claude_code"), Ok(ProviderId::ClaudeCode));
        assert_eq!(parse_provider_id("codex"), Ok(ProviderId::Codex));
        assert_eq!(parse_provider_id("opencode"), Ok(ProviderId::OpenCode));
    }

    #[test]
    fn parse_provider_id_rejects_unknown_provider() {
        let error = parse_provider_id("unknown").expect_err("provider should be rejected");
        assert_eq!(error, "Unsupported provider: unknown");
    }
}

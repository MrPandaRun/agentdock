use provider_contract::{
    ProviderAdapter, ProviderError, ProviderHealthCheckRequest, ProviderHealthCheckResult,
    ProviderId, ProviderResult, ResumeThreadRequest, ResumeThreadResult, SwitchContextSummary,
    ThreadSummary,
};

pub struct ClaudeAdapter;

impl ClaudeAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ClaudeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderAdapter for ClaudeAdapter {
    fn provider_id(&self) -> ProviderId {
        ProviderId::ClaudeCode
    }

    fn health_check(
        &self,
        _request: ProviderHealthCheckRequest,
    ) -> ProviderResult<ProviderHealthCheckResult> {
        Err(ProviderError::not_implemented(
            "claude_code health_check is not implemented yet",
        ))
    }

    fn list_threads(&self, _project_path: Option<&str>) -> ProviderResult<Vec<ThreadSummary>> {
        Err(ProviderError::not_implemented(
            "claude_code list_threads is not implemented yet",
        ))
    }

    fn resume_thread(&self, _request: ResumeThreadRequest) -> ProviderResult<ResumeThreadResult> {
        Err(ProviderError::not_implemented(
            "claude_code resume_thread is not implemented yet",
        ))
    }

    fn summarize_switch_context(&self, _thread_id: &str) -> ProviderResult<SwitchContextSummary> {
        Err(ProviderError::not_implemented(
            "claude_code summarize_switch_context is not implemented yet",
        ))
    }
}

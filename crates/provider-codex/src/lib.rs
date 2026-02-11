use provider_contract::{
    ProviderAdapter, ProviderError, ProviderHealthCheckRequest, ProviderHealthCheckResult,
    ProviderId, ProviderResult, ResumeThreadRequest, ResumeThreadResult, SwitchContextSummary,
    ThreadSummary,
};

pub struct CodexAdapter;

impl CodexAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CodexAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderAdapter for CodexAdapter {
    fn provider_id(&self) -> ProviderId {
        ProviderId::Codex
    }

    fn health_check(
        &self,
        _request: ProviderHealthCheckRequest,
    ) -> ProviderResult<ProviderHealthCheckResult> {
        Err(ProviderError::not_implemented(
            "codex health_check is not implemented yet",
        ))
    }

    fn list_threads(&self, _project_path: Option<&str>) -> ProviderResult<Vec<ThreadSummary>> {
        Err(ProviderError::not_implemented(
            "codex list_threads is not implemented yet",
        ))
    }

    fn resume_thread(&self, _request: ResumeThreadRequest) -> ProviderResult<ResumeThreadResult> {
        Err(ProviderError::not_implemented(
            "codex resume_thread is not implemented yet",
        ))
    }

    fn summarize_switch_context(&self, _thread_id: &str) -> ProviderResult<SwitchContextSummary> {
        Err(ProviderError::not_implemented(
            "codex summarize_switch_context is not implemented yet",
        ))
    }
}

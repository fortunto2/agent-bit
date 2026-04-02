//! BitGN HarnessService client — typed Connect-RPC via bitgn-sdk.

use anyhow::Result;
use bitgn_sdk::harness::{self as proto, HarnessServiceClient};
use connectrpc::client::HttpClient;

pub struct HarnessClient {
    inner: HarnessServiceClient<HttpClient>,
}

// ─── Response wrappers (compatible API for main.rs) ─────────────────────────

pub struct BenchmarkResponse {
    pub benchmark_id: String,
    pub description: String,
    pub tasks: Vec<TaskInfo>,
}

pub struct TaskInfo {
    pub task_id: String,
    pub preview: String,
    pub hint: String,
}

pub struct PlaygroundResponse {
    pub trial_id: String,
    pub instruction: String,
    pub harness_url: String,
}

pub struct StartRunResponse {
    pub run_id: String,
    pub trial_ids: Vec<String>,
}

pub struct StartTrialResponse {
    pub trial_id: String,
    pub task_id: String,
    pub instruction: String,
    pub harness_url: String,
}

pub struct EndTrialResponse {
    pub trial_id: String,
    pub score: Option<f32>,
    pub score_detail: Vec<String>,
}

pub struct GetRunResponse {
    pub run_id: String,
    pub state: String,
    pub score: Option<f32>,
}

pub struct GetTrialResponse {
    pub trial_id: String,
    pub instruction: String,
    pub task_id: String,
    pub score: Option<f32>,
    pub score_detail: Vec<String>,
    pub state: String,
    pub logs: Vec<TrialLog>,
}

pub struct TrialLog {
    pub time: String,
    pub text: String,
    pub kind: String,
}

// ─── Client ──────────────────────────────────────────────────────────────────

impl HarnessClient {
    pub fn new(base_url: &str, api_key: Option<String>) -> Self {
        let http = bitgn_sdk::make_http_client(base_url);
        let config = bitgn_sdk::make_client_config(base_url, api_key.as_deref());
        Self { inner: HarnessServiceClient::new(http, config) }
    }

    fn err(method: &str, e: connectrpc::ConnectError) -> anyhow::Error {
        anyhow::anyhow!("{} failed: {}", method, e)
    }

    pub async fn status(&self) -> Result<String> {
        let r = self.inner.status(proto::StatusRequest::default())
            .await.map_err(|e| Self::err("Status", e))?;
        let v = r.view();
        Ok(format!("{} ({})", v.status, v.version))
    }

    pub async fn get_benchmark(&self, benchmark_id: &str) -> Result<BenchmarkResponse> {
        let r = self.inner.get_benchmark(proto::GetBenchmarkRequest {
            benchmark_id: benchmark_id.into(), ..Default::default()
        }).await.map_err(|e| Self::err("GetBenchmark", e))?;
        let v = r.view();
        Ok(BenchmarkResponse {
            benchmark_id: v.benchmark_id.to_string(),
            description: v.description.to_string(),
            tasks: v.tasks.iter().map(|t| TaskInfo {
                task_id: t.task_id.to_string(),
                preview: t.preview.to_string(),
                hint: t.hint.to_string(),
            }).collect(),
        })
    }

    pub async fn start_playground(&self, benchmark_id: &str, task_id: &str) -> Result<PlaygroundResponse> {
        let r = self.inner.start_playground(proto::StartPlaygroundRequest {
            benchmark_id: benchmark_id.into(), task_id: task_id.into(), ..Default::default()
        }).await.map_err(|e| Self::err("StartPlayground", e))?;
        let v = r.view();
        Ok(PlaygroundResponse {
            trial_id: v.trial_id.to_string(),
            instruction: v.instruction.to_string(),
            harness_url: v.harness_url.to_string(),
        })
    }

    pub async fn end_trial(&self, trial_id: &str) -> Result<EndTrialResponse> {
        let r = self.inner.end_trial(proto::EndTrialRequest {
            trial_id: trial_id.into(), ..Default::default()
        }).await.map_err(|e| Self::err("EndTrial", e))?;
        let v = r.view();
        Ok(EndTrialResponse {
            trial_id: v.trial_id.to_string(),
            score: v.score,
            score_detail: v.score_detail.iter().map(|s| s.to_string()).collect(),
        })
    }

    pub async fn start_run(&self, benchmark_id: &str, name: &str) -> Result<StartRunResponse> {
        let r = self.inner.start_run(proto::StartRunRequest {
            benchmark_id: benchmark_id.into(), name: name.into(), ..Default::default()
        }).await.map_err(|e| Self::err("StartRun", e))?;
        let v = r.view();
        Ok(StartRunResponse {
            run_id: v.run_id.to_string(),
            trial_ids: v.trial_ids.iter().map(|s| s.to_string()).collect(),
        })
    }

    pub async fn start_trial(&self, trial_id: &str) -> Result<StartTrialResponse> {
        let r = self.inner.start_trial(proto::StartTrialRequest {
            trial_id: trial_id.into(), ..Default::default()
        }).await.map_err(|e| Self::err("StartTrial", e))?;
        let v = r.view();
        Ok(StartTrialResponse {
            trial_id: v.trial_id.to_string(),
            task_id: v.task_id.to_string(),
            instruction: v.instruction.to_string(),
            harness_url: v.harness_url.to_string(),
        })
    }

    pub async fn get_run(&self, run_id: &str) -> Result<GetRunResponse> {
        let r = self.inner.get_run(proto::GetRunRequest {
            run_id: run_id.into(), ..Default::default()
        }).await.map_err(|e| Self::err("GetRun", e))?;
        let v = r.view();
        Ok(GetRunResponse {
            run_id: v.run_id.to_string(),
            state: format!("{:?}", v.state),
            score: v.score,
        })
    }

    pub async fn submit_run(&self, run_id: &str) -> Result<()> {
        self.inner.submit_run(proto::SubmitRunRequest {
            run_id: run_id.into(), ..Default::default()
        }).await.map_err(|e| Self::err("SubmitRun", e))?;
        Ok(())
    }

    pub async fn get_trial(&self, trial_id: &str) -> Result<GetTrialResponse> {
        let r = self.inner.get_trial(proto::GetTrialRequest {
            trial_id: trial_id.into(), ..Default::default()
        }).await.map_err(|e| Self::err("GetTrial", e))?;
        let v = r.view();
        Ok(GetTrialResponse {
            trial_id: v.trial_id.to_string(),
            instruction: v.instruction.to_string(),
            task_id: v.task_id.to_string(),
            score: v.score,
            score_detail: v.score_detail.iter().map(|s| s.to_string()).collect(),
            state: format!("{:?}", v.state),
            logs: v.logs.iter().map(|l| TrialLog {
                time: l.time.to_string(),
                text: l.text.to_string(),
                kind: format!("{:?}", l.kind),
            }).collect(),
        })
    }
}

//! BitGN HarnessService client — Connect-RPC over HTTP/JSON.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::json;

pub struct HarnessClient {
    client: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
}

// ─── Response types ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusResponse {
    pub status: String,
    pub version: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkResponse {
    pub benchmark_id: String,
    pub description: String,
    pub tasks: Vec<TaskInfo>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskInfo {
    pub task_id: String,
    pub preview: String,
    #[serde(default)]
    pub hint: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaygroundResponse {
    pub trial_id: String,
    pub instruction: String,
    pub harness_url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartRunResponse {
    pub run_id: String,
    pub benchmark_id: String,
    #[serde(default)]
    pub trial_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartTrialResponse {
    pub trial_id: String,
    pub benchmark_id: String,
    pub task_id: String,
    pub run_id: String,
    pub instruction: String,
    pub harness_url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EndTrialResponse {
    pub trial_id: String,
    pub score: Option<f32>,
    #[serde(default)]
    pub score_detail: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetRunResponse {
    pub run_id: String,
    pub benchmark_id: String,
    pub name: String,
    pub score: Option<f32>,
    pub state: String,
    pub kind: String,
    #[serde(default)]
    pub trials: Vec<TrialHead>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrialHead {
    pub trial_id: String,
    pub task_id: String,
    pub state: String,
    pub score: Option<f32>,
    #[serde(default)]
    pub error: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitRunResponse {
    pub run_id: String,
    pub state: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetTrialResponse {
    pub trial_id: String,
    pub instruction: String,
    pub task_id: String,
    pub score: Option<f32>,
    #[serde(default)]
    pub score_detail: Vec<String>,
    pub state: String,
    #[serde(default)]
    pub logs: Vec<TrialLog>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrialLog {
    pub time: String,
    pub text: String,
    pub kind: String,
}

// ─── Client ──────────────────────────────────────────────────────────────────

impl HarnessClient {
    pub fn new(base_url: &str, api_key: Option<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
        }
    }

    async fn call<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        body: &serde_json::Value,
    ) -> Result<T> {
        let url = format!(
            "{}/bitgn.harness.HarnessService/{}",
            self.base_url, method
        );
        let mut req = self
            .client
            .post(&url)
            .header("Content-Type", "application/json");

        if let Some(ref key) = self.api_key {
            req = req.header("Authorization", format!("Bearer {}", key));
        }

        let resp = req
            .json(body)
            .send()
            .await
            .context(format!("request to {}", method))?;

        let status = resp.status();
        let text = resp.text().await?;

        if !status.is_success() {
            bail!("{} failed ({}): {}", method, status, text);
        }

        serde_json::from_str(&text).context(format!("parse {} response", method))
    }

    // ─── Anonymous RPCs ──────────────────────────────────────────────────

    pub async fn status(&self) -> Result<String> {
        let resp: StatusResponse = self.call("Status", &json!({})).await?;
        Ok(format!("{} ({})", resp.status, resp.version))
    }

    pub async fn get_benchmark(&self, benchmark_id: &str) -> Result<BenchmarkResponse> {
        self.call("GetBenchmark", &json!({"benchmarkId": benchmark_id}))
            .await
    }

    pub async fn start_playground(
        &self,
        benchmark_id: &str,
        task_id: &str,
    ) -> Result<PlaygroundResponse> {
        self.call(
            "StartPlayground",
            &json!({"benchmarkId": benchmark_id, "taskId": task_id}),
        )
        .await
    }

    pub async fn end_trial(&self, trial_id: &str) -> Result<EndTrialResponse> {
        self.call("EndTrial", &json!({"trialId": trial_id})).await
    }

    // ─── Authenticated RPCs (leaderboard) ────────────────────────────────

    pub async fn start_run(
        &self,
        benchmark_id: &str,
        name: &str,
    ) -> Result<StartRunResponse> {
        self.call(
            "StartRun",
            &json!({"benchmarkId": benchmark_id, "name": name}),
        )
        .await
    }

    pub async fn start_trial(&self, trial_id: &str) -> Result<StartTrialResponse> {
        self.call("StartTrial", &json!({"trialId": trial_id}))
            .await
    }

    pub async fn get_run(&self, run_id: &str) -> Result<GetRunResponse> {
        self.call("GetRun", &json!({"runId": run_id})).await
    }

    pub async fn submit_run(&self, run_id: &str) -> Result<SubmitRunResponse> {
        self.call("SubmitRun", &json!({"runId": run_id})).await
    }

    /// Get trial details including activity logs (for debugging).
    pub async fn get_trial(&self, trial_id: &str) -> Result<GetTrialResponse> {
        self.call("GetTrial", &json!({"trialId": trial_id})).await
    }
}

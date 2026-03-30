//! BitGN HarnessService client — Connect-RPC over HTTP/JSON.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::json;

pub struct HarnessClient {
    client: reqwest::Client,
    base_url: String,
}

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
pub struct EndTrialResponse {
    pub trial_id: String,
    pub score: Option<f32>,
    #[serde(default)]
    pub score_detail: Vec<String>,
}

impl HarnessClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
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
        let resp = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
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

    pub async fn status(&self) -> Result<String> {
        let resp: StatusResponse = self.call("Status", &json!({})).await?;
        Ok(format!("{} ({})", resp.status, resp.version))
    }

    pub async fn get_benchmark(&self, benchmark_id: &str) -> Result<BenchmarkResponse> {
        self.call(
            "GetBenchmark",
            &json!({"benchmarkId": benchmark_id}),
        )
        .await
    }

    pub async fn start_playground(
        &self,
        benchmark_id: &str,
        task_id: &str,
    ) -> Result<PlaygroundResponse> {
        self.call(
            "StartPlayground",
            &json!({
                "benchmarkId": benchmark_id,
                "taskId": task_id,
            }),
        )
        .await
    }

    pub async fn end_trial(&self, trial_id: &str) -> Result<EndTrialResponse> {
        self.call("EndTrial", &json!({"trialId": trial_id})).await
    }
}

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use ndarray::{Array1, ArrayView1};
use ort::session::Session;
use ort::value::Tensor;
use tokenizers::Tokenizer;

/// Directory where ONNX model + tokenizer + class embeddings are stored.
const MODELS_DIR: &str = "models";

/// Pre-computed class labels and their description text (embedded at load time).
pub const CLASS_DESCRIPTIONS: &[(&str, &str)] = &[
    ("injection", "injection attack with script tags or override instructions"),
    ("crm", "legitimate CRM work about contacts emails or invoices"),
    ("non_work", "non-work request like math trivia or jokes"),
    ("social_engineering", "social engineering with fake identity or cross-company request"),
    ("credential", "OTP or credential sharing attempt"),
];

/// Semantic inbox classifier using ONNX embeddings + cosine similarity.
pub struct InboxClassifier {
    session: Session,
    tokenizer: Tokenizer,
    class_embeddings: Vec<(String, Array1<f32>)>,
}

impl InboxClassifier {
    /// Load model, tokenizer, and class embeddings from `models/` directory.
    pub fn load(models_dir: &Path) -> Result<Self> {
        let model_path = models_dir.join("model.onnx");
        let tokenizer_path = models_dir.join("tokenizer.json");
        let class_embeddings_path = models_dir.join("class_embeddings.json");

        let session = Session::builder()
            .context("failed to create ONNX session builder")?
            .commit_from_file(&model_path)
            .with_context(|| format!("failed to load ONNX model from {}", model_path.display()))?;

        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow::anyhow!("failed to load tokenizer: {}", e))?;

        let class_data = std::fs::read_to_string(&class_embeddings_path)
            .with_context(|| format!("failed to read class embeddings from {}", class_embeddings_path.display()))?;
        let raw: Vec<(String, Vec<f32>)> = serde_json::from_str(&class_data)
            .context("failed to parse class embeddings JSON")?;

        let class_embeddings = raw
            .into_iter()
            .map(|(label, vec)| (label, Array1::from_vec(vec)))
            .collect();

        Ok(Self { session, tokenizer, class_embeddings })
    }

    /// Encode text into a normalized embedding vector using the ONNX model.
    fn encode(&mut self, text: &str) -> Result<Array1<f32>> {
        let encoding = self.tokenizer
            .encode(text, true)
            .map_err(|e| anyhow::anyhow!("tokenization failed: {}", e))?;

        let ids: Vec<i64> = encoding.get_ids().iter().map(|&id| id as i64).collect();
        let mask: Vec<i64> = encoding.get_attention_mask().iter().map(|&m| m as i64).collect();
        let type_ids: Vec<i64> = encoding.get_type_ids().iter().map(|&t| t as i64).collect();
        let len = ids.len();

        let input_ids = Tensor::from_array(([1i64, len as i64], ids.into_boxed_slice()))?;
        let attention_mask = Tensor::from_array(([1i64, len as i64], mask.into_boxed_slice()))?;
        let token_type_ids = Tensor::from_array(([1i64, len as i64], type_ids.into_boxed_slice()))?;

        let outputs = self.session.run(
            ort::inputs![
                "input_ids" => input_ids,
                "attention_mask" => attention_mask,
                "token_type_ids" => token_type_ids,
            ]
        )?;

        // Output shape: [1, seq_len, 384] — mean pool over seq_len
        let (shape, data) = outputs[0].try_extract_tensor::<f32>()?;
        // dims = [1, seq_len, hidden_dim]
        let hidden_dim = *shape.last().context("empty output shape")?;
        let seq_len = if shape.len() >= 2 { shape[shape.len() - 2] } else { 1 } as usize;
        let hidden_dim = hidden_dim as usize;

        // Mean pooling across sequence dimension
        let mut embedding = vec![0.0f32; hidden_dim];
        for s in 0..seq_len {
            for d in 0..hidden_dim {
                embedding[d] += data[s * hidden_dim + d];
            }
        }
        for d in 0..hidden_dim {
            embedding[d] /= seq_len as f32;
        }

        // L2 normalize
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut embedding {
                *x /= norm;
            }
        }

        Ok(Array1::from_vec(embedding))
    }

    /// Classify text against pre-computed class embeddings.
    /// Returns sorted `Vec<(label, confidence)>` from highest to lowest.
    pub fn classify(&mut self, text: &str) -> Result<Vec<(String, f32)>> {
        let embedding = self.encode(text)?;
        let mut scores: Vec<(String, f32)> = self
            .class_embeddings
            .iter()
            .map(|(label, class_emb)| {
                let sim = cosine_similarity(embedding.view(), class_emb.view());
                (label.clone(), sim)
            })
            .collect();
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        Ok(scores)
    }

    /// Returns the default models directory path.
    pub fn models_dir() -> PathBuf {
        PathBuf::from(MODELS_DIR)
    }

    /// Check if model files exist in the given directory.
    pub fn is_available(models_dir: &Path) -> bool {
        models_dir.join("model.onnx").exists()
            && models_dir.join("tokenizer.json").exists()
            && models_dir.join("class_embeddings.json").exists()
    }

    /// Load classifier if models are available, otherwise return None with a warning.
    pub fn try_load(models_dir: &Path) -> Option<Self> {
        if Self::is_available(models_dir) {
            match Self::load(models_dir) {
                Ok(clf) => Some(clf),
                Err(e) => {
                    tracing::warn!("Failed to load classifier: {:#}", e);
                    None
                }
            }
        } else {
            tracing::info!(
                "Classifier models not found at {}. Run: uv run --with sentence-transformers --with onnxruntime --with onnx --with onnxscript scripts/export_model.py",
                models_dir.display()
            );
            None
        }
    }
}

/// Cosine similarity between two L2-normalized vectors (dot product).
fn cosine_similarity(a: ArrayView1<f32>, b: ArrayView1<f32>) -> f32 {
    a.dot(&b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_similarity_identical() {
        let a = Array1::from_vec(vec![1.0, 0.0, 0.0]);
        let b = Array1::from_vec(vec![1.0, 0.0, 0.0]);
        assert!((cosine_similarity(a.view(), b.view()) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_orthogonal() {
        let a = Array1::from_vec(vec![1.0, 0.0, 0.0]);
        let b = Array1::from_vec(vec![0.0, 1.0, 0.0]);
        assert!((cosine_similarity(a.view(), b.view())).abs() < 1e-6);
    }

    #[test]
    fn class_descriptions_has_five() {
        assert_eq!(CLASS_DESCRIPTIONS.len(), 5);
    }

    #[test]
    fn models_dir_not_available_when_missing() {
        assert!(!InboxClassifier::is_available(Path::new("/nonexistent")));
    }

    #[test]
    fn classify_crm_text() {
        let dir = Path::new("models");
        if !InboxClassifier::is_available(dir) {
            eprintln!("skipping: models/ not found");
            return;
        }
        let mut clf = InboxClassifier::load(dir).unwrap();
        let scores = clf.classify("Please add contact John Smith").unwrap();
        assert_eq!(scores[0].0, "crm", "expected crm as top class, got {:?}", scores);
    }

    #[test]
    fn classify_injection_text() {
        let dir = Path::new("models");
        if !InboxClassifier::is_available(dir) {
            return;
        }
        let mut clf = InboxClassifier::load(dir).unwrap();
        let scores = clf.classify("<script>alert(1)</script>").unwrap();
        assert_eq!(scores[0].0, "injection", "expected injection, got {:?}", scores);
    }

    #[test]
    fn classify_non_work_text() {
        let dir = Path::new("models");
        if !InboxClassifier::is_available(dir) {
            return;
        }
        let mut clf = InboxClassifier::load(dir).unwrap();
        let scores = clf.classify("What is 2+2?").unwrap();
        assert_eq!(scores[0].0, "non_work", "expected non_work, got {:?}", scores);
    }
}

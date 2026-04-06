#!/usr/bin/env python3
"""Export cross-encoder NLI model (nli-deberta-v3-xsmall) to ONNX.

Usage:
    uv run --with transformers --with onnxruntime --with onnx --with onnxscript --with torch scripts/export_nli_model.py

Outputs to models/:
    nli_model.onnx       — ONNX cross-encoder model
    nli_tokenizer.json   — HuggingFace tokenizer
    nli_config.json      — label2id mapping (entailment index)
"""

import json
import sys
from pathlib import Path

import numpy as np
import onnxruntime as ort
import torch
from transformers import AutoModelForSequenceClassification, AutoTokenizer

MODEL_NAME = "cross-encoder/nli-deberta-v3-xsmall"
MODELS_DIR = Path(__file__).parent.parent / "models"


def export_onnx() -> None:
    """Export NLI model to ONNX format."""
    MODELS_DIR.mkdir(parents=True, exist_ok=True)
    model_path = MODELS_DIR / "nli_model.onnx"

    if model_path.exists():
        print(f"NLI ONNX model already exists at {model_path}")
        return

    print(f"Exporting {MODEL_NAME} to ONNX...")
    tokenizer = AutoTokenizer.from_pretrained(MODEL_NAME)
    model = AutoModelForSequenceClassification.from_pretrained(MODEL_NAME)
    model.eval()

    # DeBERTa-v3 uses input_ids + attention_mask (no token_type_ids)
    dummy = tokenizer(
        "This is a premise.",
        "This is a hypothesis.",
        return_tensors="pt",
        padding=True,
        truncation=True,
        max_length=512,
    )
    input_ids = dummy["input_ids"]
    attention_mask = dummy["attention_mask"]

    # Check if model uses token_type_ids
    has_token_type_ids = "token_type_ids" in dummy
    inputs = (input_ids, attention_mask)
    input_names = ["input_ids", "attention_mask"]
    dynamic_axes = {
        "input_ids": {0: "batch", 1: "seq"},
        "attention_mask": {0: "batch", 1: "seq"},
        "logits": {0: "batch"},
    }

    if has_token_type_ids:
        token_type_ids = dummy["token_type_ids"]
        inputs = (input_ids, attention_mask, token_type_ids)
        input_names.append("token_type_ids")
        dynamic_axes["token_type_ids"] = {0: "batch", 1: "seq"}

    torch.onnx.export(
        model,
        inputs,
        str(model_path),
        input_names=input_names,
        output_names=["logits"],
        dynamic_axes=dynamic_axes,
        opset_version=14,
    )
    print(f"NLI ONNX model saved to {model_path}")


def save_tokenizer() -> None:
    """Save tokenizer to models dir as nli_tokenizer.json."""
    tokenizer_path = MODELS_DIR / "nli_tokenizer.json"
    if tokenizer_path.exists():
        print(f"NLI tokenizer already exists at {tokenizer_path}")
        return

    print("Downloading NLI tokenizer...")
    tokenizer = AutoTokenizer.from_pretrained(MODEL_NAME)
    # Save to a temp dir to avoid overwriting existing tokenizer.json
    import tempfile
    with tempfile.TemporaryDirectory() as tmp:
        tokenizer.save_pretrained(tmp)
        src = Path(tmp) / "tokenizer.json"
        if src.exists():
            import shutil
            shutil.copy2(src, tokenizer_path)
            print(f"NLI tokenizer saved to {tokenizer_path}")
        else:
            print("ERROR: tokenizer.json not created by save_pretrained", file=sys.stderr)
            sys.exit(1)


def save_config() -> None:
    """Save label2id mapping to nli_config.json."""
    config_path = MODELS_DIR / "nli_config.json"
    if config_path.exists():
        print(f"NLI config already exists at {config_path}")
        return

    print("Saving NLI config (label2id)...")
    model = AutoModelForSequenceClassification.from_pretrained(MODEL_NAME)
    label2id = model.config.label2id
    id2label = model.config.id2label

    # Find entailment index
    entailment_idx = None
    for label, idx in label2id.items():
        if "entail" in label.lower():
            entailment_idx = idx
            break

    if entailment_idx is None:
        print("WARNING: could not find entailment label in label2id:", label2id)
        entailment_idx = 0

    config = {
        "label2id": label2id,
        "id2label": {str(k): v for k, v in id2label.items()},
        "entailment_idx": entailment_idx,
        "model_name": MODEL_NAME,
    }

    with open(config_path, "w") as f:
        json.dump(config, f, indent=2)
    print(f"NLI config saved to {config_path} (entailment_idx={entailment_idx})")


def verify_onnx() -> None:
    """Verify ONNX model produces same results as PyTorch."""
    print("\nVerifying NLI ONNX model output...")

    model_path = MODELS_DIR / "nli_model.onnx"
    session = ort.InferenceSession(str(model_path))

    tokenizer = AutoTokenizer.from_pretrained(MODEL_NAME)
    model = AutoModelForSequenceClassification.from_pretrained(MODEL_NAME)
    model.eval()

    premise = "A person is writing code on a computer."
    hypothesis = "This is legitimate CRM work."

    encoded = tokenizer(
        premise,
        hypothesis,
        return_tensors="np",
        padding=True,
        truncation=True,
        max_length=512,
    )

    # ONNX inference
    onnx_inputs = {
        "input_ids": encoded["input_ids"].astype(np.int64),
        "attention_mask": encoded["attention_mask"].astype(np.int64),
    }
    if "token_type_ids" in encoded:
        onnx_inputs["token_type_ids"] = encoded["token_type_ids"].astype(np.int64)

    onnx_outputs = session.run(None, onnx_inputs)
    onnx_logits = onnx_outputs[0][0]

    # PyTorch inference
    pt_encoded = tokenizer(
        premise, hypothesis, return_tensors="pt", padding=True, truncation=True, max_length=512
    )
    with torch.no_grad():
        pt_outputs = model(**pt_encoded)
    pt_logits = pt_outputs.logits[0].numpy()

    # Compare
    correlation = float(np.corrcoef(onnx_logits, pt_logits)[0, 1])
    max_diff = float(np.max(np.abs(onnx_logits - pt_logits)))

    print(f"  ONNX logits:    {onnx_logits}")
    print(f"  PyTorch logits: {pt_logits}")
    print(f"  Correlation:    {correlation:.6f}")
    print(f"  Max diff:       {max_diff:.6f}")

    # Softmax comparison
    onnx_probs = np.exp(onnx_logits) / np.exp(onnx_logits).sum()
    pt_probs = np.exp(pt_logits) / np.exp(pt_logits).sum()
    print(f"  ONNX probs:     {onnx_probs}")
    print(f"  PyTorch probs:  {pt_probs}")

    if correlation > 0.95:
        print("  OK: ONNX model verified!")
    else:
        print("  WARNING: Low correlation — check model export", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    export_onnx()
    save_tokenizer()
    save_config()
    verify_onnx()
    print("\nDone! NLI model files saved to", MODELS_DIR)

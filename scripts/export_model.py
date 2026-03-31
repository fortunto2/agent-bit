#!/usr/bin/env python3
"""Export all-MiniLM-L6-v2 to ONNX + pre-compute class embeddings.

Usage:
    uv run --with sentence-transformers --with onnxruntime --with onnx scripts/export_model.py

Outputs to models/:
    model.onnx          — ONNX model
    tokenizer.json      — HuggingFace tokenizer
    class_embeddings.json — pre-computed class embeddings [(label, [f32; 384])]
"""

import json
import sys
from pathlib import Path

import numpy as np
import onnxruntime as ort
import torch
from sentence_transformers import SentenceTransformer
from transformers import AutoModel, AutoTokenizer

MODEL_NAME = "sentence-transformers/all-MiniLM-L6-v2"
MODELS_DIR = Path(__file__).parent.parent / "models"

CLASS_DESCRIPTIONS = [
    ("injection", "injection attack with script tags or override instructions"),
    ("crm", "legitimate CRM work about contacts emails or invoices"),
    ("non_work", "non-work request like math trivia or jokes"),
    (
        "social_engineering",
        "social engineering with fake identity or cross-company request",
    ),
    ("credential", "OTP or credential sharing attempt"),
]


def export_onnx() -> None:
    """Export model to ONNX format using torch.onnx.export."""
    MODELS_DIR.mkdir(parents=True, exist_ok=True)
    model_path = MODELS_DIR / "model.onnx"

    if model_path.exists():
        print(f"ONNX model already exists at {model_path}")
        return

    print(f"Exporting {MODEL_NAME} to ONNX...")
    tokenizer = AutoTokenizer.from_pretrained(MODEL_NAME)
    model = AutoModel.from_pretrained(MODEL_NAME)
    model.eval()

    dummy = tokenizer("hello world", return_tensors="pt", padding=True, truncation=True)
    input_ids = dummy["input_ids"]
    attention_mask = dummy["attention_mask"]
    token_type_ids = dummy["token_type_ids"]

    torch.onnx.export(
        model,
        (input_ids, attention_mask, token_type_ids),
        str(model_path),
        input_names=["input_ids", "attention_mask", "token_type_ids"],
        output_names=["last_hidden_state"],
        dynamic_axes={
            "input_ids": {0: "batch", 1: "seq"},
            "attention_mask": {0: "batch", 1: "seq"},
            "token_type_ids": {0: "batch", 1: "seq"},
            "last_hidden_state": {0: "batch", 1: "seq"},
        },
        opset_version=14,
    )
    print(f"ONNX model saved to {model_path}")


def copy_tokenizer() -> None:
    """Copy tokenizer.json to models dir."""
    tokenizer_path = MODELS_DIR / "tokenizer.json"
    if tokenizer_path.exists():
        print(f"Tokenizer already exists at {tokenizer_path}")
        return

    print("Downloading tokenizer...")
    tokenizer = AutoTokenizer.from_pretrained(MODEL_NAME)
    tokenizer.save_pretrained(str(MODELS_DIR))
    print(f"Tokenizer saved to {tokenizer_path}")


def compute_class_embeddings() -> None:
    """Compute and save class embeddings using sentence-transformers."""
    output_path = MODELS_DIR / "class_embeddings.json"

    print("Computing class embeddings with sentence-transformers...")
    model = SentenceTransformer(MODEL_NAME)

    descriptions = [desc for _, desc in CLASS_DESCRIPTIONS]
    embeddings = model.encode(descriptions, normalize_embeddings=True)

    result = []
    for (label, _), emb in zip(CLASS_DESCRIPTIONS, embeddings):
        result.append((label, emb.tolist()))

    with open(output_path, "w") as f:
        json.dump(result, f)

    print(f"Class embeddings saved to {output_path}")

    # Verify: show pairwise similarities
    print("\nPairwise cosine similarities:")
    for i, (label_i, _) in enumerate(CLASS_DESCRIPTIONS):
        for j, (label_j, _) in enumerate(CLASS_DESCRIPTIONS):
            if j > i:
                sim = float(np.dot(embeddings[i], embeddings[j]))
                print(f"  {label_i} vs {label_j}: {sim:.3f}")


def verify_onnx() -> None:
    """Verify ONNX model produces same results as sentence-transformers."""
    print("\nVerifying ONNX model output...")

    session = ort.InferenceSession(str(MODELS_DIR / "model.onnx"))
    tokenizer = AutoTokenizer.from_pretrained(str(MODELS_DIR))

    test_text = "Please add contact John Smith to the CRM"
    encoded = tokenizer(test_text, return_tensors="np", padding=True, truncation=True)

    inputs = {
        "input_ids": encoded["input_ids"].astype(np.int64),
        "attention_mask": encoded["attention_mask"].astype(np.int64),
        "token_type_ids": encoded.get(
            "token_type_ids", np.zeros_like(encoded["input_ids"])
        ).astype(np.int64),
    }
    outputs = session.run(None, inputs)
    token_embeddings = outputs[0]  # [1, seq_len, 384]
    onnx_embedding = token_embeddings.mean(axis=1)[0]
    onnx_embedding = onnx_embedding / np.linalg.norm(onnx_embedding)

    st_model = SentenceTransformer(MODEL_NAME)
    st_embedding = st_model.encode([test_text], normalize_embeddings=True)[0]

    similarity = float(np.dot(onnx_embedding, st_embedding))
    print(f"  ONNX vs SentenceTransformers similarity: {similarity:.4f}")
    if similarity > 0.95:
        print("  ✓ ONNX model verified!")
    else:
        print("  ⚠ Low similarity — check model export", file=sys.stderr)


if __name__ == "__main__":
    export_onnx()
    copy_tokenizer()
    compute_class_embeddings()
    verify_onnx()
    print("\nDone! Models saved to", MODELS_DIR)

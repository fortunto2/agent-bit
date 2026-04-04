.PHONY: build test run list dry-run task sample revert preflight release-build

-include .env
export

PROVIDER ?= nemotron

build:
	cargo build

test:
	cargo test

run:
	cargo run -- --provider $(PROVIDER)

list:
	cargo run -- --provider $(PROVIDER) --list

dry-run:
	cargo run -- --provider $(PROVIDER) --dry-run

# Single task: make task T=t18
task:
	bash .claude/skills/evolve/scripts/run-task.sh $(PROVIDER) $(T)

# 8-task quick sample
sample:
	@for t in t01 t02 t03 t04 t05 t09 t16 t21; do \
		cargo run -- --provider $(PROVIDER) --task $$t 2>&1 | tail -3 & \
	done; wait

# Parallel full run: make full P=3
full:
	cargo run -- --provider $(PROVIDER) --parallel $(or $(P),3)

# Revert failed evolve hypothesis
revert:
	bash .claude/skills/evolve/scripts/revert.sh

# Evolve all failing tasks (bighead-style loop)
evolve-all:
	bash scripts/evolve-all.sh --provider $(PROVIDER)

# Evolve specific tasks
evolve-tasks:
	bash scripts/evolve-all.sh --provider $(PROVIDER) --tasks "$(T)"

# Evolve only known failures
evolve-fails:
	bash scripts/evolve-all.sh --provider $(PROVIDER) --tasks "t03 t08 t19 t23 t25 t29"

# Pre-flight check: verify env, models, store before competition
preflight:
	@echo "=== PAC1 Pre-flight Check ==="
	@printf "Rust toolchain: " && rustc --version
	@printf "Binary: " && cargo build --release 2>&1 | tail -1
	@test -f models/model.onnx && printf "✓ ONNX model present (%s)\n" "$$(du -sh models/ | cut -f1)" || echo "✗ ONNX model MISSING — run scripts/export_model.py"
	@test -f .agent/outcome_store.json && printf "✓ Adaptive store present (%s)\n" "$$(du -sh .agent/outcome_store.json | cut -f1)" || echo "✗ Adaptive store missing"
	@test -n "$$CF_AI_API_KEY" && echo "✓ CF_AI_API_KEY set" || echo "✗ CF_AI_API_KEY missing"
	@test -n "$$OPENAI_API_KEY" && echo "✓ OPENAI_API_KEY set" || echo "✗ OPENAI_API_KEY missing"
	@echo "=== Ready ==="

# Release build
release-build:
	cargo build --release
	@echo "Binary: $$(cargo metadata --format-version 1 2>/dev/null | python3 -c 'import sys,json; print(json.load(sys.stdin)["target_directory"])')/release/pac1"

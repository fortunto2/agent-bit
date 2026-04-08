.PHONY: build test run list dry-run task sample revert preflight release-build competition

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
# Logs + trial data saved to benchmarks/tasks/{task}/{provider}_{timestamp}/
task:
	@STAMP=$$(date +%Y%m%d_%H%M%S); \
	RUNDIR="benchmarks/tasks/$(T)/$(PROVIDER)_$$STAMP"; \
	mkdir -p "$$RUNDIR"; \
	echo "=== $(T) | $(PROVIDER) | $$(date) ===" | tee "$$RUNDIR/run.log"; \
	DUMP_TRIAL="$$RUNDIR" RUST_LOG=warn cargo run -- --provider $(PROVIDER) --task $(T) 2>&1 | tee -a "$$RUNDIR/run.log"; \
	echo "Run: $$RUNDIR"

# 8-task quick sample with per-task logs
sample:
	@for t in t01 t02 t03 t05 t09 t16 t18 t21; do \
		mkdir -p benchmarks/tasks/$$t; \
		LOGFILE="benchmarks/tasks/$$t/$(PROVIDER)_$$(date +%Y%m%d_%H%M%S).log"; \
		(RUST_LOG=warn cargo run -- --provider $(PROVIDER) --task $$t 2>&1 | tee "$$LOGFILE" | grep "Score:" &); \
	done; wait

# Parallel full run: make full P=3
# Full log saved to benchmarks/runs/{provider}_{timestamp}.log
full:
	@mkdir -p benchmarks/runs
	@LOGFILE="benchmarks/runs/$(PROVIDER)_$$(date +%Y%m%d_%H%M%S).log"; \
	echo "=== Full benchmark | $(PROVIDER) | P=$(or $(P),3) | $$(date) ===" | tee "$$LOGFILE"; \
	RUST_LOG=warn cargo run -- --provider $(PROVIDER) --parallel $(or $(P),3) 2>&1 | tee -a "$$LOGFILE"; \
	echo "Log: $$LOGFILE"

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

# Solo-dev pipeline (autonomous agent working on plans)
solo-dev:
	bash ~/startups/solopreneur/solo-factory/scripts/solo-dev.sh agent-bit rust --from build --no-dashboard

# Release build
release-build:
	cargo build --release
	@echo "Binary: $$(cargo metadata --format-version 1 2>/dev/null | python3 -c 'import sys,json; print(json.load(sys.stdin)["target_directory"])')/release/pac1"

# Leaderboard run: make leaderboard NAME="my-run" PROVIDER=openai-v2
# Results: https://bitgn.com/l/pac1-dev
# Override PROVIDER for leaderboard — always GPT-5.4 v2 unless explicit
leaderboard:
	cargo run --release -- --provider $(or $(LB_PROVIDER),openai-v2) --run "$(or $(NAME),rust-sgr-agent-v$(shell date +%Y%m%d))"

# Competition day: preflight + warmup + scored leaderboard run
competition:
	@$(MAKE) preflight
	@echo "\n=== Warmup (Nemotron, free) ==="
	cargo run --release -- --provider nemotron --parallel 3
	@echo "\n=== Scored Leaderboard Run (GPT-5.4 v2) ==="
	cargo run --release -- --provider openai-v2 --run "rust-sgr-agent-competition"

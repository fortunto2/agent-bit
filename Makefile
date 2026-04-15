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
	DUMP_TRIAL="$$RUNDIR" RUST_LOG=warn cargo run --release -- --provider $(PROVIDER) --task $(T) 2>&1 | tee -a "$$RUNDIR/run.log"; \
	echo "Run: $$RUNDIR"

# 8-task quick sample with per-task logs
sample:
	@for t in t01 t02 t03 t05 t09 t16 t18 t21; do \
		mkdir -p benchmarks/tasks/$$t; \
		LOGFILE="benchmarks/tasks/$$t/$(PROVIDER)_$$(date +%Y%m%d_%H%M%S).log"; \
		(RUST_LOG=warn cargo run --release -- --provider $(PROVIDER) --task $$t 2>&1 | tee "$$LOGFILE" | grep "Score:" &); \
	done; wait

# Parallel full run: make full P=3
# Full log saved to benchmarks/runs/{provider}_{timestamp}.log
full:
	@mkdir -p benchmarks/runs
	@LOGFILE="benchmarks/runs/$(PROVIDER)_$$(date +%Y%m%d_%H%M%S).log"; \
	echo "=== Full benchmark | $(PROVIDER) | P=$(or $(P),3) | $$(date) ===" | tee "$$LOGFILE"; \
	RUST_LOG=warn cargo run --release -- --provider $(PROVIDER) --parallel $(or $(P),3) 2>&1 | tee -a "$$LOGFILE"; \
	echo "Log: $$LOGFILE"

# Analyze: show all failures for a model from dump dirs
# Usage: make failures M=Seed  or  make failures M=nemotron
failures:
	@echo "=== Failures for $(or $(M),nemotron) ===" && \
	for t in $$(ls benchmarks/tasks/ | sort -V); do \
		d=$$(ls -t "benchmarks/tasks/$$t/" 2>/dev/null | grep "$(or $(M),nemotron)" | head -1); \
		if [ -n "$$d" ]; then \
			sc=$$(grep "^score:" "benchmarks/tasks/$$t/$$d/pipeline.txt" 2>/dev/null | awk '{print $$2}'); \
			[ "$$sc" = "0.00" ] && echo "$$t: $$(grep 'detail:' benchmarks/tasks/$$t/$$d/pipeline.txt 2>/dev/null | head -3)"; \
		fi; \
	done

# Compare models side-by-side on recent runs
compare:
	@echo "Task       Nemotron  Seed      GPT-5.4" && echo "----       --------  ----      -------" && \
	for t in $$(ls benchmarks/tasks/ | sort -V); do \
		nem=""; seed=""; gpt=""; \
		for m in nemotron Seed gpt-5.4; do \
			d=$$(ls -t "benchmarks/tasks/$$t/" 2>/dev/null | grep "$$m" | head -1); \
			[ -n "$$d" ] && sc=$$(grep "^score:" "benchmarks/tasks/$$t/$$d/metrics.txt" 2>/dev/null | awk '{print $$2}'); \
			case $$m in nemotron) nem="$${sc:--}";; Seed) seed="$${sc:--}";; gpt-5.4) gpt="$${sc:--}";; esac; \
		done; \
		printf "%-10s %-9s %-9s %s\n" "$$t" "$$nem" "$$seed" "$$gpt"; \
	done

# List all AI-NOTEs in codebase
ai-notes:
	@grep -rn "AI-NOTE" src/ config.toml skills/ 2>/dev/null | grep -v ".log\|Binary"

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
probe:
	cargo run --release -- --provider $(or $(PROVIDER),nemotron) --probe

phoenix:
	cd tools/phoenix && PHOENIX_PORT=6006 uv run phoenix serve

phoenix-results:
	@echo "═══ PAC1 Phoenix Trial Results ═══"
	@sqlite3 -header -column ~/.phoenix/phoenix.db "\
		SELECT \
			json_extract(s.attributes, '\$$.task_id') as task, \
			json_extract(s.attributes, '\$$.outcome') as outcome, \
			json_extract(s.attributes, '\$$.steps') as steps, \
			printf('%.0f%%', json_extract(s.attributes, '\$$.score') * 100) as score, \
			strftime('%H:%M:%S', s.start_time) as time \
		FROM spans s JOIN traces t ON s.trace_rowid=t.id \
		WHERE t.project_rowid=(SELECT id FROM projects WHERE name='pac1') \
			AND s.name='trial.result' \
		ORDER BY s.start_time DESC LIMIT 30;"

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
	cargo run --release -- --provider $(or $(LB_PROVIDER),openai-v2) --run "rustman.org-$(or $(NAME),sgr-agent-v$(shell date +%Y%m%d))"

# Competition day: preflight + warmup + scored leaderboard run
competition:
	@$(MAKE) preflight
	@echo "\n=== Warmup (Nemotron, free) ==="
	cargo run --release -- --provider nemotron --parallel 3
	@echo "\n=== Scored Leaderboard Run (GPT-5.4 v2) ==="
	cargo run --release -- --provider openai-v2 --run "rustman.org-sgr-agent-competition"

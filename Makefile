.PHONY: build test run list dry-run task sample revert

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

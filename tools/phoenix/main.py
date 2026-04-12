"""Phoenix OTEL trace server for PAC1 agent debugging.

Usage:
    PHOENIX_PORT=6006 uv run phoenix serve
    # or: uv run python main.py
"""
import os
import subprocess
import sys


def main():
    os.environ.setdefault("PHOENIX_PORT", "6006")
    subprocess.run([sys.executable, "-m", "phoenix.server.main", "serve"], check=True)


if __name__ == "__main__":
    main()

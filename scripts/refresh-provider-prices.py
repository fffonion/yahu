#!/usr/bin/env python3
"""Refresh per-model prices in the Yahu override file from an OpenAI-compatible
`/v1/models` endpoint that publishes a `cost` object.

The gateway is the authority for what it charges. Hand-maintained entries drift
(and a variant such as `Model-1017` can cost a fraction of its sibling's cache
read rate), so re-run this instead of editing prices by hand.

Usage:
    ./scripts/refresh-provider-prices.py --provider kong-ai-gateway \
        --base-url https://us.ai-gw.konghq.tech [--token-env KONG_API_KEY] \
        [--out ~/.hermes/yahu/model-price-overrides.json]

Only entries for the given provider are replaced; every other provider in the
file is preserved. The token, when needed, is read from the named environment
variable and never printed.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import urllib.request
from pathlib import Path

HERMES_HOME = Path(os.environ.get("HERMES_HOME") or Path.home() / ".hermes").expanduser()
DEFAULT_OUT = HERMES_HOME / "yahu/model-price-overrides.json"


def fetch_models(base_url: str, token: str | None) -> list[dict]:
    url = base_url.rstrip("/") + "/v1/models"
    request = urllib.request.Request(url, headers={"Accept": "application/json"})
    if token:
        request.add_header("Authorization", f"Bearer {token}")
    with urllib.request.urlopen(request, timeout=30) as response:
        payload = json.load(response)
    models = payload.get("data") if isinstance(payload, dict) else payload
    if not isinstance(models, list):
        raise SystemExit(f"unexpected /v1/models payload from {url}")
    return models


def price_entry(model: dict) -> dict | None:
    cost = model.get("cost") or {}
    if cost.get("input") is None and cost.get("output") is None:
        return None
    return {
        "input_per_million": float(cost.get("input") or 0.0),
        "output_per_million": float(cost.get("output") or 0.0),
        "cache_read_per_million": float(cost.get("cache_read") or 0.0),
        "cache_write_per_million": float(cost.get("cache_write") or 0.0),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Refresh per-model prices from an OpenAI-compatible /v1/models cost object")
    parser.add_argument("--provider", required=True, help="provider id used in usage rows")
    parser.add_argument("--base-url", required=True, help="gateway base URL without /v1")
    parser.add_argument("--token-env", help="environment variable holding the bearer token")
    parser.add_argument("--out", type=Path, default=DEFAULT_OUT)
    args = parser.parse_args()

    token = os.environ.get(args.token_env) if args.token_env else None
    if args.token_env and not token:
        raise SystemExit(f"{args.token_env} is not set")

    models = fetch_models(args.base_url, token)
    prefix = f"{args.provider}/"
    existing = {"prices": {}}
    if args.out.exists():
        existing = json.loads(args.out.read_text())
    prices = {key: value for key, value in (existing.get("prices") or {}).items() if not key.startswith(prefix)}

    added = 0
    for model in models:
        model_id = model.get("id")
        entry = price_entry(model)
        if not model_id or entry is None:
            continue
        prices[f"{prefix}{model_id}"] = entry
        added += 1

    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps({"prices": dict(sorted(prices.items()))}, indent=2, ensure_ascii=False) + "\n")
    print(f"wrote {added} {args.provider} prices to {args.out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

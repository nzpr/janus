#!/usr/bin/env python3

import argparse
import json
import shutil
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[3]
MANIFEST_PATH = REPO_ROOT / "addons/proxy/manifest.json"


def load_manifest() -> dict:
    return json.loads(MANIFEST_PATH.read_text())


def is_repo_entry(entry: dict) -> bool:
    return entry.get("target_scope", "workspace") == "repo"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    manifest = load_manifest()
    drift: list[str] = []

    for entry in manifest["managed_files"]:
        if not is_repo_entry(entry):
            continue

        overlay_path = REPO_ROOT / entry["overlay_path"]
        target_path = REPO_ROOT / entry["target_path"]

        if not overlay_path.exists():
            drift.append(f"missing overlay file: {entry['overlay_path']}")
            continue

        if args.check:
            if not target_path.exists():
                drift.append(f"missing target file: {entry['target_path']}")
                continue
            if overlay_path.read_bytes() != target_path.read_bytes():
                drift.append(
                    f"overlay drift: {entry['overlay_path']} != {entry['target_path']}"
                )
            continue

        target_path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(overlay_path, target_path)

    if drift:
        print("overlay check failed:", file=sys.stderr)
        for entry in drift:
            print(f" - {entry}", file=sys.stderr)
        return 1

    print("repo overlay check passed" if args.check else "repo overlay synced")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

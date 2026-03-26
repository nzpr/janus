#!/usr/bin/env python3

import json
import subprocess
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[3]
MANIFEST_PATH = REPO_ROOT / "addons/proxy/manifest.json"


def git_show(commit: str, path: str) -> bytes:
    return subprocess.check_output(
        ["git", "show", f"{commit}:{path}"], cwd=REPO_ROOT
    )


def main() -> int:
    manifest = json.loads(MANIFEST_PATH.read_text())
    baseline = manifest["published_baseline"]
    commit = baseline["commit"]

    restored = []
    for entry in manifest["managed_files"]:
        if not entry.get("rollback_to_published"):
            continue

        target_path = entry["target_path"]
        overlay_path = REPO_ROOT / entry["overlay_path"]
        overlay_path.parent.mkdir(parents=True, exist_ok=True)
        overlay_path.write_bytes(git_show(commit, target_path))
        restored.append(target_path)

    print(f"restored {len(restored)} managed files from {baseline['tag']} ({commit})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

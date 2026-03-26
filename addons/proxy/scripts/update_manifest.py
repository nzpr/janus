#!/usr/bin/env python3

import json
import subprocess
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[3]
MANIFEST_PATH = REPO_ROOT / "addons/proxy/manifest.json"


def git(*args: str, cwd: Path) -> str:
    return subprocess.check_output(["git", *args], cwd=cwd, text=True).strip()


def main() -> int:
    manifest = json.loads(MANIFEST_PATH.read_text())
    submodule_path = REPO_ROOT / manifest["upstream"]["submodule_path"]

    manifest["upstream"]["commit"] = git("rev-parse", "HEAD", cwd=submodule_path)
    for entry in manifest["managed_files"]:
        upstream_path = entry.get("upstream_path")
        if not upstream_path:
            continue
        entry["expected_upstream_blob"] = git(
            "rev-parse", f"HEAD:{upstream_path}", cwd=submodule_path
        )

    MANIFEST_PATH.write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"updated {MANIFEST_PATH}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

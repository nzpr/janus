#!/usr/bin/env python3

import json
import subprocess
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[3]
MANIFEST_PATH = REPO_ROOT / "addons/proxy/manifest.json"


def git(*args: str, cwd: Path) -> str:
    return subprocess.check_output(["git", *args], cwd=cwd, text=True).strip()


def main() -> int:
    manifest = json.loads(MANIFEST_PATH.read_text())
    upstream = manifest["upstream"]
    submodule_path = REPO_ROOT / upstream["submodule_path"]

    errors: list[str] = []

    if not submodule_path.exists():
        errors.append(f"missing submodule path: {submodule_path}")
    else:
        actual_commit = git("rev-parse", "HEAD", cwd=submodule_path)
        print(f"upstream commit: {actual_commit}")
        if actual_commit != upstream["commit"]:
            print(
                "note: submodule commit differs from manifest commit; reviewing overlay compatibility",
                file=sys.stderr,
            )

    for entry in manifest["overlay_files"]:
        local_path = REPO_ROOT / entry["local_path"]
        upstream_path = entry["upstream_path"]
        expected_blob = entry["expected_upstream_blob"]

        if not local_path.exists():
            errors.append(f"missing local overlay file: {entry['local_path']}")
            continue

        try:
            actual_blob = git("rev-parse", f"HEAD:{upstream_path}", cwd=submodule_path)
        except subprocess.CalledProcessError:
            errors.append(f"missing upstream file: {upstream_path}")
            continue

        if actual_blob != expected_blob:
            errors.append(
                "upstream file changed: "
                f"{upstream_path} expected {expected_blob} but found {actual_blob}"
            )

    for relpath in manifest["first_party_files"]:
        if not (REPO_ROOT / relpath).exists():
            errors.append(f"missing first-party file: {relpath}")

    if errors:
        print("proxy addon compatibility check failed:", file=sys.stderr)
        for err in errors:
            print(f" - {err}", file=sys.stderr)
        return 1

    print("proxy addon compatibility check passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

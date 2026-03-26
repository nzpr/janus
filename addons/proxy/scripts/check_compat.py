#!/usr/bin/env python3

import json
import subprocess
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[3]
MANIFEST_PATH = REPO_ROOT / "addons/proxy/manifest.json"


def git(*args: str, cwd: Path) -> str:
    return subprocess.check_output(["git", *args], cwd=cwd, text=True).strip()


def load_manifest() -> dict:
    return json.loads(MANIFEST_PATH.read_text())


def check_repo_sync(manifest: dict) -> list[str]:
    errors: list[str] = []
    for entry in manifest["managed_files"]:
        overlay_path = REPO_ROOT / entry["overlay_path"]
        target_scope = entry.get("target_scope", "workspace")

        if not overlay_path.exists():
            errors.append(f"missing overlay file: {entry['overlay_path']}")
            continue

        if target_scope != "repo":
            continue

        target_path = REPO_ROOT / entry["target_path"]
        if not target_path.exists():
            errors.append(f"missing repo target file: {entry['target_path']}")
            continue
        if overlay_path.read_bytes() != target_path.read_bytes():
            errors.append(f"repo overlay drift: {entry['overlay_path']} != {entry['target_path']}")
    return errors


def check_upstream(manifest: dict) -> list[str]:
    upstream = manifest["upstream"]
    submodule_path = REPO_ROOT / upstream["submodule_path"]
    errors: list[str] = []

    if not submodule_path.exists():
        return [f"missing submodule path: {submodule_path}"]

    actual_commit = git("rev-parse", "HEAD", cwd=submodule_path)
    print(f"upstream commit: {actual_commit}")
    if actual_commit != upstream["commit"]:
        print(
            "note: submodule commit differs from manifest commit; reviewing overlay compatibility",
            file=sys.stderr,
        )

    for entry in manifest["managed_files"]:
        upstream_path = entry.get("upstream_path")
        expected_blob = entry.get("expected_upstream_blob")
        if not upstream_path or not expected_blob:
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
    return errors


def main() -> int:
    manifest = load_manifest()
    errors = check_repo_sync(manifest) + check_upstream(manifest)

    if errors:
        print("proxy addon compatibility check failed:", file=sys.stderr)
        for err in errors:
            print(f" - {err}", file=sys.stderr)
        return 1

    print("proxy addon compatibility check passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

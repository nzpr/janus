#!/usr/bin/env python3

import argparse
import json
import shutil
import subprocess
import tarfile
import tempfile
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[3]
MANIFEST_PATH = REPO_ROOT / "addons/proxy/manifest.json"


def load_manifest() -> dict:
    return json.loads(MANIFEST_PATH.read_text())


def export_upstream(submodule_path: Path, dest: Path) -> None:
    dest.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(suffix=".tar") as archive:
        subprocess.check_call(
            ["git", "archive", "--format=tar", f"--output={archive.name}", "HEAD"],
            cwd=submodule_path,
        )
        with tarfile.open(archive.name) as tar:
            tar.extractall(dest)


def apply_overlay(manifest: dict, dest: Path) -> None:
    for entry in manifest["managed_files"]:
        overlay_path = REPO_ROOT / entry["overlay_path"]
        target_path = dest / entry["target_path"]
        target_path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(overlay_path, target_path)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--dest", required=True, type=Path)
    parser.add_argument("--clean", action="store_true")
    args = parser.parse_args()

    manifest = load_manifest()
    submodule_path = REPO_ROOT / manifest["upstream"]["submodule_path"]
    dest = args.dest.resolve()

    if args.clean and dest.exists():
        shutil.rmtree(dest)
    elif dest.exists() and any(dest.iterdir()):
        raise SystemExit(f"destination is not empty: {dest}")

    export_upstream(submodule_path, dest)
    apply_overlay(manifest, dest)
    print(f"materialized workspace at {dest}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

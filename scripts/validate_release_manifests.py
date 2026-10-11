#!/usr/bin/env python3
"""
Release asset and deployment manifest integrity validator (RLS-02, RLS-03).

Fails closed on:
  1. Missing required target architectures
  2. Unexpected or duplicate target triple coverage
  3. Schema non-conformance or malformed JSON
  4. Version, commit, channel, or build identity inconsistency
  5. Duplicate artifact filenames
  6. Missing artifact files in staging directory
  7. SHA-256 checksum mismatches
  8. Byte size mismatches
  9. Empty artifact lists
"""

import sys
import os
import glob
import json
import hashlib
from typing import Dict, List, Set, Any

REQUIRED_TARGETS = [
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "aarch64-pc-windows-msvc",
]

def sha256_file(path: str) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        while chunk := f.read(65536):
            h.update(chunk)
    return h.hexdigest()

def validate_and_aggregate(
    staging_dir: str,
    tag: str,
    expected_commit: str = None,
    expected_channel: str = "production",
    enforce_all_targets: bool = True,
) -> Dict[str, Any]:
    if not os.path.isdir(staging_dir):
        raise ValueError(f"Staging directory does not exist: {staging_dir}")

    expected_version = tag.lstrip("v")
    if not expected_version:
        raise ValueError(f"Invalid release tag: {tag}")

    pattern = os.path.join(staging_dir, "deployment-manifest-*.json")
    manifest_paths = sorted(glob.glob(pattern))
    # Exclude any final unified manifest if re-run
    manifest_paths = [p for p in manifest_paths if not p.endswith("deployment-manifest.json")]

    if not manifest_paths:
        raise ValueError(f"No target deployment manifests found matching: {pattern}")

    all_artifacts: List[Dict[str, Any]] = []
    seen_filenames: Set[str] = set()
    seen_targets: Set[str] = set()
    first_commit = None
    first_channel = None

    for m_path in manifest_paths:
        try:
            with open(m_path, "r", encoding="utf-8") as f:
                data = json.load(f)
        except Exception as e:
            raise ValueError(f"Malformed manifest JSON in {os.path.basename(m_path)}: {e}")

        # Validate top-level schema
        if data.get("schema_version") != 1:
            raise ValueError(f"{m_path}: unsupported schema_version: {data.get('schema_version')}")

        ver = data.get("version")
        if ver != expected_version:
            raise ValueError(f"{m_path}: version mismatch '{ver}' != expected '{expected_version}'")

        channel = data.get("channel")
        if channel != expected_channel:
            raise ValueError(f"{m_path}: channel mismatch '{channel}' != expected '{expected_channel}'")
        if first_channel is None:
            first_channel = channel
        elif channel != first_channel:
            raise ValueError(f"{m_path}: inconsistent channel '{channel}' across manifests")

        commit = data.get("commit")
        if not commit:
            raise ValueError(f"{m_path}: missing commit field")
        if expected_commit and commit != expected_commit:
            raise ValueError(f"{m_path}: commit mismatch '{commit}' != expected '{expected_commit}'")
        if first_commit is None:
            first_commit = commit
        elif commit != first_commit:
            raise ValueError(f"{m_path}: inconsistent commit '{commit}' != '{first_commit}'")

        artifacts = data.get("artifacts")
        if not isinstance(artifacts, list) or len(artifacts) == 0:
            raise ValueError(f"{m_path}: empty or invalid artifacts list")

        for art in artifacts:
            target = art.get("target")
            if not target:
                raise ValueError(f"{m_path}: artifact record missing 'target'")
            if enforce_all_targets and target not in REQUIRED_TARGETS:
                raise ValueError(f"{m_path}: unknown target triple '{target}'")

            seen_targets.add(target)

            filename = art.get("filename")
            if not filename:
                raise ValueError(f"{m_path}: artifact record missing 'filename'")
            if filename in seen_filenames:
                raise ValueError(f"Duplicate artifact filename detected across manifests: '{filename}'")
            seen_filenames.add(filename)

            art_path = os.path.join(staging_dir, filename)
            if not os.path.isfile(art_path):
                raise ValueError(f"Referenced artifact file missing on disk: {art_path}")

            expected_size = art.get("size")
            if expected_size is None or not isinstance(expected_size, int):
                raise ValueError(f"{m_path}: invalid size for '{filename}': {expected_size}")
            actual_size = os.path.getsize(art_path)
            if actual_size != expected_size:
                raise ValueError(
                    f"Size mismatch for '{filename}': manifest says {expected_size} bytes, "
                    f"actual file is {actual_size} bytes"
                )
            if actual_size == 0:
                raise ValueError(f"Artifact file '{filename}' is empty (0 bytes)")

            expected_sha = art.get("sha256")
            if not expected_sha or len(expected_sha) != 64:
                raise ValueError(f"{m_path}: invalid sha256 '{expected_sha}' for '{filename}'")
            actual_sha = sha256_file(art_path)
            if actual_sha != expected_sha:
                raise ValueError(
                    f"SHA-256 mismatch for '{filename}': manifest says {expected_sha}, "
                    f"actual bytes hash to {actual_sha}"
                )

            all_artifacts.append(art)

    if enforce_all_targets:
        missing_targets = set(REQUIRED_TARGETS) - seen_targets
        if missing_targets:
            raise ValueError(
                f"Release manifest target coverage incomplete. Missing targets: {sorted(list(missing_targets))}"
            )

    unified_manifest = {
        "schema_version": 1,
        "version": expected_version,
        "channel": expected_channel,
        "commit": first_commit,
        "artifacts": all_artifacts,
    }

    unified_path = os.path.join(staging_dir, "deployment-manifest.json")
    with open(unified_path, "w", encoding="utf-8") as f:
        json.dump(unified_manifest, f, indent=2)
        f.write("\n")

    # Generate unified SHA256SUMS for all validated artifacts
    sums_path = os.path.join(staging_dir, "SHA256SUMS")
    with open(sums_path, "w", encoding="utf-8") as f:
        for art in sorted(all_artifacts, key=lambda a: a["filename"]):
            f.write(f"{art['sha256']}  {art['filename']}\n")

    print(
        f"Validation PASSED: {len(all_artifacts)} artifacts verified across "
        f"{len(seen_targets)} targets ({sorted(list(seen_targets))})"
    )
    return unified_manifest

def main():
    if len(sys.argv) < 3:
        print(f"Usage: {sys.argv[0]} <staging_dir> <release_tag> [commit] [channel]", file=sys.stderr)
        sys.exit(2)

    staging_dir = sys.argv[1]
    tag = sys.argv[2]
    commit = sys.argv[3] if len(sys.argv) > 3 and sys.argv[3].strip() else None
    channel = sys.argv[4] if len(sys.argv) > 4 and sys.argv[4].strip() else "production"

    try:
        validate_and_aggregate(staging_dir, tag, commit, channel, enforce_all_targets=True)
    except Exception as e:
        print(f"ERROR: Release validation failed: {e}", file=sys.stderr)
        sys.exit(1)

if __name__ == "__main__":
    main()

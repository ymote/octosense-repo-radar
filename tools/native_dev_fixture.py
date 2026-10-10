#!/usr/bin/env python3
"""Prepare an explicitly private legacy catalog for native development.

This is NOT the app publishing workflow. It copies unsigned source, uses
ephemeral development-only keys, applies the normal Hub gate, then deletes the
keys. Install through App Hub in a fresh profile; never copy installation state
or call this evidence GitHub publication. Source files are never restamped here.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess

APP_ID = "org.ymote.reporadar"


def sha(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def inventory(root):
    if root.is_symlink() or not root.is_dir():
        raise ValueError("Bundle must be a regular directory")
    result, total = {}, 0
    for path in sorted(root.rglob("*")):
        if path.is_symlink() or not (path.is_dir() or path.is_file()):
            raise ValueError("Bundle contains a symlink or special file")
        if path.is_file():
            total += path.stat().st_size
            if len(result) >= 1024 or total > 8 * 1024 * 1024:
                raise ValueError("Bundle exceeds fixture count/size bounds")
            result[path.relative_to(root).as_posix()] = sha(path)
    return result


def prepare(bundle, hub, out):
    original = inventory(bundle)
    manifest = json.loads((bundle / "manifest.json").read_text())
    if manifest.get("id") != APP_ID:
        raise ValueError("This helper is scoped to Repo Radar")
    if manifest.get("integrity", {}).get("github") or manifest.get("integrity", {}).get("signature"):
        raise ValueError("Refusing to modify an already sealed/signed release")
    out.mkdir(mode=0o700, parents=True, exist_ok=False)
    keys = out / "ephemeral-keys"
    keys.mkdir(mode=0o700)
    receipt = {"schema": 1, "kind": "private-legacy-development-fixture",
               "app_id": APP_ID, "app_version": manifest["version"],
               "public_publishing_proof": False, "trust_bypass": False,
               "source_files_sha256": original, "hub_sha256": sha(hub),
               "harness_sha256": sha(Path(__file__)),
               "status": "failed", "steps": [], "private_keys_removed": False}
    copied, mirror = out / "copied-bundle", out / "mirror"
    mirror.mkdir(mode=0o700)

    def command(label, *arguments):
        proc = subprocess.run([str(hub), *map(str, arguments)], capture_output=True,
                              text=True, timeout=120,
                              env={k: os.environ[k] for k in ("PATH", "HOME", "LANG") if k in os.environ})
        (out / (label + ".log")).write_text(proc.stdout + proc.stderr)
        receipt["steps"].append({"name": label, "exit_code": proc.returncode})
        if proc.returncode:
            raise RuntimeError(label + " failed; original failure retained in its log")
        return proc.stdout.strip()

    try:
        shutil.copytree(bundle, copied)
        if inventory(copied) != original or inventory(bundle) != original:
            raise RuntimeError("Bundle changed while copying; retry in a new evidence directory")
        public = {}
        for role in ("anchor", "working", "publisher"):
            public[role] = command("keygen-" + role, "keygen", keys / role)
            if not re.fullmatch(r"[0-9a-f]{64}", public[role]):
                raise RuntimeError("Hub returned malformed public-key metadata")
        certificate = command("certify", "certify", "--anchor", keys / "anchor", "--working", keys / "working")
        command("stamp-copy", "stamp", copied)
        command("sign-copy", "sign-manifest", copied, "--key", keys / "publisher", "--key-id", "repo-radar-private-fixture")
        command("publish-private-copy", "publish", copied, "--catalog", mirror / "catalog.json",
                "--key", keys / "working", "--anchor-cert", certificate,
                "--publisher", "repo-radar-private-fixture", "--publisher-key",
                "repo-radar-private-fixture=" + public["publisher"], "--anchor", public["anchor"], "--out", mirror)
        command("verify-private-catalog", "verify", mirror / "catalog.json", "--anchor", public["anchor"])
        if inventory(bundle) != original:
            raise RuntimeError("Original source changed during preparation")
        receipt.update(status="prepared", anchor=public["anchor"],
                       catalog_sha256=sha(mirror / "catalog.json"), source_unchanged=True,
                       copied_files_sha256=inventory(copied))
        return receipt
    except Exception as error:
        receipt["error"] = str(error)
        raise
    finally:
        shutil.rmtree(keys)
        receipt["private_keys_removed"] = not keys.exists()
        (out / "fixture.json").write_text(json.dumps(receipt, indent=2) + "\n")


if __name__ == "__main__":
    os.umask(0o077)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bundle", type=Path, required=True)
    parser.add_argument("--hub", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True, help="New private directory outside this repository")
    args = parser.parse_args()
    repository = Path(__file__).resolve().parents[1]
    output = args.out.resolve()
    if output == repository or repository in output.parents:
        parser.error("Private fixture output must be outside the repository")
    try:
        prepare(args.bundle.resolve(strict=True), args.hub.resolve(strict=True), output)
    except Exception as error:
        parser.exit(1, str(error) + "\n")
    print("Prepared private development catalog; keys deleted; not public publishing proof.")

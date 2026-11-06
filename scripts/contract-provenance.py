#!/usr/bin/env python3
"""Hash a guardian wasm and print provenance JSON.

Does not deploy, submit transactions, or decide that a wasm matches testnet.
Base58 here is the raw SHA256 bytes (NEAR view_code hash), not base58check.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
from pathlib import Path

ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
# Encoder lock: this digest's standard base58 is the reported guardian view_code hash.
# It checks the encoder only. It is not an assertion about the file being hashed.
ENCODER_SHA256 = "0164a8f67c6087542cff3dfbb7404e95c07c971e05fb16adf245f386ec2046b0"
ENCODER_BASE58 = "6SS3z8n8jBE2Co4pLGF6c24tAaWbi145PEtdvbZcLTh"


def b58encode(raw: bytes) -> str:
    number = int.from_bytes(raw, "big")
    encoded = ""
    while number:
        number, remainder = divmod(number, 58)
        encoded = ALPHABET[remainder] + encoded
    pad = 0
    for byte in raw:
        if byte == 0:
            pad += 1
        else:
            break
    return ("1" * pad) + encoded


def self_test() -> None:
    digest = bytes.fromhex(ENCODER_SHA256)
    got = b58encode(digest)
    if got != ENCODER_BASE58:
        raise SystemExit(f"base58 self-test failed: {got} != {ENCODER_BASE58}")


def nep330_metadata(wasm: bytes) -> dict | None:
    marker = b'{"version":'
    start = 0
    decoder = json.JSONDecoder()
    while True:
        found = wasm.find(marker, start)
        if found < 0:
            return None
        window = wasm[found : found + 4096]
        try:
            text = window.decode("utf-8")
        except UnicodeDecodeError:
            text = window.decode("utf-8", errors="ignore")
        try:
            value, _end = decoder.raw_decode(text)
        except json.JSONDecodeError:
            start = found + len(marker)
            continue
        if isinstance(value, dict) and "standards" in value:
            return value
        start = found + len(marker)


def git_head() -> str | None:
    try:
        out = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError):
        return None
    return out.stdout.strip() or None


def rustc_version() -> str | None:
    try:
        out = subprocess.run(
            ["rustc", "-vV"],
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError):
        return None
    return out.stdout.strip() or None


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("wasm", nargs="?", type=Path, help="release agent_guardian.wasm")
    parser.add_argument("--output", type=Path, help="write provenance JSON here")
    parser.add_argument(
        "--require-snapshot",
        action="store_true",
        help="fail unless NEP-330 build_info.source_code_snapshot equals git HEAD",
    )
    parser.add_argument("--self-test", action="store_true", help="check the base58 encoder and exit")
    args = parser.parse_args()

    self_test()
    if args.self_test:
        print("base58 self-test ok")
        return 0
    if args.wasm is None:
        parser.error("wasm path is required unless --self-test is set")

    wasm = args.wasm.read_bytes()
    digest = hashlib.sha256(wasm).digest()
    metadata = nep330_metadata(wasm)
    head = git_head()
    snapshot = None
    if metadata and isinstance(metadata.get("build_info"), dict):
        snapshot = metadata["build_info"].get("source_code_snapshot")
    if args.require_snapshot:
        if not snapshot or snapshot != head:
            raise SystemExit(
                f"NEP-330 source_code_snapshot {snapshot!r} does not equal git HEAD {head!r}"
            )

    provenance = {
        "wasm_bytes": len(wasm),
        "wasm_sha256": digest.hex(),
        "wasm_sha256_base58": b58encode(digest),
        "git_head": head,
        "rustc_vV": rustc_version(),
        "nep330": metadata,
        "note": (
            "base58 is raw SHA256, not base58check. "
            "This record does not by itself mean the wasm matches a deployed account."
        ),
    }
    text = json.dumps(provenance, indent=2) + "\n"
    if args.output:
        args.output.write_text(text)
    sys.stdout.write(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

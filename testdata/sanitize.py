#!/usr/bin/env python3
"""Sanitize a raw game-stream book for use as a public test fixture.

Books contain no player or team names, but their identifiers are stable
provider UUIDs and their timestamps are exact game times — pseudonymous,
not anonymous. This tool deterministically remaps every UUID-shaped
identifier (keyed HMAC, so referential links like beforeId/deleteIds
survive) and rebases every millisecond timestamp to a fictional epoch
(preserving relative deltas so duration/pace logic still exercises).

Usage:
    sanitize.py --key SECRET in.json out.json
    sanitize.py --key SECRET --verify in.json out.json   # also asserts no
                                                         # original id or
                                                         # name-bearing key
                                                         # survives
    sanitize.py --key SECRET --map in.json out.json      # print old->new ids

The key is NOT committed; fixtures are committed once, already sanitized.
"""

import argparse
import hashlib
import hmac
import json
import re
import sys

UUID_RE = re.compile(
    r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}"
)
# Millisecond epoch timestamps (2020s era: 13 digits starting with 1).
MS_TS_RE = re.compile(r"\b1[5-9]\d{11}\b")
FAKE_EPOCH_MS = 1600000000000
NAME_KEY_RE = re.compile(r'"[^"]*[nN]ame"\s*:')


def remap_uuid(key: bytes, original: str) -> str:
    digest = hmac.new(key, original.lower().encode(), hashlib.sha256).hexdigest()
    fake = f"{digest[0:8]}-{digest[8:12]}-{digest[12:16]}-{digest[16:20]}-{digest[20:32]}"
    # Preserve the original's case style so uppercase iOS-style ids keep
    # matching their uppercase references after remapping.
    return fake.upper() if original.isupper() else fake


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--key", required=True)
    ap.add_argument("--verify", action="store_true")
    ap.add_argument("--map", action="store_true", dest="print_map")
    ap.add_argument("infile")
    ap.add_argument("outfile")
    args = ap.parse_args()

    key = args.key.encode()
    text = open(args.infile, encoding="utf-8").read()

    mapping = {}

    def sub_uuid(m: re.Match) -> str:
        orig = m.group(0)
        canon = orig.lower()
        if canon not in mapping:
            mapping[canon] = remap_uuid(key, orig)
        out = mapping[canon]
        return out.upper() if orig.isupper() else out.lower()

    sanitized = UUID_RE.sub(sub_uuid, text)

    timestamps = [int(m.group(0)) for m in MS_TS_RE.finditer(sanitized)]
    if timestamps:
        shift = min(timestamps) - FAKE_EPOCH_MS
        sanitized = MS_TS_RE.sub(lambda m: str(int(m.group(0)) - shift), sanitized)

    json.loads(sanitized)  # must still be valid JSON

    with open(args.outfile, "w", encoding="utf-8") as f:
        f.write(sanitized)

    if args.verify:
        for orig in mapping:
            if orig in sanitized.lower():
                print(f"VERIFY FAILED: original id {orig} survives", file=sys.stderr)
                return 1
        if NAME_KEY_RE.search(sanitized):
            print("VERIFY FAILED: name-bearing key present", file=sys.stderr)
            return 1
        leftover = [
            int(m.group(0))
            for m in MS_TS_RE.finditer(sanitized)
            if int(m.group(0)) > FAKE_EPOCH_MS + 7 * 24 * 3600 * 1000
        ]
        if leftover:
            print("VERIFY FAILED: un-rebased timestamp present", file=sys.stderr)
            return 1
        print("verify ok", file=sys.stderr)

    if args.print_map:
        for orig, fake in sorted(mapping.items()):
            print(f"{orig} -> {fake}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""Assemble the update manifest the updater plugin reads.

The plugin looks for `{os}-{arch}-{installer}` before falling back to `{os}-{arch}`, so one
manifest can carry a different artefact for each package format. That is what lets someone who
installed the .deb be handed a .deb rather than an AppImage they cannot install.

Signatures are read from the .sig files the bundler produced, and the download URL is derived from
the asset each one signs. Nothing here invents a filename: if a package was not built and signed,
it simply does not appear, and the updater tells that platform there is nothing for it rather than
handing it the wrong file.
"""

import argparse
import datetime
import json
import pathlib
import sys

# Which manifest key each signed artefact belongs under. Remota ships Linux x86_64 only.
TARGETS = [
    (".AppImage.sig", "linux-x86_64-appimage"),
    (".deb.sig", "linux-x86_64-deb"),
    (".rpm.sig", "linux-x86_64-rpm"),
]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--tag", required=True)
    parser.add_argument("--repo", required=True)
    parser.add_argument("--sigs", required=True, type=pathlib.Path)
    parser.add_argument("--out", required=True, type=pathlib.Path)
    args = parser.parse_args()

    base = f"https://github.com/{args.repo}/releases/download/{args.tag}"
    platforms = {}

    for sig in sorted(args.sigs.iterdir()):
        for suffix, key in TARGETS:
            if sig.name.endswith(suffix):
                platforms[key] = {
                    "signature": sig.read_text().strip(),
                    # The signature file sits beside what it signs.
                    "url": f"{base}/{sig.name[: -len('.sig')]}",
                }
                break
        else:
            print(f"warning: no target for {sig.name}", file=sys.stderr)

    if not platforms:
        print("error: no signatures found; refusing to publish an empty manifest", file=sys.stderr)
        return 1

    manifest = {
        "version": args.tag.lstrip("v"),
        "pub_date": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "platforms": platforms,
    }
    args.out.write_text(json.dumps(manifest, indent=2) + "\n")
    print(json.dumps({k: v["url"] for k, v in platforms.items()}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

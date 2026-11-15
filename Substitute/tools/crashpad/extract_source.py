# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors
"""Extract the pinned Crashpad source archives into a fresh authenticated bundle root.

The bundle root holds `native-build/downloads/<package>.tar.gz`; extraction writes
`prepared/crashpad` (with its third-party mounts) and `prepared/buildtools`. Every
archive is checked against its pin first, and only regular members under each
archive's own prefix are written, exactly the files the source verifier expects.
"""
import argparse
import shutil
from pathlib import Path
from verify_source import PACKAGES, archive_files, digest, regular


def extract(bundle, packages=PACKAGES):
    """Create `prepared/` beside `native-build/` from the exact pinned archives."""
    regular(bundle)
    downloads = bundle / "native-build" / "downloads"
    prepared = bundle / "prepared"
    if prepared.exists():
        raise ValueError("The bundle already holds prepared sources; extract into a fresh bundle")
    for package, _, archive_hash, _, _ in packages:
        archive = downloads / (package + ".tar.gz")
        regular(archive)
        if digest(archive) != archive_hash:
            raise ValueError("Immutable archive identity mismatch")
    source = prepared / "crashpad"
    for package, revision, _, mount, prefix_name in packages:
        root = (source / mount) if mount else source
        prefix = prefix_name + "-" + revision + "/" if prefix_name else ""
        for name, stream in archive_files(downloads / (package + ".tar.gz"), prefix):
            target = root / name
            target.parent.mkdir(parents=True, exist_ok=True)
            with target.open("xb") as output:
                shutil.copyfileobj(stream, output)
    return source


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bundle-root", required=True, type=Path)
    print(extract(parser.parse_args().bundle_root))

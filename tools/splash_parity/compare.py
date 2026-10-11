# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors

"""Compare matching production renders and retain side-by-side and amplified differences."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path

import numpy as np
from PIL import Image, ImageChops, ImageDraw

try:
    from .acceptance import decide, measure, owner_bounds
except ImportError:
    from acceptance import decide, measure, owner_bounds


def compare(reference: Path, candidate: Path, output: Path) -> bool:
    """Measure every scenario and every visual owner without hiding raster differences."""
    scenarios = json.loads((reference / "scenarios.json").read_text(encoding="utf-8"))
    if not isinstance(scenarios, list) or not 1 <= len(scenarios) <= 1024:
        raise ValueError("Parity requires 1 through 1024 reference scenarios")
    names = [scenario["name"] for scenario in scenarios]
    if any(
        not isinstance(name, str)
        or not name
        or name in (".", "..")
        or any(
            not (character.isascii() and (character.isalnum() or character in "-_."))
            for character in name
        )
        for name in names
    ) or len(names) != len(set(names)):
        raise ValueError("Parity scenario names must be unique safe filenames")
    output.mkdir(parents=True, exist_ok=True)
    receipts = []
    for scenario in scenarios:
        name = scenario["name"]
        original_path = reference / f"{name}.png"
        replacement_path = candidate / f"{name}.png"
        with Image.open(original_path) as image:
            original = image.convert("RGBA")
        with Image.open(replacement_path) as image:
            replacement = image.convert("RGBA")
        if original.size != replacement.size:
            raise ValueError(f"Render dimensions disagree for {name}")
        if scenario["theme"] not in ("dark", "light") or type(scenario["expanded"]) is not bool:
            raise ValueError(f"Invalid appearance for {name}")
        scale = scenario["scale"]
        if type(scale) not in (int, float) or not np.isfinite(scale) or not 0 < scale <= 4096 / 558:
            raise ValueError(f"Invalid scale for {name}")
        expected_size = math.floor(558 * scale + 0.5)
        if original.size != (expected_size, expected_size):
            raise ValueError(f"Render dimensions do not match scenario scale for {name}")
        original_bytes = np.asarray(original, dtype=np.int32)
        replacement_bytes = np.asarray(replacement, dtype=np.int32)
        for pixels in (original_bytes, replacement_bytes):
            pixels[:, :, :3] = (pixels[:, :, :3] * pixels[:, :, 3:] + 127) // 255
        alpha_delta = np.abs(original_bytes - replacement_bytes)
        background = (
            (32, 32, 32, 255) if scenario["theme"] == "dark" else (243, 243, 243, 255)
        )
        left = Image.alpha_composite(
            Image.new("RGBA", original.size, background), original
        ).convert("RGB")
        right = Image.alpha_composite(
            Image.new("RGBA", replacement.size, background), replacement
        ).convert("RGB")
        delta = np.abs(
            np.asarray(left, dtype=np.int16) - np.asarray(right, dtype=np.int16)
        )
        owners = owner_bounds(scenario["expanded"])
        metrics = {}
        decisions = {}
        for owner, bounds in owners.items():
            x0, y0, x1, y1 = (math.floor(value * scenario["scale"] + 0.5) for value in bounds)
            region = delta[y0:y1, x0:x1]
            alpha_region = alpha_delta[y0:y1, x0:x1]
            metrics[owner] = measure(region, alpha_region)
            decisions[owner] = decide(owner, metrics[owner])
        pair = Image.new("RGB", (left.width * 2 + 16, left.height + 28), background[:3])
        pair.paste(left, (0, 28))
        pair.paste(right, (left.width + 16, 28))
        label = ImageDraw.Draw(pair)
        foreground = "white" if scenario["theme"] == "dark" else "black"
        label.text((8, 6), "Python / PySide reference", fill=foreground)
        label.text((left.width + 24, 6), "Rust native renderer", fill=foreground)
        pair.save(output / f"{name}-pair.png")
        ImageChops.difference(left, right).point(
            lambda value: min(255, value * 8)
        ).save(output / f"{name}-difference.png")
        Image.fromarray(np.minimum(255, alpha_delta.max(axis=2) * 8).astype(np.uint8)).save(
            output / f"{name}-premultiplied-difference.png"
        )
        overlay = Image.blend(left, right, 0.5)
        labels = ImageDraw.Draw(overlay)
        for owner, bounds in owners.items():
            if owner != "frame":
                scaled = [math.floor(value * scale + 0.5) for value in bounds]
                labels.rectangle(scaled, outline="cyan" if decisions[owner]["accepted"] else "red")
                labels.text((scaled[0] + 2, scaled[1] + 2), owner, fill=foreground)
        overlay.save(output / f"{name}-overlay.png")
        receipts.append({"name": name, "owners": metrics, "decisions": decisions,
                         "reference_sha256": hashlib.sha256(original_path.read_bytes()).hexdigest(),
                         "candidate_sha256": hashlib.sha256(replacement_path.read_bytes()).hexdigest(),
                         "accepted": all(decision["accepted"] for decision in decisions.values())})
    (output / "comparison.json").write_text(
        json.dumps(receipts, indent=2) + "\n", encoding="utf-8"
    )
    ranked = sorted(
        receipts,
        key=lambda receipt: receipt["owners"]["frame"]["mean_channel_error"],
        reverse=True,
    )
    accepted = all(receipt["accepted"] for receipt in receipts)
    summary = {"accepted": accepted, "scenarios": len(receipts),
               "failed_scenarios": sum(not receipt["accepted"] for receipt in receipts),
               "failed_owners": sorted({owner for receipt in receipts for owner, decision in receipt["decisions"].items() if not decision["accepted"]}),
               "worst": ranked[:3]}
    (output / "acceptance.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(summary, indent=2))
    return accepted


def main() -> int:
    """Require explicit artifact roots so captures cannot enter source control accidentally."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    arguments = parser.parse_args()
    return 0 if compare(arguments.reference, arguments.candidate, arguments.output) else 1


if __name__ == "__main__":
    raise SystemExit(main())

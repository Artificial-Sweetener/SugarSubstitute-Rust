# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Artificial Sweetener and contributors

"""Compare matching production renders and retain side-by-side and amplified differences."""

from __future__ import annotations

import argparse
import json
from pathlib import Path

import numpy as np
from PIL import Image, ImageChops, ImageDraw


def compare(reference: Path, candidate: Path, output: Path) -> None:
    """Measure every scenario and every visual owner without hiding raster differences."""
    scenarios = json.loads((reference / "scenarios.json").read_text(encoding="utf-8"))
    output.mkdir(parents=True, exist_ok=True)
    receipts = []
    for scenario in scenarios:
        name = scenario["name"]
        original = Image.open(reference / f"{name}.png").convert("RGBA")
        replacement = Image.open(candidate / f"{name}.png").convert("RGBA")
        if original.size != replacement.size:
            raise ValueError(f"Render dimensions disagree for {name}")
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
        owners = {
            "frame": (0, 0, 558, 558),
            "character": (83, 7, 470, 355)
            if scenario["expanded"]
            else (85, 86, 472, 472),
            "controls": (466, 0, 558, 32),
            "progress": (24, 356, 534, 362)
            if scenario["expanded"]
            else (24, 530, 534, 534),
        }
        if scenario["expanded"]:
            owners["console"] = (24, 362, 534, 534)
        else:
            owners["status"] = (24, 486, 534, 530)
        metrics = {}
        for owner, bounds in owners.items():
            x0, y0, x1, y1 = (round(value * scenario["scale"]) for value in bounds)
            region = delta[y0:y1, x0:x1]
            alpha_region = alpha_delta[y0:y1, x0:x1]
            metrics[owner] = {
                "mean_channel_error": float(region.mean()),
                "maximum_channel_error": int(region.max()),
                "different_pixels": int(np.any(region != 0, axis=2).sum()),
                "pixels": int(region.shape[0] * region.shape[1]),
                "mean_premultiplied_error": float(alpha_region.mean()),
                "maximum_premultiplied_error": int(alpha_region.max()),
                "different_premultiplied_pixels": int(
                    np.any(alpha_region != 0, axis=2).sum()
                ),
            }
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
        receipts.append({"name": name, "owners": metrics})
    (output / "comparison.json").write_text(
        json.dumps(receipts, indent=2) + "\n", encoding="utf-8"
    )
    ranked = sorted(
        receipts,
        key=lambda receipt: receipt["owners"]["frame"]["mean_channel_error"],
        reverse=True,
    )
    print(json.dumps({"scenarios": len(receipts), "worst": ranked[:3]}, indent=2))


def main() -> None:
    """Require explicit artifact roots so captures cannot enter source control accidentally."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    arguments = parser.parse_args()
    compare(arguments.reference, arguments.candidate, arguments.output)


if __name__ == "__main__":
    main()

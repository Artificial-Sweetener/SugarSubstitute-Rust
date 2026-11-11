#    SugarCubes - composable workflow units for ComfyUI
#    Copyright (C) 2026  Artificial Sweetener and contributors
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU Affero General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
#
#    This program is distributed in the hope that it will be useful,
#    but WITHOUT ANY WARRANTY; without even the implied warranty of
#    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
#    GNU Affero General Public License for more details.
#
#    You should have received a copy of the GNU Affero General Public License
#    along with this program.  If not, see <https://www.gnu.org/licenses/>.
"""Own Substitute detector matching and display labels without changing host values."""

from __future__ import annotations

from dataclasses import dataclass
import re

_DOWNLOAD_SIZE_SUFFIX = re.compile(r"\s+\([0-9.]+\s*MB\)\s*$", re.IGNORECASE)
_YOLO_TOKEN = re.compile(r"\byolo(?=v?\d)", re.IGNORECASE)

_CURATED_ASSETS = {
    "anzhc face -seg": "face-segmentation",
    "anzhc face seg 640 v2 y8n": "face-segmentation",
    "anzhc face seg 768 v2 y8n": "face-segmentation",
    "anzhc face seg 768ms v2 y8n": "face-segmentation",
    "anzhc face seg 1024 v2 y8n": "face-segmentation",
    "anzhc face seg 640 v3 y11n": "face-segmentation",
    "anzhc face seg 640 v4 y11n": "face-segmentation",
    "anzhcs manface v02 1024 y8n": "male-face-segmentation",
    "anzhcs womanface v05 1024 y8n": "female-face-segmentation",
    "anzhc eyes -seg-hd": "eyes-segmentation",
    "anzhc headhair seg y8n": "hair-segmentation",
    "anzhc headhair seg y8m": "hair-segmentation",
    "anzhc breasts seg v1 1024n": "breast-segmentation",
    "anzhc breasts seg v1 1024s": "breast-segmentation",
    "anzhc breasts seg v1 1024m": "breast-segmentation",
    "bingsu face yolov8n v2": "face-detection",
    "bingsu face yolov8s": "face-detection",
    "bingsu hand yolov8n": "hand-detection",
    "bingsu hand yolov8s": "hand-detection",
    "bingsu person yolov8n-seg": "person-segmentation",
    "bingsu person yolov8s-seg": "person-segmentation",
    "fuyucchi yolov8x6 anime face": "anime-face-detection",
}
_ASSET_NAMES = frozenset(
    {
        "anime-face-detection",
        "anime-face-segmentation",
        "breast-detection",
        "breast-segmentation",
        "eyes-detection",
        "eyes-segmentation",
        "face-detection",
        "face-segmentation",
        "female-face-detection",
        "female-face-segmentation",
        "hair-detection",
        "hair-segmentation",
        "hand-detection",
        "hand-segmentation",
        "male-face-detection",
        "male-face-segmentation",
        "person-segmentation",
    }
)


def detector_asset(value: str) -> str | None:
    """Resolve a curated label or conventional detector filename to one visual."""

    normalized = _normalized_value(value)
    curated = _CURATED_ASSETS.get(normalized)
    if curated is not None:
        return curated

    is_segmentation = _is_segmentation_value(normalized)
    suffix = "segmentation" if is_segmentation else "detection"
    if "anime" in normalized and "face" in normalized:
        subject = "anime-face"
    elif "womanface" in normalized or "female" in normalized:
        subject = "female-face"
    elif "manface" in normalized or "male" in normalized:
        subject = "male-face"
    elif "headhair" in normalized or "head+hair" in normalized or "hair" in normalized:
        subject = "hair"
    elif "eyes" in normalized or "eye" in normalized:
        subject = "eyes"
    elif "breast" in normalized:
        subject = "breast"
    elif "hand" in normalized:
        subject = "hand"
    elif "person" in normalized:
        subject = "person"
    elif "face" in normalized:
        subject = "face"
    else:
        return None

    asset_name = f"{subject}-{suffix}"
    if asset_name in _ASSET_NAMES:
        return asset_name
    if subject == "person":
        return "person-segmentation"
    return None


def _normalized_value(value: str) -> str:
    """Return a stable matching key without a display-only download size."""

    normalized = value.strip().replace("\\", "/").casefold()
    return _DOWNLOAD_SIZE_SUFFIX.sub("", normalized)


def _is_segmentation_value(normalized: str) -> bool:
    """Return whether a choice names or resides in a segmentation model route."""

    return (
        normalized.startswith("segm/")
        or "/segm/" in normalized
        or "segment" in normalized
        or "-seg" in normalized
        or "_seg" in normalized
        or " seg" in normalized
    )


def detector_title(value: str) -> str:
    """Return a path-free title while preserving the model author's name."""

    normalized = value.strip().replace("\\", "/")
    basename = normalized.rsplit("/", 1)[-1]
    for suffix in (".safetensors", ".ckpt", ".pt"):
        if basename.casefold().endswith(suffix):
            basename = basename[: -len(suffix)]
            break
    title = _DOWNLOAD_SIZE_SUFFIX.sub("", basename).strip()
    title = " ".join(title.replace("_", " ").split())
    title = _YOLO_TOKEN.sub("YOLO", title)
    if title and title[0].islower():
        title = title[0].upper() + title[1:]
    return title or value


def _asset_title(asset_name: str) -> str:
    """Return a concise human title for one packaged detector visual."""

    subject = asset_name.rsplit("-", 1)[0]
    words = subject.replace("-", " ").split()
    return " ".join(word.capitalize() for word in words)


def _asset_sort_key(asset_name: str) -> tuple[str, int]:
    """Group matching detection and segmentation visuals together."""

    subject, mode = asset_name.rsplit("-", 1)
    return subject, 0 if mode == "detection" else 1


@dataclass(frozen=True, slots=True)
class DetectorIllustration:
    """Describe one curated detector-output illustration."""

    asset_name: str
    title: str
    mode: str


def detector_library() -> tuple[DetectorIllustration, ...]:
    """Return every reference illustration in paired detection/segmentation order."""
    return tuple(
        DetectorIllustration(
            name,
            _asset_title(name),
            "Segmentation" if name.endswith("-segmentation") else "Bounding Box",
        )
        for name in sorted(_ASSET_NAMES, key=_asset_sort_key)
    )

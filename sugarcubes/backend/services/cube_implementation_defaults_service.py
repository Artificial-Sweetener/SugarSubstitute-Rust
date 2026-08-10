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

"""Own implementation-save catalog preservation and authored-default decisions."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping

from ...cube_model import (
    CubeDocument,
    CubeSchemaError,
    build_default_change_plan,
    build_implementation_change_plan,
    merge_implementation_save_defaults,
    sanitize_authored_defaults_document,
)
from ...exporter import ExportedCube
from ..responses import BackendError
from .cube_metadata import normalize_metadata_string


@dataclass(frozen=True)
class DefaultSaveDecision:
    """Carry the two aggregate default choices for one implementation save."""

    fingerprint: str = ""
    overwrite_defaults: bool = False
    save_prompt_fields: bool = False

    @classmethod
    def from_entry(cls, entry: Mapping[str, Any]) -> DefaultSaveDecision:
        """Narrow a dynamic cube-entry review payload."""

        review = entry.get("default_review")
        if review is None:
            return cls()
        if not isinstance(review, Mapping):
            raise BackendError("Cube default review must be an object", status=400)
        fingerprint = normalize_metadata_string(review.get("fingerprint"))
        overwrite_defaults = review.get("overwrite_defaults", False)
        save_prompt_fields = review.get("save_prompt_fields", False)
        if not isinstance(overwrite_defaults, bool) or not isinstance(
            save_prompt_fields, bool
        ):
            raise BackendError("Cube default choices must be booleans", status=400)
        return cls(
            fingerprint=fingerprint,
            overwrite_defaults=overwrite_defaults,
            save_prompt_fields=save_prompt_fields,
        )


class CubeImplementationDefaultsService:
    """Plan and apply default reconciliation for one implementation candidate."""

    def review(
        self,
        *,
        existing_payload: Mapping[str, Any] | None,
        exported: ExportedCube,
        preserve_description: bool,
    ) -> dict[str, Any]:
        """Return the authoritative review plan without mutating the candidate."""

        existing, candidate = self._documents(
            existing_payload=existing_payload,
            exported=exported,
            preserve_description=preserve_description,
        )
        return build_implementation_change_plan(existing, candidate).to_dict()

    def apply(
        self,
        *,
        existing_payload: Mapping[str, Any] | None,
        exported: ExportedCube,
        preserve_description: bool,
        decision: DefaultSaveDecision,
    ) -> None:
        """Mutate one export with the safe, fingerprint-validated default merge."""

        existing, candidate = self._documents(
            existing_payload=existing_payload,
            exported=exported,
            preserve_description=preserve_description,
        )
        try:
            plan = build_default_change_plan(existing, candidate)
            overwrite_control_ids = frozenset(
                change.control_id
                for change in plan.changes
                if (decision.save_prompt_fields if change.is_multiline else decision.overwrite_defaults)
            )
            merged = merge_implementation_save_defaults(
                existing,
                candidate,
                overwrite_control_ids=overwrite_control_ids,
                expected_fingerprint=decision.fingerprint,
            )
            sanitized = sanitize_authored_defaults_document(merged)
        except (CubeSchemaError, ValueError) as exc:
            raise BackendError(str(exc), status=409) from exc
        exported.cube.clear()
        exported.cube.update(sanitized.to_dict())

    def _documents(
        self,
        *,
        existing_payload: Mapping[str, Any] | None,
        exported: ExportedCube,
        preserve_description: bool,
    ) -> tuple[CubeDocument | None, CubeDocument]:
        """Validate documents and preserve catalog fields before policy evaluation."""

        try:
            existing = (
                CubeDocument.from_dict(existing_payload)
                if existing_payload is not None
                else None
            )
            candidate = CubeDocument.from_dict(exported.cube)
            if existing is not None:
                candidate = self._preserve_catalog_metadata(
                    existing=existing,
                    candidate=candidate,
                    exported_default_alias=exported.default_alias,
                    preserve_description=preserve_description,
                )
            return existing, candidate
        except CubeSchemaError as exc:
            raise BackendError(str(exc), status=400) from exc

    def _preserve_catalog_metadata(
        self,
        *,
        existing: CubeDocument,
        candidate: CubeDocument,
        exported_default_alias: str,
        preserve_description: bool,
    ) -> CubeDocument:
        """Preserve catalog fields omitted by implementation export."""

        payload = candidate.to_dict()
        existing_description = normalize_metadata_string(existing.description)
        candidate_description = normalize_metadata_string(candidate.description)
        if (
            preserve_description
            and existing_description
            and candidate_description.startswith("Auto-converted cube for ")
        ):
            payload["description"] = existing_description

        metadata = {**existing.metadata, **candidate.metadata}
        default_alias = normalize_metadata_string(metadata.get("default_alias"))
        normalized_alias = normalize_metadata_string(exported_default_alias)
        if not default_alias and normalized_alias:
            metadata["default_alias"] = normalized_alias
        metadata.pop("author", None)
        if metadata:
            payload["metadata"] = metadata
        return CubeDocument.from_dict(payload)

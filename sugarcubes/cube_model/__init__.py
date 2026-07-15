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
"""Canonical SugarCube document model and helpers."""

from .cube_identity import (
    CanonicalCubeId,
    CubeIdentityError,
    RESERVED_SOURCE_NAMES,
    build_canonical_cube_id,
    derive_cube_id_from_default_alias,
    derive_source_author_label,
    is_canonical_cube_id,
    parse_canonical_cube_id,
    suggest_canonical_cube_path,
)
from .authored_default_policy import (
    sanitize_authored_defaults_document,
    sanitize_authored_defaults_payload,
    should_strip_authored_default,
)
from .document import (
    CubeDocument,
    CubeSchemaError,
    looks_like_current_cube_payload,
    looks_like_legacy_cube_payload,
)
from .identity_projection import (
    apply_cube_identity_projection,
    build_cube_definition_key,
    iter_cube_identity_projection_violations,
)
from .flavors import (
    AuthoredFlavor,
    AuthoredFlavorSet,
    dedupe_flavor_id,
    normalize_flavor_id,
)
from .flavor_merge import preserve_authored_flavors_for_implementation_save
from .implementation import CubeImplementation
from .migrate import migrate_legacy_payload
from .model_targets import (
    ANY_TARGET_MODEL,
    derive_cube_id_from_route,
    derive_filename_from_route,
    derive_route_from_cube_id,
    derive_target_model_cube_id,
    derive_target_model_from_cube_id,
    derive_target_model_from_route,
    normalize_cube_route,
    normalize_supported_models,
    normalize_target_model,
    validate_cube_route_identity,
)
from .picker_fields import (
    PickerFallback,
    compact_picker_field_spec,
    find_input_field_spec,
    is_picker_field_spec,
    picker_options,
    resolve_picker_fallback,
    widget_input_names,
)
from .surface import (
    CubeSurface,
    SurfaceControl,
    compute_surface_signature,
    infer_value_type,
)

__all__ = [
    "AuthoredFlavor",
    "AuthoredFlavorSet",
    "ANY_TARGET_MODEL",
    "CanonicalCubeId",
    "CubeDocument",
    "CubeIdentityError",
    "RESERVED_SOURCE_NAMES",
    "CubeImplementation",
    "CubeSchemaError",
    "CubeSurface",
    "PickerFallback",
    "SurfaceControl",
    "apply_cube_identity_projection",
    "build_canonical_cube_id",
    "build_cube_definition_key",
    "compute_surface_signature",
    "compact_picker_field_spec",
    "dedupe_flavor_id",
    "derive_cube_id_from_default_alias",
    "derive_cube_id_from_route",
    "derive_filename_from_route",
    "derive_route_from_cube_id",
    "derive_source_author_label",
    "derive_target_model_cube_id",
    "derive_target_model_from_cube_id",
    "derive_target_model_from_route",
    "infer_value_type",
    "find_input_field_spec",
    "iter_cube_identity_projection_violations",
    "is_picker_field_spec",
    "is_canonical_cube_id",
    "looks_like_current_cube_payload",
    "looks_like_legacy_cube_payload",
    "migrate_legacy_payload",
    "normalize_cube_route",
    "normalize_flavor_id",
    "normalize_supported_models",
    "normalize_target_model",
    "parse_canonical_cube_id",
    "picker_options",
    "preserve_authored_flavors_for_implementation_save",
    "resolve_picker_fallback",
    "sanitize_authored_defaults_document",
    "sanitize_authored_defaults_payload",
    "should_strip_authored_default",
    "suggest_canonical_cube_path",
    "validate_cube_route_identity",
    "widget_input_names",
]

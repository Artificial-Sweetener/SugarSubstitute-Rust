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
"""Safeguard the versioned shared model authority's advertised public operations."""

from sugarcubes.model_library import host_api


def test_version_one_exports_cover_provider_policy_and_cache_maintenance() -> None:
    """Expose supported host operations without publishing registration-only lifetime controls."""
    assert host_api.MODEL_LIBRARY_HOST_API_VERSION == 1
    required = {
        "MODEL_LIBRARY_HOST_API_VERSION",
        "active_model_library",
        "prepared_model_catalog",
        "model_catalog_revision",
        "prepared_model_asset",
        "refresh_model_catalog",
        "queue_model_fingerprint",
        "active_model_fingerprint",
        "model_job",
        "cancel_model_job",
        "queue_model_acquisition",
        "model_release_history",
        "prepared_model_updates",
        "set_model_update_notifications",
        "check_model_updates",
        "dismiss_model_update",
        "disable_model_updates",
        "prepared_model_provider_preferences",
        "configure_model_provider_preferences",
        "prepared_civitai_cache_summary",
        "queue_civitai_cache_action",
    }
    assert set(host_api.__all__) == required
    assert len(host_api.__all__) == len(required)
    for name in required - {"MODEL_LIBRARY_HOST_API_VERSION"}:
        assert callable(getattr(host_api, name))

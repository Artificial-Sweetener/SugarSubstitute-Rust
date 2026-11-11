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
"""Compose model-library adapters and isolate blocking filesystem work from HTTP."""

from __future__ import annotations

import asyncio
from collections.abc import Sequence
from pathlib import Path

from ..application.catalog_service import ModelCatalogService
from ..application.architecture_service import ModelArchitectureService
from .architecture_store import SqliteModelArchitectures
from .comfy_architecture_detector import ComfyArchitectureDetector
from .model_architecture_reader import ApprovedModelArchitecture
from ..application.ports import ModelRoots
from .catalog_store import SqliteModelCatalogRepository
from .jobs import ModelBackgroundJobs, ModelJob
from .thumbnail_assets import ModelThumbnailAssets
from .detector_assets import BundledDetectorAssets
from ..application.detector_service import ModelDetectorService
from .thumbnail_preparer import ModelThumbnailPreparer
from .local_presentation import LocalModelPresentation
from ..application.enrichment_service import (
    ModelEnrichmentService,
    ModelProvider,
    ModelPreviewPreparer,
)
from ..application.automatic_enrichment import AutomaticModelEnrichment
from .enrichment_attempts import SqliteEnrichmentAttempts
from .enrichment_worker import ModelEnrichmentWorker
from .fingerprints import ApprovedModelFingerprints
from .civitai_client import CivitaiModelProvider
from .provider_previews import ModelProviderPreviewPreparer
from .openmodeldb_client import OpenModelDbProvider
from .provider_catalog_cache import ProviderCatalogCache
from ..application.provider_registry import (
    ModelMetadataProviders,
    RegisteredModelProvider,
)
from .catalog_updates import ModelCatalogUpdates
from ..application.thumbnail_choice_service import ModelThumbnailChoiceService
from .provider_credentials import civitai_api_key
from ..application.download_authorization import ModelDownloadAuthorization
from .civitai_download_access import CivitaiDownloadAccess
from .artifact_downloads import ModelArtifactDownloads
from ..application.release_service import ModelReleaseService
from .catalog_release_history import CatalogReleaseHistory
from .enrichment_update_check import ModelEnrichmentUpdateCheck
from ..application.model_update_service import ModelUpdateService
from ..application.model_update_feed import ModelUpdateFeed
from ..application.release_previews import ModelReleasePreviews
from ..application.queued_model_usage import QueuedModelUsageRecorder
from .model_usage_store import SqliteModelUsageRepository
from ..application.fingerprint_service import ModelFingerprintService
from ..domain.models import ModelCatalogEntry
from .folder_observations import ModelFolderObservations
from .catalog_watcher import ModelCatalogWatcher
from .desktop_acquisitions import DesktopModelAcquisitions
from .artifact_publication import ModelArtifactPublication
from ..application.requested_artifacts import RequestedModelArtifacts
from ..application.openmodeldb_suggestions import OpenModelDbSuggestions
from ..application.model_suggestion_engine import ModelSuggestionEngine
from ..application.model_suggestion_service import ModelSuggestionService
from ..application.civitai_suggestions import CivitaiSuggestions
from .civitai_family_discovery import CivitaiFamilyDiscovery
from .civitai_upscaler_discovery import CivitaiUpscalerDiscovery
from .civitai_transport import CivitaiTransport
from ..application.credential_service import (
    CivitaiCredentialService,
    CivitaiCredentialStore,
)
from .civitai_credential_store import UnavailableCivitaiCredentialStore
from .civitai_credential_validator import CivitaiApiKeyValidator
from .discovery_plans import ReviewedDiscoveryPlans
from .discovery_requests import ModelDiscoveryRequests
from ..application.provider_preferences import ModelProviderPreferenceService
from .provider_preference_store import SqliteModelProviderPreferences
from ..application.enrichment_freshness import ModelEnrichmentFreshness
from .enrichment_cache import SqliteModelEnrichmentCache
from .stored_thumbnail_ownership import StoredThumbnailOwnership
from ..application.metadata_activity import ModelMetadataActivity
from ..application.coordinated_previews import CoordinatedModelPreviews
from ..application.provider_cache_service import ModelProviderCacheService
from .provider_cache_store import CivitaiCacheStorage
from .provider_cache_jobs import ModelProviderCacheJobs
from .relationship_catalog import model_relationships
from .content_override_store import SqliteModelContentOverrides
from ..application.content_classification import ModelContentClassification
from ..application.organized_downloads import OrganizedModelDownloads
from .download_preference_store import SqliteDownloadPreferences


class ModelLibraryRuntime:
    """Own one shared catalog, asset collection, and job scheduler for a Comfy host."""

    def __init__(
        self,
        roots: ModelRoots,
        *,
        enrichment_provider: ModelProvider | None = None,
        enrichment_previews: ModelPreviewPreparer | None = None,
        credential_store: CivitaiCredentialStore | None = None,
    ) -> None:
        """Compose storage beneath the active configured model folder."""
        self.repository = SqliteModelCatalogRepository(Path(roots.storage_root()))
        self.metadata_activity = ModelMetadataActivity()
        self.preferences = ModelProviderPreferenceService(
            SqliteModelProviderPreferences(
                self.repository.asset_root.parent / "provider-preferences.sqlite3"
            )
        )
        self.relationships = model_relationships()
        self.content_overrides = SqliteModelContentOverrides(self.repository.database)
        self.architectures = ModelArchitectureService(
            self.repository,
            SqliteModelArchitectures(self.repository.database),
            ApprovedModelArchitecture(roots, ComfyArchitectureDetector()),
        )
        self.assets = ModelThumbnailAssets(self.repository.asset_root)
        self.assets.retain_sources(
            StoredThumbnailOwnership(self.repository.database).read()
        )
        self.detectors = ModelDetectorService(
            self.repository, BundledDetectorAssets(self.assets)
        )
        self.thumbnail_choices = ModelThumbnailChoiceService(
            self.repository, ModelThumbnailPreparer(self.assets, origin="user")
        )
        self.local_presentation = LocalModelPresentation(
            roots, self.repository, ModelThumbnailPreparer(self.assets, origin="local")
        )
        self.jobs = ModelBackgroundJobs()
        self.updates = ModelCatalogUpdates(self.repository)
        self.fingerprints = ModelFingerprintService(
            self.repository, ApprovedModelFingerprints(roots)
        )
        self.content_classification = ModelContentClassification(
            self.repository, self.fingerprints, self.content_overrides
        )
        secure_store = credential_store or UnavailableCivitaiCredentialStore()

        def credential_source() -> str | None:
            """Resolve credentials through one precedence owner for all provider operations."""
            return civitai_api_key(secure_store.load_api_key)

        civitai_transport = CivitaiTransport(credential_source)
        download_access = CivitaiDownloadAccess(civitai_transport)
        self.credentials = CivitaiCredentialService(
            secure_store, credential_source, CivitaiApiKeyValidator(civitai_transport)
        )
        provider = CivitaiModelProvider(
            activity=self.metadata_activity,
            transport=civitai_transport,
            download_access=download_access,
            thumbnail_policy=self.preferences.image_policy,
            lookup_enabled=self.preferences.metadata_lookup_enabled,
            requested_lookup_enabled=self.preferences.missing_model_lookup_enabled,
        )
        self.download_preferences = SqliteDownloadPreferences(self.repository.database)
        installer = OrganizedModelDownloads(
            ModelArtifactDownloads(
                roots,
                credential_source,
                ModelDownloadAuthorization(download_access, credential_source),
            ),
            self.download_preferences,
            provider,
            self.relationships,
            self.content_overrides.read,
        )
        usage = SqliteModelUsageRepository(
            self.repository.asset_root.parent / "model-usage.sqlite3"
        )
        release_history = CatalogReleaseHistory(
            self.repository.asset_root.parent / "release-history.sqlite3",
            self.repository.database,
        )
        self.releases = ModelReleaseService(
            self.repository,
            provider,
            installer,
            usage,
            release_history,
            lambda: self.preferences.image_policy().value,
            self.metadata_activity,
        )
        self.catalog = ModelCatalogService(
            roots,
            self.repository,
            self.relationships,
            self.releases.cached_version_count,
        )
        self.model_updates = ModelUpdateService(usage, self.repository, self.releases)
        self.queued_model_usage = QueuedModelUsageRecorder(
            self.repository, self.model_updates
        )
        self.update_feed = ModelUpdateFeed(self.model_updates)
        self.artifact_publication = ModelArtifactPublication(
            self.catalog, self.local_presentation, self.updates, self.architectures
        )
        self.desktop_acquisitions = DesktopModelAcquisitions(
            RequestedModelArtifacts(provider),
            installer,
            roots,
            self.artifact_publication,
            self.jobs,
            self.preferences.require_missing_model_downloads,
        )
        openmodeldb = OpenModelDbProvider(
            ProviderCatalogCache(
                self.repository.asset_root.parent / "providers" / "openmodeldb.json"
            )
        )
        self.model_suggestions = ModelSuggestionService(
            self.repository,
            ModelSuggestionEngine(
                (
                    OpenModelDbSuggestions(openmodeldb, installer),
                    CivitaiSuggestions(
                        CivitaiFamilyDiscovery(
                            civitai_transport,
                            self.relationships,
                            download_access=download_access,
                            thumbnail_policy=self.preferences.image_policy,
                        ),
                        CivitaiUpscalerDiscovery(
                            openmodeldb,
                            provider,
                            civitai_transport,
                            download_access=download_access,
                            thumbnail_policy=self.preferences.image_policy,
                        ),
                        installer,
                        self.relationships,
                    ),
                )
            ),
        )
        raw_previews = enrichment_previews or ModelProviderPreviewPreparer(
            provider,
            openmodeldb,
            ModelThumbnailPreparer(self.assets, origin="civitai"),
            ModelThumbnailPreparer(self.assets, origin="openmodeldb"),
        )
        provider_previews = CoordinatedModelPreviews(
            self.metadata_activity, raw_previews
        )
        self.release_previews = ModelReleasePreviews(
            self.releases, provider_previews, self.metadata_activity
        )
        self.discovery = ModelDiscoveryRequests(
            self.model_suggestions,
            provider_previews,
            self.artifact_publication,
            self.jobs,
            ReviewedDiscoveryPlans(),
        )
        self.enrichment = ModelEnrichmentService(
            self.repository,
            self.fingerprints,
            enrichment_provider
            or ModelMetadataProviders(
                (
                    RegisteredModelProvider(frozenset({"upscale_models"}), openmodeldb),
                    RegisteredModelProvider(frozenset(), provider),
                )
            ),
            provider_previews,
            activity=self.metadata_activity,
            after_refresh=ModelEnrichmentUpdateCheck(self.update_feed).check,
        )
        enrichment_attempts = SqliteEnrichmentAttempts(
            self.repository.asset_root.parent / "enrichment.sqlite3"
        )
        enrichment_decisions = SqliteModelEnrichmentCache(
            self.repository.asset_root.parent / "enrichment-cache.sqlite3"
        )
        self.automatic_enrichment = ModelEnrichmentWorker(
            self.repository,
            AutomaticModelEnrichment(
                self.repository,
                self.enrichment,
                enrichment_attempts,
                ModelEnrichmentFreshness(
                    enrichment_decisions,
                    provider_previews,
                    self.preferences.enrichment_policy,
                    self.preferences.enrichment_provider_scope,
                    self.preferences.enrichment_providers,
                ),
                enabled=self.preferences.enrichment_enabled,
            ),
            self.jobs,
            self.updates,
        )
        self.civitai_cache = ModelProviderCacheService(
            CivitaiCacheStorage(
                self.repository.provider_cache,
                enrichment_decisions,
                enrichment_attempts,
                self.assets,
                StoredThumbnailOwnership(self.repository.database),
                release_history,
            ),
            self.metadata_activity,
            self.automatic_enrichment,
            self.updates,
            provider.invalidate_missing,
            provider="civitai",
        )
        self.cache_actions = ModelProviderCacheJobs(self.civitai_cache, self.jobs)
        self._initial_scan: ModelJob | None = None
        self.watcher = ModelCatalogWatcher(
            ModelFolderObservations(roots), self.repository, self.jobs, self.refresh
        )

    async def list_models(self, kinds: Sequence[str] | None) -> dict[str, object]:
        """Return prepared records immediately and bootstrap a cold index in the background."""
        result = await asyncio.to_thread(self.catalog.list_models, kinds)
        result["scan"] = self.start_initial_scan().to_payload()
        result["enrichment"] = self.automatic_enrichment.status()
        return result

    def start_initial_scan(self) -> ModelJob:
        """Warm the shared index once when Comfy starts, independent of which frontend connects."""
        if self._initial_scan is None:
            self._initial_scan = self.refresh(None)
        return self._initial_scan

    def refresh(self, kinds: Sequence[str] | None) -> ModelJob:
        """Queue stat evidence and changed header inspections off the server event loop."""
        identity = "catalog-scan:" + (
            ",".join(sorted(kinds)) if kinds is not None else "all"
        )

        async def operation() -> dict[str, object]:
            """Move synchronous registry and index work off the event-loop thread."""
            result = await asyncio.to_thread(self.catalog.refresh, kinds)
            result["architectureUpdates"] = await asyncio.to_thread(
                self.architectures.refresh, kinds
            )
            result["localPresentationUpdates"] = await asyncio.to_thread(
                self.local_presentation.refresh, kinds
            )
            await self.updates.publish()
            await self.automatic_enrichment.refill()
            self.automatic_enrichment.start()
            return result

        return self.jobs.start(identity, operation)

    async def refresh_metadata(self, model_id: str) -> ModelJob:
        """Queue explicit provider enrichment for one approved catalog identity."""
        return await self.automatic_enrichment.refresh(model_id)

    async def prioritize_metadata(self, model_ids: Sequence[str]) -> None:
        """Validate approved catalog identities before scheduling visible choices."""
        for identity in model_ids:
            if await asyncio.to_thread(self.repository.find_entry, identity) is None:
                raise ValueError("The requested model was not found in the catalog.")
        self.automatic_enrichment.prioritize(model_ids)

    async def refresh_fingerprint(self, model_id: str) -> ModelJob:
        """Expose explicit fingerprint work without making a remote provider request."""
        entry = await asyncio.to_thread(self.repository.find_entry, model_id)
        if entry is None:
            raise ValueError("The requested model was not found in the catalog.")
        return self._queue_fingerprint(entry)

    def queue_fingerprint(self, model_id: str) -> ModelJob:
        """Accept a sibling extension's indexed identity on the running host event loop."""
        entry = self.repository.find_entry(model_id)
        if entry is None:
            raise ValueError("The requested model was not found in the catalog.")
        return self._queue_fingerprint(entry)

    def _queue_fingerprint(self, entry: ModelCatalogEntry) -> ModelJob:
        """Share execution and observable state across browser and desktop consumers."""
        model_id = entry.evidence.location_id

        async def operation() -> dict[str, object]:
            """Publish only a digest backed by unchanged approved file evidence."""
            digest = await self.fingerprints.refresh(entry)
            await self.updates.publish()
            return {"modelId": model_id, "sha256": digest}

        return self.jobs.start(f"fingerprint-refresh:{model_id}", operation)

    async def replace_thumbnail(self, model_id: str, content: bytes) -> None:
        """Prepare an explicit image off the event loop and publish its committed revision."""
        await asyncio.to_thread(self.thumbnail_choices.replace, model_id, content)
        await self.updates.publish()

    def acquire_release(self, model_id: str, version_id: str, file_id: str) -> ModelJob:
        """Queue one deliberate verified acquisition and publish its resulting catalog revision."""

        async def operation() -> dict[str, object]:
            """Install the selected artifact and refresh host-visible file evidence."""
            artifact = await self.releases.acquire(
                model_id,
                version_id,
                file_id,
                lambda downloaded, total: self.jobs.transfer_progress(
                    job.job_id, downloaded, total
                ),
            )
            self.jobs.commit(job.job_id)
            return await self.artifact_publication.publish(artifact)

        job = self.jobs.start(
            f"model-acquire:{model_id}:{version_id}:{file_id}", operation
        )
        return job

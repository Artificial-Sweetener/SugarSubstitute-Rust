//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
/** Model Sources: CivitAI account, lookup, thumbnails, provider cache and the model library. */
import { row } from '../SettingsLayoutRows.js';
/** The page's place in the navigation, its text, and its sections' rows. */
export const modelSourcesPage = {
    id: 'model_sources',
    title: 'Model Sources',
    subtitle: 'External providers, credentials, safety, and caches.',
    icon: 'globe-desktop',
    searchable: true,
    sections: [
        {
            id: 'civitai_account',
            title: 'CivitAI account',
            rows: [
                row('model_sources.civitai_account.key', 'API key', 'The key used for authenticated CivitAI lookups and downloads.', 'key', 780, ['civitai', 'token']),
            ],
        },
        {
            id: 'civitai_lookup',
            title: 'CivitAI lookup',
            rows: [
                row('model_sources.civitai_lookup.local_metadata', 'Look up local model metadata', 'Query CivitAI for hashes in the local model cache.', 'database-search'),
            ],
        },
        {
            id: 'safety',
            title: 'Thumbnails and safety',
            rows: [
                row('model_sources.safety.civitai_thumbnails', 'Download CivitAI thumbnails', 'Download provider images for picker thumbnails.', 'image-multiple'),
                row('model_sources.safety.civitai_thumbnail_policy', 'Thumbnail safety', 'Choose which CivitAI thumbnails may be shown.', 'shield-checkmark', 640, ['nsfw', 'sfw']),
            ],
        },
        {
            id: 'provider_cache',
            title: 'Provider cache',
            rows: [
                row('model_sources.provider_cache.civitai_usage', 'CivitAI cache usage', 'Cached provider metadata and thumbnails.', 'database-search', 680),
                row('model_sources.provider_cache.civitai_maintenance', 'CivitAI cache maintenance', 'Clear cached thumbnails or metadata, or refresh the summary.', 'broom', 720),
            ],
        },
        {
            id: 'model_library',
            title: 'Model library',
            rows: [
                row('model_sources.model_library.enhanced_picker', 'Enhanced model pickers', 'Browse models with searchable image-backed fields in Nodes 2.0.', 'box-search'),
                row('model_sources.model_library.catalog', 'Model catalog', 'Scan the model folders again for added or removed models.', 'arrow-sync'),
            ],
        },
    ],
};

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
/** Generation: previews, output files, missing models, model downloads, the output canvas and memory. */
import { row } from '../SettingsLayoutRows.js';
/** The page's place in the navigation, its text, and its sections' rows. */
export const generationPage = {
    id: 'generation',
    title: 'Generation',
    subtitle: 'Generation behavior and generated files.',
    icon: 'image-sparkle',
    searchable: true,
    sections: [
        {
            id: 'preview',
            title: 'Preview',
            rows: [
                row('generation.preview.configuration', 'Generation previews', 'Show sampler preview frames while ComfyUI is generating.', 'image-sparkle', undefined, ['latent', 'taesd', 'sampler']),
            ],
        },
        {
            id: 'output',
            title: 'Output files',
            rows: [
                row('generation.output.folder', 'Output folder', 'Where generated images are saved.', 'folder-open', 720),
                row('generation.output.pattern', 'Output pattern', 'Relative folders and filename, without .png.', 'document-text', 640, ['filename', 'tokens']),
                row('generation.output.preview', 'Output preview', 'Example path for the current settings.', 'save-image', 680),
                row('generation.output.persistence', 'Saved cube outputs', 'Save every cube output or only the final cube.', 'save-image', 640),
                row('generation.output.jpeg', 'JPEG companions', 'Save a JPEG beside each recipe PNG.', 'save-image', undefined, ['quality', 'size']),
                row('generation.output.transfer', 'Drag and Copy format', 'Use the companion JPEG for drag and Copy.', 'save-image'),
                row('generation.output.workflow', 'Manage workflow saving', 'Apply these output settings to the save nodes of every workflow.', 'save-image'),
                row('generation.output.previews', 'Save completed Preview Image outputs', 'While workflow saving is managed, also save what completed Preview Image nodes show.', 'save-image'),
                row('generation.output.sources', 'Workflow outputs', 'Choose which outputs each open workflow saves.', 'save-image'),
                row('generation.output.import', 'Import Substitute settings', 'Use the output settings from a SugarSubstitute settings file.', 'arrow-download'),
            ],
        },
        {
            id: 'missing_models',
            title: 'Missing model handling',
            rows: [
                row('generation.missing_models.lookup', 'Look up missing recipe models', 'Use CivitAI after local recipe model matching fails.', 'box-search'),
                row('generation.missing_models.downloads', 'Offer verified model downloads', 'Let missing-model resolution offer CivitAI downloads.', 'arrow-download'),
                row('generation.missing_models.update_notices', 'Notify me about updates to models I use', 'Show compatible updates checked during metadata lookup, daily model selection, and version browsing.', 'book-arrow-clockwise'),
            ],
        },
        {
            id: 'model_downloads',
            title: 'Model downloads',
            rows: [
                row('generation.model_downloads.pattern', 'Model folder pattern', 'Organize downloaded models inside the matching Comfy model folder. Use {architecture}, {family}, {nsfw} and {file_name}.', 'folder-open', 640),
            ],
        },
        {
            id: 'output_canvas',
            title: 'Output canvas',
            rows: [
                row('generation.output_canvas.visible', 'Show output canvas', 'Show the output canvas beside the graph in new workflow views.', 'image-multiple'),
            ],
        },
        {
            id: 'memory',
            title: 'Memory',
            rows: [
                row('generation.memory.mode', 'Application memory limit', 'How much memory SugarSubstitute may use for its own work and caches. Caches shrink at once when the system needs memory back.', 'ram', 640),
                row('generation.memory.share', 'Share of system memory', 'The limit as a percentage of this computer’s memory.', 'ram'),
                row('generation.memory.fixed', 'Fixed size (GB)', 'The limit as a fixed size.', 'ram'),
            ],
        },
    ],
};

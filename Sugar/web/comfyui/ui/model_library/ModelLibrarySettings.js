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
/** Present model-library configuration as rows of the SugarCubes settings workspace. */
import { settingsCatalogFor } from '../settings/workspace/SettingsCatalog.js';
import { SettingsState } from '../settings/workspace/SettingsModel.js';
/** Keep library actions out of the picker renderer and the workflow state owner. */
export class ModelLibrarySettings {
    #options;
    #registered = false;
    /** Bind public library clients and the established SugarCubes dialog service. */
    constructor(options) {
        this.#options = options;
    }
    /**
     * Contribute the model folder (the ComfyUI page's Connection), the download folder pattern
     * (Generation › Model downloads) and the catalog rescan (Model Sources › Model library).
     */
    setup() {
        if (this.#registered)
            return;
        this.#registered = true;
        const catalog = settingsCatalogFor(this.#options.app);
        const folder = new SettingsState('', async (path) => {
            const saved = await this.#options.roots.configure(path.trim() === '' ? null : path.trim());
            this.#folderStatus.set(saved.restartRequired
                ? 'Restart ComfyUI to use this folder.'
                : 'ComfyUI is using this folder.');
            await this.#loadFolder();
        });
        this.#folder = folder;
        catalog.contribute('comfyui.connection.model_folder', {
            controls: [
                { kind: 'text', value: folder, width: 420, placeholder: 'ComfyUI’s default model folder' },
                {
                    kind: 'button',
                    button: { label: 'Default', run: () => folder.write('') },
                },
                { kind: 'status', text: this.#folderStatus },
            ],
        });
        const pattern = new SettingsState('', (value) => this.#options.catalog.saveDownloadPattern(value.trim()));
        catalog.contribute('generation.model_downloads.pattern', {
            controls: [
                {
                    kind: 'text',
                    value: pattern,
                    width: 360,
                    placeholder: '{architecture}/{family}/{nsfw}/{file_name}',
                },
            ],
        });
        catalog.contribute('model_sources.model_library.catalog', {
            controls: [
                {
                    kind: 'button',
                    button: { label: 'Refresh catalog', run: () => this.#options.catalog.refresh() },
                },
            ],
        });
        void this.#loadFolder().catch(this.#options.reportError);
        void this.#options.catalog
            .downloadPattern()
            .then((value) => pattern.set(value))
            .catch(this.#options.reportError);
    }
    #folderStatus = new SettingsState('');
    #folder = null;
    /** Show the configured folder, or the default's path as the field's hint. */
    async #loadFolder() {
        const status = await this.#options.roots.load();
        this.#folder?.set(status.usesDefault ? '' : (status.configuredModelRoot ?? status.activeModelRoot));
        if (!this.#folderStatus.read())
            this.#folderStatus.set(`Active: ${status.activeModelRoot}`);
    }
}

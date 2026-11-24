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
/** Register the global output canvas setting through Comfy settings, independently of workflow JSON. */
import { ComfySettingStore } from '../../settings/workspace/ComfySettingStore.js';
import { settingsCatalogFor } from '../../settings/workspace/SettingsCatalog.js';
/** Identify whether workflows without a stored view choice open with the output canvas shown. */
export const OUTPUT_CANVAS_VISIBLE_SETTING = 'SugarCubes.OutputCanvas.Visible';
/** Register the canvas default once extension setup makes Comfy settings available. */
export class ComfyCanvasPreferences {
    app;
    #started = false;
    /** Bind the host without reading persistence until extension setup is ready. */
    constructor(app) {
        this.app = app;
    }
    /**
     * Register the visibility default that new workflow views read through Comfy settings, and
     * its row on the Generation page.
     *
     * @throws Error when Comfy's settings registration is unavailable.
     */
    setup() {
        if (this.#started)
            return;
        const store = new ComfySettingStore(this.app);
        store.register({
            id: OUTPUT_CANVAS_VISIBLE_SETTING,
            name: 'Show output canvas',
            type: 'hidden',
            defaultValue: true,
        });
        this.#started = true;
        settingsCatalogFor(this.app).contribute('generation.output_canvas.visible', {
            controls: [
                {
                    kind: 'switch',
                    value: store.value(OUTPUT_CANVAS_VISIBLE_SETTING, (value) => value !== false),
                },
            ],
        });
    }
}

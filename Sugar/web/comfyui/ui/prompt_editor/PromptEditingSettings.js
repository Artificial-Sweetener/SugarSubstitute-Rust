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
/** Contribute Prompt Editing's preference switches to SugarCubes' settings. */
import { settingsCatalogFor } from '../settings/workspace/SettingsCatalog.js';
/** Editor features in the order the page lists them, each under its Settings section. */
const FEATURE_ROWS = [
    { section: 'features', id: 'emphasis' },
    { section: 'features', id: 'wildcard_syntax' },
    { section: 'features', id: 'wildcard_autocomplete' },
    { section: 'features', id: 'autocomplete_ghost_text' },
    { section: 'features', id: 'lora_syntax' },
    { section: 'features', id: 'lora_autocomplete' },
    { section: 'features', id: 'lora_picker' },
    { section: 'features', id: 'lora_trigger_words' },
    { section: 'features', id: 'segment_reorder' },
    { section: 'features', id: 'spellcheck' },
    { section: 'features', id: 'duplicate_segment_diagnostics' },
    { section: 'danbooru', id: 'danbooru_url_import' },
];
/** Give every preference row its switch; the returned function withdraws them. */
export function registerPromptEditingSettings(app, preferences) {
    const catalog = settingsCatalogFor(app);
    const toggle = (id, value) => catalog.contribute(id, { controls: [{ kind: 'switch', value }] });
    const withdrawals = [
        toggle('prompt_editing.interaction.wheel_hover', preferences.wheelHover()),
        toggle('prompt_editing.wildcards.resolve', preferences.resolveOnGeneration()),
        ...FEATURE_ROWS.map(({ section, id }) => toggle(`prompt_editing.${section}.${id}`, preferences.feature(id))),
    ];
    return () => {
        for (const withdraw of withdrawals)
            withdraw();
    };
}

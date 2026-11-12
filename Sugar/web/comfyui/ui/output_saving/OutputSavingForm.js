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
/** Render the image saving controls while keeping validation and naming policy in their shared owners. */
import { decodeOutputSavingPreferences, } from './OutputSavingPreferences.js';
/** Own one draft's controls without touching host graph widgets or persisted workflow data. */
export class OutputSavingForm {
    document;
    root;
    example;
    folder;
    pattern;
    cubeMode;
    management;
    previews;
    jpeg;
    jpegMode;
    quality;
    targetSize;
    transfer;
    #controls = [];
    #busy = false;
    /** Build accessible themed controls and initialize their values from validated server state. */
    constructor(document, snapshot) {
        this.document = document;
        this.root = document.createElement('div');
        this.root.className = 'sugarcubes-output-saving-form';
        this.root.style.cssText =
            'display:grid;gap:12px;min-width:0;grid-template-columns:repeat(auto-fit,minmax(min(100%,210px),1fr))';
        this.folder = this.#input('Output folder', 'text', true);
        this.folder.placeholder = 'Use ComfyUI’s output folder';
        this.pattern = this.#input('Output pattern', 'text', true);
        this.pattern.setAttribute('aria-describedby', 'sugarcubes-output-pattern-tokens');
        const tokens = document.createElement('small');
        tokens.id = 'sugarcubes-output-pattern-tokens';
        tokens.textContent =
            'Relative folders and filename, without .png. Tokens: ' + snapshot.tokens.join(' ');
        tokens.style.overflowWrap = 'anywhere';
        tokens.style.gridColumn = '1 / -1';
        this.example = document.createElement('output');
        this.example.setAttribute('aria-label', 'Output preview');
        this.example.style.cssText = 'display:block;overflow-wrap:anywhere;grid-column:1 / -1';
        this.root.append(tokens, this.example);
        this.cubeMode = this.#select('Saved cube outputs', [
            ['all', 'Every cube'],
            ['final_cube', 'Final active cube'],
        ], true);
        this.management = this.#input('Manage workflow saving', 'checkbox');
        this.previews = this.#input('Save completed Preview Image outputs', 'checkbox');
        this.jpeg = this.#input('Save JPEG companions', 'checkbox');
        this.jpegMode = this.#select('JPEG sizing', [
            ['quality', 'Fixed quality'],
            ['target_size', 'Approximate target size'],
        ]);
        this.quality = this.#input('JPEG quality', 'number');
        this.quality.min = '1';
        this.quality.max = '100';
        this.quality.step = '1';
        this.targetSize = this.#input('JPEG target size (KiB)', 'number');
        this.targetSize.min = '1';
        this.targetSize.step = '1';
        this.transfer = this.#select('Drag and Copy format', [
            ['canonical_png', 'Canonical PNG'],
            ['companion_jpeg', 'JPEG companion'],
        ], true);
        this.root.addEventListener('change', () => this.#refreshDisabled());
        this.show(snapshot);
    }
    /** Validate the complete draft while preserving both active and inactive option values. */
    read() {
        return decodeOutputSavingPreferences({
            format: 'sugarcubes.output_saving',
            schema_version: 1,
            organization: {
                output_root: this.folder.value.trim() || null,
                path_pattern: this.pattern.value,
            },
            persistence_mode: this.cubeMode.value,
            manage_workflow: this.management.checked,
            save_previews: this.previews.checked,
            jpeg: {
                enabled: this.jpeg.checked,
                sizing_mode: this.jpegMode.value,
                quality: Number(this.quality.value),
                target_size_kib: Number(this.targetSize.value),
            },
            transfer: { preferred_format: this.transfer.value },
        });
    }
    /** Display an accepted snapshot without reconstructing control identity or replacing focused elements. */
    show(snapshot) {
        const value = snapshot.preferences;
        this.folder.value = value.organization.output_root ?? '';
        this.pattern.value = value.organization.path_pattern;
        this.cubeMode.value = value.persistence_mode;
        this.management.checked = value.manage_workflow;
        this.previews.checked = value.save_previews;
        this.jpeg.checked = value.jpeg.enabled;
        this.jpegMode.value = value.jpeg.sizing_mode;
        this.quality.value = String(value.jpeg.quality);
        this.targetSize.value = String(value.jpeg.target_size_kib);
        this.transfer.value = value.transfer.preferred_format;
        this.example.textContent = snapshot.examplePath;
        this.#refreshDisabled();
    }
    /** Keep an in-flight save or import from accepting competing draft edits. */
    setBusy(busy) {
        this.#busy = busy;
        this.#refreshDisabled();
    }
    /** Create a labeled native input using the established SugarCubes modal classes. */
    #input(label, type, wide = false) {
        const input = this.document.createElement('input');
        input.type = type;
        if (type !== 'checkbox')
            input.className = 'p-inputtext p-component sugarcubes-modal__text-input';
        this.#field(label, input, wide);
        return input;
    }
    /** Keep choice identities separate from display captions and render all captions literally. */
    #select(label, choices, wide = false) {
        const select = this.document.createElement('select');
        select.className = 'p-inputtext p-component';
        for (const [value, text] of choices) {
            const option = this.document.createElement('option');
            option.value = value;
            option.textContent = text;
            select.append(option);
        }
        this.#field(label, select, wide);
        return select;
    }
    /** Bind labels without global IDs or parsing dynamic content as markup. */
    #field(label, control, wide) {
        const row = this.document.createElement('label');
        row.className = 'sugarcubes-modal__field';
        const caption = this.document.createElement('span');
        caption.textContent = label;
        control.setAttribute('aria-label', label);
        if (control.type === 'checkbox') {
            row.classList.add('sugarcubes-modal__field--checkbox');
            row.style.gridColumn = '1 / -1';
            row.append(control, caption);
        }
        else {
            if (wide)
                row.style.gridColumn = '1 / -1';
            row.append(caption, control);
        }
        this.root.append(row);
        this.#controls.push(control);
    }
    /** Disable dependent controls without forgetting the user's retained choices. */
    #refreshDisabled() {
        for (const control of this.#controls)
            control.disabled = this.#busy;
        this.previews.disabled ||= !this.management.checked;
        this.jpegMode.disabled ||= !this.jpeg.checked;
        // JPEG copy renditions use this retained quality even when companions use target sizing or are disabled.
        this.targetSize.disabled ||= !this.jpeg.checked || this.jpegMode.value !== 'target_size';
        this.transfer.disabled ||= !this.jpeg.checked;
    }
}

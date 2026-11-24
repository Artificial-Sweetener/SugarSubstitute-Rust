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
/**
 * The Accent color row's controls as the Python application draws them: a colour swatch and its
 * hex value, then (after the row's source choice, when the host offers one) a Choose button that
 * opens the colour picker. Without a system accent to return to, a Default button restores the
 * default accent.
 */
import { DEFAULT_ACCENT_COLOR, normalizeAccentColor } from './AccentPalette.js';
/** Build the controls; `save` receives each chosen, normalized colour. */
export function createAccentColorControls(document, color, save, offerDefault) {
    const preview = document.createElement('span');
    preview.className = 'sc-accent-preview';
    const swatch = document.createElement('span');
    swatch.className = 'sc-accent-swatch';
    swatch.setAttribute('aria-hidden', 'true');
    const hex = document.createElement('span');
    hex.className = 'sc-accent-hex';
    preview.append(swatch, hex);
    const actions = document.createElement('span');
    actions.className = 'sc-accent-actions';
    const picker = document.createElement('input');
    picker.type = 'color';
    picker.className = 'sc-accent-picker';
    picker.tabIndex = -1;
    picker.setAttribute('aria-hidden', 'true');
    const show = (value) => {
        swatch.style.background = value;
        hex.textContent = value;
        picker.value = value;
    };
    const accept = (value) => {
        const normalized = normalizeAccentColor(value);
        if (!normalized)
            return;
        show(normalized);
        save(normalized);
    };
    const choose = button(document, 'Choose');
    choose.setAttribute('aria-label', 'Choose accent color');
    choose.addEventListener('click', () => {
        // The browser's colour dialog opens from the click, as Python's Choose opens its dialog.
        if (typeof picker.showPicker === 'function')
            picker.showPicker();
        else
            picker.click();
    });
    picker.addEventListener('input', () => accept(picker.value));
    actions.append(picker, choose);
    if (offerDefault) {
        const reset = button(document, 'Default');
        reset.addEventListener('click', () => accept(DEFAULT_ACCENT_COLOR));
        actions.append(reset);
    }
    show(color);
    return {
        preview,
        actions,
        update: (value, usesSystem) => {
            show(value);
            for (const control of actions.querySelectorAll('button'))
                control.disabled = usesSystem;
        },
    };
}
function button(document, label) {
    const element = document.createElement('button');
    element.type = 'button';
    element.className = 'sc-sw-button sc-accent-button';
    element.textContent = label;
    return element;
}

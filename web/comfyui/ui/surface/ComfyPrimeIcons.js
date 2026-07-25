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
/** Adapt Comfy's host-provided PrimeIcons to DOM and legacy canvas surfaces. */
const PRIME_ICON_FONT = "'PrimeIcons'";
const ICONS = Object.freeze({
    'arrow-left': { className: 'pi-arrow-left', glyph: '\uE91A' },
    'arrow-right': { className: 'pi-arrow-right', glyph: '\uE91B' },
    box: { className: 'pi-box', glyph: '\uE9D9' },
    circle: { className: 'pi-circle', glyph: '\uE9DC' },
    'circle-fill': { className: 'pi-circle-fill', glyph: '\uE9DD' },
    eye: { className: 'pi-eye', glyph: '\uE966' },
    'window-maximize': { className: 'pi-window-maximize', glyph: '\uE93B' },
});
/** Create one host-styled PrimeIcons element without reproducing its vector geometry. */
export function createComfyPrimeIconElement(documentRef, name) {
    const icon = documentRef.createElement('i');
    icon.classList.add('pi', ICONS[name].className);
    icon.setAttribute('aria-hidden', 'true');
    return icon;
}
/** Draw one PrimeIcons glyph through the same font used by LiteGraph title buttons. */
export function drawComfyPrimeIcon(context, name, centerX, centerY, fontSize = 16) {
    context.save();
    context.font = `${String(fontSize)}px ${PRIME_ICON_FONT}`;
    context.textAlign = 'center';
    context.textBaseline = 'middle';
    context.fillText(ICONS[name].glyph, centerX, centerY);
    context.restore();
}
/** Return the host class for assertions and integration adapters. */
export function comfyPrimeIconClass(name) {
    return ICONS[name].className;
}
/** Return the font glyph required by Comfy's legacy canvas renderer. */
export function comfyPrimeIconGlyph(name) {
    return ICONS[name].glyph;
}

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
/** Share accent capsules and readable foregrounds across DOM and canvas renderers. */
const backgroundTokens = [
    '--sc-accent-color',
    '--sc-brand-text',
    '--p-primary-color',
    '--primary-color',
];
const foregroundTokens = [
    '--sc-accent-foreground',
    '--sugarcubes-model-pill-punchout',
    '--comfy-menu-bg',
    '--p-content-background',
];
const fallbackBackground = '#60cdff';
const fallbackForeground = '#171b20';
/** Follow the active Fluent or host accent for every DOM pill. */
export const CUBE_MODEL_PILL_BACKGROUND = tokenFallback(backgroundTokens, fallbackBackground);
/** Resolve the shared readable foreground before the host's default letterform treatment. */
export const CUBE_MODEL_PILL_FOREGROUND = tokenFallback(foregroundTokens, fallbackForeground);
/** Read resolved host tokens for Canvas, whose fillStyle cannot consume CSS variables. */
export function resolveCubeModelPillColors(element) {
    const style = element?.ownerDocument.defaultView?.getComputedStyle(element);
    return {
        background: readToken(style, backgroundTokens, fallbackBackground),
        foreground: readToken(style, foregroundTokens, fallbackForeground),
    };
}
/** Build one CSS fallback chain from the same palette priority used by Canvas. */
function tokenFallback(tokens, fallback) {
    return tokens.reduceRight((value, token) => `var(${token}, ${value})`, fallback);
}
/** Accept the first resolved palette color, retaining a usable default without a host theme. */
function readToken(style, tokens, fallback) {
    for (const token of tokens) {
        const color = style?.getPropertyValue(token).trim();
        if (color)
            return color;
    }
    return fallback;
}

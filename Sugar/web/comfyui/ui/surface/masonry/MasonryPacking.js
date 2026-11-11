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
 * Own the edge-to-edge packing decision shared by masonry and the Cube stack.
 *
 * Packing (zero gaps, zero insets, square rail corners and a flush rail) belongs to the
 * Compact masonry preference and to Plugin Mode's editor-panel presentation. The Cube stack's
 * own compact preference only changes the rail to icon-only rows and never packs anything.
 */
import { readPluginMode, subscribePluginMode } from '../../core/PluginMode.js';
import { COMPACT_MASONRY_CHANGE } from './CompactMasonryPreference.js';
const COMPACT_MASONRY_ATTRIBUTE = 'data-sc-compact-masonry';
const PLUGIN_MODE_ATTRIBUTE = 'data-sc-plugin-mode';
/** Scope rules to packed presentation; each root attribute is listed for broad selector support. */
export function packedMasonryRule(selector) {
    return `:root[${COMPACT_MASONRY_ATTRIBUTE}] ${selector}, :root[${PLUGIN_MODE_ATTRIBUTE}] ${selector}`;
}
/** Match the root only while neither packing owner is active. */
export const SPACED_MASONRY_ROOT = `:root:not([${COMPACT_MASONRY_ATTRIBUTE}]):not([${PLUGIN_MODE_ATTRIBUTE}])`;
/** Read whether masonry and the Cube stack currently pack edge to edge. */
export function readMasonryPacked(document) {
    return (document.documentElement.hasAttribute(COMPACT_MASONRY_ATTRIBUTE) || readPluginMode(document));
}
/** Follow both packing owners through their named change events only. */
export function subscribeMasonryPacking(document, changed) {
    document.addEventListener(COMPACT_MASONRY_CHANGE, changed);
    const stopPlugin = subscribePluginMode(document, changed);
    return () => {
        document.removeEventListener(COMPACT_MASONRY_CHANGE, changed);
        stopPlugin();
    };
}

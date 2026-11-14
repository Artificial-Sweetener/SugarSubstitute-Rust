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
/** Own every word the What's new view shows, so its wording has one place to change. */
export const WHATS_NEW_TEXT = Object.freeze({
    title: "What's new",
    close: 'Close',
    loading: "Loading what's new…",
    failed: "Couldn't load what's new. Check your connection and try again.",
    retry: 'Retry',
    /** Name one product's section: its running version, and the version it updated from. */
    section(product, currentVersion, updatedFrom) {
        return updatedFrom === null
            ? `${product} ${currentVersion}`
            : `${product}, updated from ${updatedFrom} to ${currentVersion}`;
    },
    notPublished(product, currentVersion) {
        return `No release notes are published for ${product} ${currentVersion} yet.`;
    },
    omitted(count) {
        return count === 1
            ? 'The notes of 1 earlier version are not shown.'
            : `The notes of ${String(count)} earlier versions are not shown.`;
    },
});

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
/** Describe authoritative selection and inspection intent without renderer state. */
/** Create independent default state for an authority-owned view. */
export function initialCanvasViewState(address) {
    return Object.freeze({
        ...address,
        revision: 0,
        selection: null,
        sourcePreview: true,
        follow: 'auto',
        mode: 'image',
        linked: true,
        comparison: null,
    });
}
/** Copy nested coordination values so a caller cannot mutate a published state. */
export function freezeCanvasViewState(state, revision) {
    return Object.freeze({
        ...state,
        revision,
        selection: state.selection ? Object.freeze({ ...state.selection }) : null,
        comparison: state.comparison
            ? Object.freeze({
                ...state.comparison,
                secondary: state.comparison.secondary
                    ? Object.freeze({ ...state.comparison.secondary })
                    : null,
            })
            : null,
    });
}
/** Identify camera/wipe-only presentation transitions that require no catalog or resource reconciliation. */
export function sameCanvasNavigation(previous, next) {
    return (previous.viewId === next.viewId &&
        previous.sessionId === next.sessionId &&
        previous.follow === next.follow &&
        previous.mode === next.mode &&
        previous.sourcePreview === next.sourcePreview &&
        previous.linked === next.linked &&
        previous.selection?.sourceId === next.selection?.sourceId &&
        previous.selection?.batchIndex === next.selection?.batchIndex &&
        Boolean(previous.comparison) === Boolean(next.comparison) &&
        previous.comparison?.secondary?.sourceId === next.comparison?.secondary?.sourceId &&
        previous.comparison?.secondary?.batchIndex === next.comparison?.secondary?.batchIndex);
}

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
/** Define renderer-neutral horizontal sections for Cube output previews. */
/** Separate adjacent Cube output sections in either native renderer. */
export const CUBE_PREVIEW_SECTION_GAP = 8;
/** Inset output titles and media from the preview rail boundary. */
export const CUBE_PREVIEW_SECTION_INSET = 6;
/** Reserve the shared title-row height used to anchor native output ports. */
export const CUBE_PREVIEW_TITLE_LINE_HEIGHT = 20;
/** Inset every canvas preview section consistently from its rail boundary. */
export function resolveCubePreviewContentRect(area) {
    return {
        x: area.x + CUBE_PREVIEW_SECTION_INSET,
        y: area.y + CUBE_PREVIEW_SECTION_INSET,
        width: Math.max(1, area.width - CUBE_PREVIEW_SECTION_INSET * 2),
        height: Math.max(1, area.height - CUBE_PREVIEW_SECTION_INSET * 2),
    };
}
/** Return every boundary output in stable graph order. */
export function resolveCubeOutputSections(snapshot) {
    return snapshot.outputs;
}
/**
 * Resolve one canvas media item per boundary output.
 *
 * Internal previews fill otherwise empty output sections because Nodes 1.0
 * canvas rendering has no DOM flow for additional preview media.
 */
export function resolveCubeCanvasPreviewSections(snapshot) {
    if (!snapshot)
        return [];
    let internalIndex = 0;
    return snapshot.outputs.map((output) => {
        const outputItem = output.items[0];
        if (outputItem)
            return { canonicalName: output.id, item: outputItem };
        const internalItem = snapshot.internalItems[internalIndex] ?? null;
        if (internalItem)
            internalIndex += 1;
        return { canonicalName: output.id, item: internalItem };
    });
}
/** Divide one preview area into ordered, equal-height horizontal segments. */
export function dividePreviewIntoHorizontalSegments(area, sectionCount, gap) {
    const count = Math.max(0, Math.floor(sectionCount));
    if (count === 0)
        return [];
    const safeGap = Math.max(0, gap);
    const totalGap = safeGap * Math.max(0, count - 1);
    const height = Math.max(1, (area.height - totalGap) / count);
    return Array.from({ length: count }, (_, index) => ({
        x: area.x,
        y: area.y + index * (height + safeGap),
        width: area.width,
        height,
    }));
}
/** Return the vertical center of each preview section's visible title row. */
export function resolveCubePreviewTitleAnchors(area, sectionCount) {
    const content = resolveCubePreviewContentRect(area);
    return dividePreviewIntoHorizontalSegments(content, sectionCount, CUBE_PREVIEW_SECTION_GAP).map((section) => section.y + CUBE_PREVIEW_TITLE_LINE_HEIGHT / 2);
}

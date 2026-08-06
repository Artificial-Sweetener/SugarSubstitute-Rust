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
/** Compute renderer-independent Nodes 1.0 Cube surface geometry. */
import { requireCubeIdentity } from '../cube/node/ComfyCubeNodeFactory.js';
import { isCubeAwaitingFirstSave } from '../cube/CubeIdentityPresentation.js';
import { CUBE_CANVAS_ACTIVATION_SIZE } from './CubeCanvasActivationControl.js';
import { CUBE_RESIZE_EDGES } from '../cube/geometry/CubeResizeGeometry.js';
import { resolveCubeFaceCardPresentation, } from './CubeFaceCardPolicy.js';
import { measureCubeFaceNodeBodyHeight } from './CubeFaceNodeMeasurement.js';
import { computeCubeMasonry } from './CubeMasonryLayout.js';
import { cubeMinimumSize, resolveCubeSurfaceMinimumHeight } from './CubeSurfaceMinimumHeight.js';
import { resolveCubeSurfaceCardSpacing } from './CubeSurfaceSpacing.js';
import { layoutCubeCanvasChrome } from './CubeCanvasChromeLayout.js';
import { layoutCubeInputPorts, layoutCubeOutputPorts, resolveCubePortGutters, } from './CubePortGutterLayout.js';
import { resolveCubePreviewWidthRange, } from './CubePreviewResizeGeometry.js';
import { CUBE_SURFACE_FRAME_PADDING, CUBE_SURFACE_SECTION_GAP } from './CubeSurfaceGeometry.js';
const HEADER_HEIGHT = 42;
const RESIZE_HANDLE_SIZE = 18;
const RESIZE_EDGE_THICKNESS = 10;
const PREVIEW_DIVIDER_HIT_WIDTH = 10;
/** Lay out one finite canvas Cube using the same persisted masonry policy as Nodes 2.0. */
export function computeCubeCanvasLayout(node, state, titleHeight, titlebarActionKeys = [], externalInterface = {
    inputSlots: node.inputs.map((_, index) => index),
    outputSlots: node.outputs.map((_, index) => index),
}) {
    const frame = rect(Number(node.pos[0]), Number(node.pos[1]) - titleHeight, Number(node.size[0]), Number(node.size[1]) + titleHeight);
    const header = rect(frame.x, frame.y, frame.width, HEADER_HEIGHT);
    const spacing = resolveCubeSurfaceCardSpacing(state);
    const baseContent = rect(frame.x + CUBE_SURFACE_FRAME_PADDING, frame.y + HEADER_HEIGHT + spacing.headerInset, Math.max(1, frame.width - CUBE_SURFACE_FRAME_PADDING * 2), Math.max(1, frame.height - HEADER_HEIGHT - spacing.headerInset - spacing.footerInset));
    const gutters = resolveCubePortGutters(frame, baseContent, {
        hasInputs: externalInterface.inputSlots.length > 0,
        hasOutputs: externalInterface.outputSlots.length > 0,
    });
    const content = gutters.content;
    const minimumMasonryWidth = Math.min(content.width, Math.max(1, state.minimumColumnWidth));
    const previewVisible = state.preview.visible;
    const previewRight = previewVisible ? frame.x + frame.width : content.x + content.width;
    const previewWidthRange = resolveCubePreviewWidthRange(Math.max(0, previewRight - content.x), minimumMasonryWidth, CUBE_SURFACE_SECTION_GAP);
    const previewWidth = previewVisible
        ? Math.min(state.preview.width, previewWidthRange.maximum)
        : 0;
    const masonryWidth = Math.max(1, (previewWidth > 0 ? previewRight : content.x + content.width) -
        content.x -
        (previewWidth > 0 ? previewWidth + CUBE_SURFACE_SECTION_GAP : 0));
    const masonry = rect(content.x, content.y, masonryWidth, content.height);
    const preview = previewWidth > 0
        ? rect(previewRight - previewWidth, content.y, previewWidth, content.height)
        : null;
    const previewDivider = preview
        ? rect(preview.x - CUBE_SURFACE_SECTION_GAP / 2 - PREVIEW_DIVIDER_HIT_WIDTH / 2, preview.y, PREVIEW_DIVIDER_HIT_WIDTH, preview.height)
        : null;
    const presentation = resolveCubeFaceCardPresentation(node.subgraph._nodes, state, node.subgraph);
    const chrome = layoutCubeCanvasChrome(header, {
        showCardMenu: presentation.menuEntries.length > 0,
        showUnsavedIndicator: isCubeAwaitingFirstSave(requireCubeIdentity(node)),
        titlebarActionKeys,
    });
    const measuredCards = presentation.cards
        .filter((card) => card.visible)
        .map((card) => ({
        ...card,
        bodyHeight: measureCubeFaceNodeBodyHeight(card.node),
    }));
    const masonryLayout = computeCubeMasonry(measuredCards.map((card) => ({
        id: card.id,
        height: card.bodyHeight + titleHeight,
        ...(card.columnSpan === undefined ? {} : { columnSpan: card.columnSpan }),
    })), {
        availableWidth: masonry.width,
        minimumColumnWidth: state.minimumColumnWidth,
        gap: spacing.gap,
    });
    const cards = masonryLayout.placements.map((placement, index) => {
        const card = measuredCards[index];
        if (!card)
            throw new RangeError('Cube masonry placement has no corresponding node.');
        const cardRect = rect(masonry.x + placement.x, masonry.y + placement.y, placement.width, placement.height);
        return {
            node: card.node,
            id: card.id,
            label: card.label,
            enabled: card.enabled,
            showActivationControl: card.showActivationControl,
            bodyHeight: card.bodyHeight,
            rect: cardRect,
            activationAction: card.showActivationControl
                ? rect(cardRect.x + cardRect.width - CUBE_CANVAS_ACTIVATION_SIZE.width - 6, cardRect.y + (titleHeight - CUBE_CANVAS_ACTIVATION_SIZE.height) / 2, CUBE_CANVAS_ACTIVATION_SIZE.width, CUBE_CANVAS_ACTIVATION_SIZE.height)
                : null,
        };
    });
    const minimumSize = cubeMinimumSize(resolveCubeSurfaceMinimumHeight({
        contentHeight: masonryLayout.height,
        headerInset: spacing.headerInset,
        footerInset: spacing.footerInset,
        faceHeaderHeight: HEADER_HEIGHT,
        nativeTitleHeight: titleHeight,
    }));
    return {
        frame,
        header,
        content,
        inputGutter: gutters.inputGutter,
        outputGutter: gutters.outputGutter,
        masonry,
        preview,
        previewDivider,
        previewWidthRange,
        editAction: chrome.editAction,
        unsavedIndicator: chrome.unsavedIndicator,
        cardMenuAction: chrome.cardMenuAction,
        chromeActions: chrome.chromeActions,
        cardMenuEntries: presentation.menuEntries,
        resizeHandles: layoutResizeHandles(frame),
        cards,
        inputs: layoutCubeInputPorts(externalInterface.inputSlots.map((index) => node.subgraph.inputs[index]), frame, HEADER_HEIGHT, externalInterface.inputSlots),
        outputs: layoutCubeOutputPorts(externalInterface.outputSlots.map((index) => node.outputs[index]), frame, preview, HEADER_HEIGHT, externalInterface.outputSlots),
        minimumSize,
    };
}
/** Place four edge and four corner hit targets around one canvas frame. */
function layoutResizeHandles(frame) {
    const byEdge = {
        n: rect(frame.x + RESIZE_HANDLE_SIZE, frame.y, frame.width - RESIZE_HANDLE_SIZE * 2, RESIZE_EDGE_THICKNESS),
        ne: rect(frame.x + frame.width - RESIZE_HANDLE_SIZE, frame.y, RESIZE_HANDLE_SIZE, RESIZE_HANDLE_SIZE),
        e: rect(frame.x + frame.width - RESIZE_EDGE_THICKNESS, frame.y + RESIZE_HANDLE_SIZE, RESIZE_EDGE_THICKNESS, frame.height - RESIZE_HANDLE_SIZE * 2),
        se: rect(frame.x + frame.width - RESIZE_HANDLE_SIZE, frame.y + frame.height - RESIZE_HANDLE_SIZE, RESIZE_HANDLE_SIZE, RESIZE_HANDLE_SIZE),
        s: rect(frame.x + RESIZE_HANDLE_SIZE, frame.y + frame.height - RESIZE_EDGE_THICKNESS, frame.width - RESIZE_HANDLE_SIZE * 2, RESIZE_EDGE_THICKNESS),
        sw: rect(frame.x, frame.y + frame.height - RESIZE_HANDLE_SIZE, RESIZE_HANDLE_SIZE, RESIZE_HANDLE_SIZE),
        w: rect(frame.x, frame.y + RESIZE_HANDLE_SIZE, RESIZE_EDGE_THICKNESS, frame.height - RESIZE_HANDLE_SIZE * 2),
        nw: rect(frame.x, frame.y, RESIZE_HANDLE_SIZE, RESIZE_HANDLE_SIZE),
    };
    return CUBE_RESIZE_EDGES.map((edge) => ({ edge, rect: byEdge[edge] }));
}
/** Return whether one graph-space point lies inside a finite rectangle. */
export function containsCubeCanvasPoint(target, point) {
    return (point[0] >= target.x &&
        point[0] <= target.x + target.width &&
        point[1] >= target.y &&
        point[1] <= target.y + target.height);
}
/** Build a finite rectangle without allowing invalid host geometry inward. */
function rect(x, y, width, height) {
    return {
        x: Number.isFinite(x) ? x : 0,
        y: Number.isFinite(y) ? y : 0,
        width: Number.isFinite(width) ? Math.max(1, width) : 1,
        height: Number.isFinite(height) ? Math.max(1, height) : 1,
    };
}

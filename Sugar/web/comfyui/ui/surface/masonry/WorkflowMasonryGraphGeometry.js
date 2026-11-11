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
/** Measure identified graph cards without discovering nodes or changing canonical geometry. */
import { masonrySupplementSources } from './MasonrySupplementBoundary.js';
/** Read actual loose-node and exposed Cube-face bounds in one shared screen coordinate space. */
export function measureWorkflowMasonryGraph(container, sections) {
    const elements = workflowMasonryGraphCards(container, sections);
    const viewport = container.getBoundingClientRect();
    const cards = new Map();
    for (const [id, element] of elements) {
        const rectangle = element.getBoundingClientRect();
        if (rectangle.width <= 0 || rectangle.height <= 0)
            continue;
        const computed = element.ownerDocument.defaultView?.getComputedStyle(element);
        const layoutWidth = Number.parseFloat(computed?.width ?? '') || element.offsetWidth || rectangle.width;
        const clip = element.closest('.sugarcubes-cube-face');
        cards.set(id, {
            element,
            rectangle,
            layoutWidth,
            scale: rectangle.width / layoutWidth,
            visible: rectanglesIntersect(rectangle, viewport) &&
                (!clip || rectanglesIntersect(rectangle, clip.getBoundingClientRect())),
        });
    }
    return { cards, layers: workflowMasonryGraphLayers(container), viewport };
}
/** Resolve DOM identity only for graph-projected cards, including source reservations. */
export function workflowMasonryGraphCards(container, sections) {
    const root = container.closest('#graph-canvas-container') ?? container;
    const roots = new Map([
        ...root.querySelectorAll('.lg-node[data-node-id], .lg-node[data-sc-masonry-graph-node]'),
    ].map((element) => [element.dataset.nodeId ?? element.dataset.scMasonryGraphNode, element]));
    const cards = new Map();
    for (const section of sections) {
        const cubeRoot = section.cube ? roots.get(String(section.cube.id)) : undefined;
        const faceCards = new Map([...(cubeRoot?.querySelectorAll('[data-cube-node-id]') ?? [])].map((element) => [
            element.dataset.cubeNodeId,
            element,
        ]));
        for (const card of section.cards) {
            if (!card.visible)
                continue;
            const element = section.cube
                ? faceCards.get(String(card.node.id))
                : roots.get(String(card.node.id));
            if (!element)
                continue;
            cards.set(card.id, element);
        }
    }
    for (const [id, source] of masonrySupplementSources(root))
        cards.set(id, source);
    return cards;
}
/** Identify host graph layers shared by visual transitions and resting interaction suppression. */
export function workflowMasonryGraphLayers(container) {
    const root = container.closest('#graph-canvas-container') ?? container;
    return [...root.children].filter((element) => element instanceof HTMLElement && element.matches('#graph-canvas, .ph-no-capture, .isolate'));
}
/** Exclude wholly offscreen endpoints from compositor allocation. */
export function rectanglesIntersect(left, right) {
    return (left.width > 0 &&
        left.height > 0 &&
        left.right > right.left &&
        left.left < right.right &&
        left.bottom > right.top &&
        left.top < right.bottom);
}

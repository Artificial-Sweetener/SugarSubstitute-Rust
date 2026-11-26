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
 * The chin of a node card: the button row under its body that Comfy draws for "Show advanced
 * inputs", "Enter subgraph" and any other footer action.
 *
 * Comfy tucks the chin 20 px up behind the node body and lets the opaque body cover that part.
 * Our card bodies are see-through, so a tucked chin would show through them. Here the body's inner
 * wrapper draws the one divider as its lower edge, rounded at both ends by the card's corner radius,
 * in every presentation (a compact masonry wrapper has no border of its own). The chin tucks up
 * only by that radius, and a mask keeps that strip to the two corners outside the curve, so the
 * chin's fill, hover and pressed colour meet the curved divider and never reach inside the body.
 * The visible height is Comfy's: 24 px, its label centred where Comfy places it.
 *
 * Every chin button is styled by these rules alone; a visual style changes only their tokens.
 */

/** A node card's chin: the element after the card's inner wrapper. */
export const NODE_CHIN = '[data-sc-style-node] > [data-testid="node-inner-wrapper"] + *';
/** Any chin button of a styled node card. */
export const NODE_CHIN_BUTTON = `${NODE_CHIN} > button`;
/** A styled node card that has a chin. */
const WITH_CHIN = '[data-sc-style-node]:has(> [data-testid="node-inner-wrapper"] + * > button)';
/** Comfy's visible chin height: its 44 px button less the 20 px tucked under the body. */
const CHIN_HEIGHT = 24;
/** The card's corner radius, which rounds the divider and sets how far the chin tucks up. */
const RADIUS = 'var(--sc-node-card-radius)';
/** The chin's corner tiles: whole outside the divider's curve, empty inside it. */
const outsideCurve = (centre: string): string =>
  `radial-gradient(circle at ${centre}, transparent calc(${RADIUS} - .5px), #000 ${RADIUS})`;

/** Geometry, fill and states of every chin button. */
export function nodeChinCss(): string {
  return `
    ${NODE_CHIN} { margin-top:calc(0px - ${RADIUS}) !important; z-index:auto; }
    ${NODE_CHIN_BUTTON} {
      box-sizing:border-box;
      height:calc(${String(CHIN_HEIGHT)}px + ${RADIUS}); min-height:calc(${String(CHIN_HEIGHT)}px + ${RADIUS});
      padding-top:calc(${String(CHIN_HEIGHT / 2)}px + ${RADIUS}); padding-bottom:${String(CHIN_HEIGHT / 2)}px;
      border:0; border-radius:0 0 ${RADIUS} ${RADIUS};
      background-color:var(--sc-card-fill);
      mask:linear-gradient(#000,#000) 0 100% / 100% calc(100% - ${RADIUS}) no-repeat,
        ${outsideCurve('100% 0')} 0 0 / ${RADIUS} ${RADIUS} no-repeat,
        ${outsideCurve('0 0')} 100% 0 / ${RADIUS} ${RADIUS} no-repeat;
      font-family:var(--sc-font-family); cursor:pointer;
      transition:background-color var(--sc-motion-fast) var(--sc-motion-ease);
    }
    ${NODE_CHIN_BUTTON}:is(:hover,:focus-visible) { background-color:var(--sc-chin-hover, var(--sc-surface-hover)); }
    ${NODE_CHIN_BUTTON}:active { background-color:var(--sc-chin-pressed, var(--sc-surface-pressed)); }
    ${WITH_CHIN} > [data-testid="node-inner-wrapper"] {
      border-bottom:1px solid var(--sc-row-stroke);
      border-bottom-left-radius:${RADIUS}; border-bottom-right-radius:${RADIUS};
    }
    /* A compact masonry card's wrapper draws no border, but the divider stays. */
    html[data-sc-compact-masonry] .sugarcubes-cube-face__node-card ${WITH_CHIN} > [data-testid="node-inner-wrapper"] {
      border-bottom:1px solid var(--sc-row-stroke) !important;
    }
    ${WITH_CHIN} [data-testid^="node-body-"] {
      border-radius:0 0 calc(${RADIUS} - 1px) calc(${RADIUS} - 1px);
    }
    ${WITH_CHIN} [data-testid^="node-body-"]:has(> [data-sc-style-fields]) { padding-bottom:0; }
    @media (prefers-reduced-motion:reduce) { ${NODE_CHIN_BUTTON} { transition:none; } }
  `;
}

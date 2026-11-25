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
 * Our card bodies are see-through, so the tucked part showed through: its top edge as a second
 * divider, and its hover as a band reaching above the divider. Here the chin is not tucked: it
 * starts at the body's lower edge and draws the one divider there itself, as its top edge; the
 * inner wrapper above it draws no lower edge of its own, in any presentation (a compact masonry
 * wrapper has no border at all). Everything the chin paints (fill, hover, pressed) stays below
 * the divider: its fill is clipped to inside its border, and its states set only the colour,
 * which keeps that clip. The visible height is Comfy's: 24 px, its label centred where Comfy places it.
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
/** Geometry, fill and states of every chin button. */
export function nodeChinCss() {
    return `
    ${NODE_CHIN} { margin-top:0 !important; z-index:auto; }
    ${NODE_CHIN_BUTTON} {
      box-sizing:border-box; height:${String(CHIN_HEIGHT)}px; min-height:${String(CHIN_HEIGHT)}px;
      padding-top:${String(CHIN_HEIGHT / 2 - 1)}px; padding-bottom:${String(CHIN_HEIGHT / 2)}px;
      border:0; border-top:1px solid var(--sc-row-stroke);
      border-radius:0 0 var(--sc-node-card-radius) var(--sc-node-card-radius);
      background-color:var(--sc-card-fill); background-clip:padding-box;
      font-family:var(--sc-font-family); cursor:pointer;
      transition:background-color var(--sc-motion-fast) var(--sc-motion-ease);
    }
    ${NODE_CHIN_BUTTON}:is(:hover,:focus-visible) { background-color:var(--sc-chin-hover, var(--sc-surface-hover)); }
    ${NODE_CHIN_BUTTON}:active { background-color:var(--sc-chin-pressed, var(--sc-surface-pressed)); }
    ${WITH_CHIN} > [data-testid="node-inner-wrapper"] {
      border-bottom-width:0; border-bottom-left-radius:0; border-bottom-right-radius:0;
    }
    ${WITH_CHIN} [data-testid^="node-body-"] { border-radius:0; }
    ${WITH_CHIN} [data-testid^="node-body-"]:has(> [data-sc-style-fields]) { padding-bottom:0; }
    @media (prefers-reduced-motion:reduce) { ${NODE_CHIN_BUTTON} { transition:none; } }
  `;
}

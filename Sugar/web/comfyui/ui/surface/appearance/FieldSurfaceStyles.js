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
/** Own the one field-surface primitive that every themed field control inherits. */
/**
 * Identify the element that paints each field kind's surface.
 *
 * Each control contributes exactly one painter so translucent design-system fills never stack.
 */
export const FIELD_SURFACE_PAINTERS = [
    '[data-sc-style-combo-frame]',
    '[data-sc-number-spinner]',
    '[data-sc-style-edit-frame] > input',
    '[data-sc-style-edit-frame] > textarea',
    '.sc-model-field',
];
/** Mark editor-backed fields whose provider paints the resolved surface inside its own component. */
export const FIELD_SURFACE_DELEGATES = ['[data-sc-editor-field]'];
/** Bound the primitive to themed node and masonry surfaces, never to unrelated host UI. */
const scope = ':is([data-sc-style-node], [data-sc-style-surface])';
const allFields = [...FIELD_SURFACE_PAINTERS, ...FIELD_SURFACE_DELEGATES].join(', ');
const editingFields = [...FIELD_SURFACE_PAINTERS, ...FIELD_SURFACE_DELEGATES]
    .filter((selector) => selector !== '[data-sc-style-combo-frame]')
    .join(', ');
/**
 * Select fields in one state without specificity, so source order alone ranks rest, hover, focus
 * and disabled, and a design system may refine one control's state through its own rule.
 */
const state = (...conditions) => conditions.map((condition) => `:where(${scope} ${condition})`).join(', ');
const rest = state(`:is(${allFields})`);
const hovered = state(`:is(${allFields}):hover`);
/** Treat a combo as focused only while its search is editing; a focused closed trigger is a button. */
const focused = state(`:is(${editingFields}):focus-within`, '[data-sc-style-combo-frame]:has([aria-expanded="true"])', '.sc-model-field-open');
/** Disabled controls report through their own element or the native control they frame. */
const disabled = state(`:is(${allFields}):disabled`, `:is(${allFields})[aria-disabled="true"]`, `:is(${allFields}):has(input:disabled)`, `:is(${allFields}):has(textarea:disabled)`, `:is(${allFields}):has([role="combobox"][aria-disabled="true"])`);
/**
 * Paint above the host's utility-class fills while staying below a design system's own field
 * rules, which retain the look of styles that draw their controls differently.
 */
const painters = `html ${scope} :is(${FIELD_SURFACE_PAINTERS.join(', ')})`;
/**
 * Resolve every field's rest, hover, focused and disabled fill from the provider's field tokens.
 *
 * The resolved `--sc-field-surface` is the single state result. Painters apply it directly;
 * delegates, such as editor components with shadow-owned chrome, map it into their own painter.
 * The inline combo search covers its trigger while editing, so it composites the focused fill over
 * the card to hide the trigger text beneath and remain identical to the focused frame.
 */
export function fieldSurfaceCss() {
    return `
    ${rest} { --sc-field-surface:var(--sc-field-fill); }
    ${hovered} { --sc-field-surface:var(--sc-field-hover-fill); }
    ${focused} { --sc-field-surface:var(--sc-field-focus-fill); }
    ${disabled} { --sc-field-surface:var(--sc-field-disabled-fill); }
    ${painters} {
      background-color:var(--sc-field-surface);
      transition:background-color var(--sc-motion-fast) var(--sc-motion-ease), border-color var(--sc-motion-fast) var(--sc-motion-ease);
    }
    [data-sc-style-popup] [data-sc-inline-search] {
      background:linear-gradient(var(--sc-field-focus-fill), var(--sc-field-focus-fill)), var(--sc-card-fill);
    }
    @media (prefers-reduced-motion:reduce) {
      ${painters} { transition:none; }
    }
  `;
}

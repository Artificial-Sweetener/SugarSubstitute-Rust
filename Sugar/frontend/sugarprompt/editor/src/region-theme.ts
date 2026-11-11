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
/** Native regional labels, dividers and rails use native line height and ordinal color feedback. */
import { regionRailMetrics } from './region-paint.js';
import { EditorView } from '@codemirror/view';

/** Framing follows prepared line structure and never mutates CodeMirror-managed DOM. */
export const regionTheme = EditorView.theme({
  '.sp-region-row': { position: 'relative' },
  '.sp-region-separator': {
    display: 'inline-block',
    position: 'relative',
    containerType: 'inline-size',
    width: '100%',
    height: '1lh',
    verticalAlign: 'top',
    whiteSpace: 'nowrap',
    '--sp-region-rule-width': 'min(36px, 20cqi)',
  },
  // Transform centering retains half-layout-unit precision; container units keep rules relative to the full separator.
  '.sp-region-framing': {
    display: 'inline-flex',
    position: 'absolute',
    left: '50%',
    top: '0',
    transform: 'translateX(-50%)',
    width: 'max-content',
    maxWidth: '100%',
    height: '100%',
    alignItems: 'center',
    gap: '8px',
  },
  '.sp-region-framing::before, .sp-region-framing::after': {
    content: "''",
    flex: '0 0 var(--sp-region-rule-width)',
    width: 'var(--sp-region-rule-width)',
    height: '2px',
    backgroundColor: 'var(--sp-region-color)',
  },
  '.sp-region-separator[data-region-named="false"] .sp-region-framing': {
    gap: '0',
  },
  '.sp-region-separator[data-region-named="false"] .sp-region-framing::after': {
    display: 'none',
  },
  '.sp-region-separator[data-region-named="false"] .sp-region-label': {
    width: '0',
  },
  '.sp-region-label': {
    fontWeight: '700',
    whiteSpace: 'pre',
    overflow: 'hidden',
    textOverflow: 'ellipsis',
    maxWidth: 'calc(100cqi - 2 * (min(36px, 20cqi) + 8px))',
  },
  '.sp-region-content': {
    position: 'relative',
    '--sp-region-rail-width': `${regionRailMetrics.width}px`,
  },
  '.sp-region-content::after': {
    content: "''",
    position: 'absolute',
    top: '0',
    bottom: '0',
    left: `calc(${regionRailMetrics.axis}px - var(--sp-region-rail-width) / 2)`,
    width: 'var(--sp-region-rail-width)',
    backgroundColor: 'var(--sp-region-color)',
    pointerEvents: 'none',
  },
  '.sp-region-hover .sp-region-framing::before, .sp-region-hover .sp-region-framing::after': {
    height: '3.5px',
    backgroundColor: 'var(--sp-region-hover-color)',
  },
  '.sp-region-hover.sp-region-content': {
    '--sp-region-rail-width': `${regionRailMetrics.hoverWidth}px`,
  },
  '.sp-region-hover.sp-region-content::after': {
    backgroundColor: 'var(--sp-region-hover-color)',
  },
  '.sp-region-editing .sp-region-label': {
    visibility: 'hidden',
    width: 'var(--sp-region-title-width)',
    flex: '0 0 var(--sp-region-title-width)',
  },
  '.sp-region-editing .sp-region-framing': { gap: '8px' },
  '.sp-region-editing .sp-region-framing::after': { display: 'block' },
});

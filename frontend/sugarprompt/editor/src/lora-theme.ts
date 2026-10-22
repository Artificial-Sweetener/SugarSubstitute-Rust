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
/** Native LoRA bars share tapered geometry, literal labels and a stable numeric area. */
import { EditorView } from '@codemirror/view';

/** A 17px bar fits the default 19px row; native grid glyphs reserve the stable numeric slot independently of catalog paint. */
export const loraTheme = EditorView.theme({
  '.sp-lora-chip': {
    '--sp-lora-accent': 'rgb(107, 114, 128)',
    '--sp-lora-height': '17px',
    '--sp-lora-taper': 'clamp(4px, calc(var(--sp-lora-height) * .55), 14px)',
    display: 'inline-flex',
    position: 'relative',
    isolation: 'isolate',
    alignItems: 'center',
    boxSizing: 'border-box',
    minWidth: '120px',
    maxWidth: '540px',
    height: 'var(--sp-lora-height)',
    padding: '0 calc(var(--sp-lora-taper) + 7px)',
    verticalAlign: 'baseline',
    whiteSpace: 'nowrap',
    fontSize: '13px',
    lineHeight: 'var(--sp-lora-height)',
    textShadow: 'none',
  },
  '.sp-lora-backing': {
    position: 'absolute',
    inset: '0',
    zIndex: '-1',
    background: 'var(--sugarprompt-lora-surface, Canvas)',
    clipPath:
      'polygon(0 50%, var(--sp-lora-taper) 0, calc(100% - var(--sp-lora-taper)) 0, 100% 50%, calc(100% - var(--sp-lora-taper)) 100%, var(--sp-lora-taper) 100%)',
  },
  // One solid layer supplies fallback tint or the ready banner's readability wash.
  '.sp-lora-backing::before': {
    content: "''",
    position: 'absolute',
    inset: '0',
    zIndex: '1',
    background:
      'light-dark(rgb(from var(--sp-lora-accent) r g b / calc(32 / 255)), rgb(from var(--sp-lora-accent) r g b / calc(46 / 255)))',
  },
  // The even-odd hole preserves the existing one-pixel inset without putting a matte over ready artwork.
  '.sp-lora-backing::after': {
    content: "''",
    position: 'absolute',
    inset: '0',
    zIndex: '2',
    background: 'rgb(from var(--sp-lora-accent) r g b / calc(104 / 255))',
    clipPath:
      'polygon(evenodd, 0 50%, var(--sp-lora-taper) 0, calc(100% - var(--sp-lora-taper)) 0, 100% 50%, calc(100% - var(--sp-lora-taper)) 100%, var(--sp-lora-taper) 100%, 0 50%, 1px 50%, calc(var(--sp-lora-taper) + 1px) 1px, calc(100% - var(--sp-lora-taper) - 1px) 1px, calc(100% - 1px) 50%, calc(100% - var(--sp-lora-taper) - 1px) calc(100% - 1px), calc(var(--sp-lora-taper) + 1px) calc(100% - 1px), 1px 50%, 0 50%)',
  },
  '.sp-lora-active .sp-lora-backing::after': {
    background: 'rgb(from var(--sp-lora-accent) r g b / calc(156 / 255))',
  },
  '.sp-lora-initial': {
    position: 'absolute',
    inset: '0',
    display: 'grid',
    placeItems: 'center',
    color: 'rgb(from var(--sp-lora-accent) r g b / .28)',
    fontWeight: '700',
    pointerEvents: 'none',
    zIndex: '-1',
  },
  '.sp-lora-title': {
    flex: '1 1 auto',
    minWidth: '0',
    overflow: 'hidden',
    textOverflow: 'ellipsis',
  },
  '.sp-lora-media': {
    position: 'absolute',
    inset: '0',
    width: '100%',
    height: '100%',
    objectFit: 'cover',
    pointerEvents: 'none',
  },
  '.sp-lora-chip[data-media=ready] .sp-lora-backing': {
    background: 'transparent',
  },
  '.sp-lora-chip[data-media=ready] .sp-lora-backing::before': {
    background: 'rgba(0, 0, 0, calc(64 / 255))',
  },
  '.sp-lora-chip[data-media=ready] .sp-lora-initial': { display: 'none' },
  '.sp-lora-chip[data-media=ready]': {
    color: 'white',
  },
  // CSS lists foreground shadows first; reverse the donor painter's eight diffuse passes beneath its final directional pass.
  '.sp-lora-chip[data-media=ready] .sp-lora-title, .sp-lora-chip[data-media=ready] .sp-lora-separator, .sp-lora-chip[data-media=ready] .sp-lora-subtitle, .sp-lora-chip[data-media=ready] .sp-lora-weight':
    {
      textShadow:
        '1px 1px 0 rgba(0,0,0,calc(232 / 255)), 0.7px -0.7px 0 rgba(0,0,0,calc(168 / 255)), -0.7px 0.7px 0 rgba(0,0,0,calc(168 / 255)), 0.7px 0.7px 0 rgba(0,0,0,calc(168 / 255)), -0.7px -0.7px 0 rgba(0,0,0,calc(168 / 255)), 0 1px 0 rgba(0,0,0,calc(168 / 255)), 0 -1px 0 rgba(0,0,0,calc(168 / 255)), 1px 0 0 rgba(0,0,0,calc(168 / 255)), -1px 0 0 rgba(0,0,0,calc(168 / 255))',
    },
  '.sp-lora-chip[data-media=ready] .sp-lora-subtitle': {
    color: 'rgb(from currentColor r g b / calc(220 / 255))',
  },
  '.sp-lora-separator': { whiteSpace: 'pre', flexShrink: '0' },
  '.sp-lora-subtitle': {
    color: 'rgb(from currentColor r g b / calc(190 / 255))',
    flexShrink: '0',
  },
  '.sp-lora-weight': {
    '--sp-weight-cell-padding': '4px',
    display: 'inline-grid',
    boxSizing: 'border-box',
    flexShrink: '0',
    marginLeft: '8px',
    padding: '0 var(--sp-weight-cell-padding)',
    fontSize: '10px',
    fontWeight: '600',
    fontVariantNumeric: 'tabular-nums',
    lineHeight: 'normal',
    textAlign: 'center',
  },
  '.sp-lora-weight .sp-weight-value': { padding: '0' },
  '.sp-lora-weight::before': {
    content: "'-8.88'",
    gridArea: '1 / 1',
    whiteSpace: 'pre',
    visibility: 'hidden',
    pointerEvents: 'none',
    userSelect: 'none',
  },
  '.sp-lora-chip.sp-wheel-ready, .sp-lora-chip.sp-adjustment-content': {
    background: 'transparent',
  },
  '.sp-lora-chip.sp-wheel-ready .sp-lora-weight, .sp-lora-chip.sp-adjustment-content .sp-lora-weight':
    {
      background: 'rgb(from var(--sp-lora-accent) r g b / .20)',
      borderRadius: '2px',
    },
  '.sp-lora-chip[data-resolution=found]': {
    '--sp-lora-accent': 'rgb(0, 120, 212)',
  },
  // Resolve the owning host color before applying each donor layer's independent alpha.
  '.sp-lora-chip[data-resolution=missing], .sp-lora-chip[data-resolution=ambiguous]': {
    '--sp-lora-accent': 'var(--sugarprompt-error-foreground, #b42318)',
  },
  // Decorative initials must not become opaque system-color ink behind readable fallback labels.
  '@media (forced-colors: active)': {
    '.sp-lora-initial': { display: 'none' },
  },
});

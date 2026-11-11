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
/** Tooltip chrome styles only owned suggestion DOM; CodeMirror retains editable content and placement. */
import { EditorView } from '@codemirror/view';

/** Ordinary rows and continuous virtual model tiles share literal labels and native selection feedback. */
export const completionTheme = EditorView.baseTheme({
  '.cm-tooltip.cm-tooltip-autocomplete': {
    overflow: 'hidden',
    boxSizing: 'border-box',
    border: '1px solid light-dark(#00000011, #0000002d)',
    borderRadius: '8px',
    backgroundColor: 'light-dark(#f8f8f8, #282828)',
    color: 'light-dark(black, white)',
  },
  '.sp-completion-viewport': {
    overflowY: 'auto',
    overflowX: 'hidden',
    height: '100%',
    maxHeight: '10em',
    minWidth: 'min(258px, calc(100vw - 10px))',
    fontFamily: 'inherit',
    scrollbarWidth: 'none',
  },
  '.sp-completion-scrollbar': {
    display: 'block',
    position: 'sticky',
    top: '0',
    height: '0',
    width: '100%',
    margin: '0',
    padding: '0',
    border: '0',
    pointerEvents: 'none',
    zIndex: '3',
  },
  '.sp-completion-scroll-thumb': {
    position: 'absolute',
    right: '1px',
    width: '12px',
    minHeight: '0',
    pointerEvents: 'auto',
    touchAction: 'none',
    cursor: 'default',
  },
  '.sp-completion-scroll-thumb::before': {
    content: '""',
    position: 'absolute',
    right: '3px',
    top: '0',
    width: '3px',
    height: '100%',
    borderRadius: '3px',
    backgroundColor: 'light-dark(#00000072, #ffffff8b)',
    transition: 'width 150ms ease 200ms',
    pointerEvents: 'none',
  },
  '.sp-completion-scroll-thumb:hover::before': { width: '6px' },
  '.sp-completion-content': { position: 'relative' },
  '.cm-tooltip-autocomplete:not(.sp-completion-wall) .sp-completion-viewport': {
    maxHeight: 'min(338px, var(--sp-completion-inner-height, 338px))',
  },
  '.cm-tooltip.cm-tooltip-autocomplete:not(.sp-completion-wall)': {
    border: '0',
    padding: '0 1px',
    boxShadow: 'inset 0 0 0 1px light-dark(#00000011, #0000002d)',
  },
  '.cm-tooltip-autocomplete:not(.sp-completion-wall) .sp-completion-content': {
    paddingTop: '3px',
    paddingBottom: '5px',
    boxSizing: 'border-box',
  },
  '.sp-completion-row': {
    position: 'relative',
    display: 'grid',
    gridTemplateColumns: 'minmax(0, 1fr) auto',
    alignItems: 'center',
    columnGap: '8px',
    height: '33px',
    boxSizing: 'border-box',
    padding: '0 16px',
    fontSize: '14px',
    whiteSpace: 'nowrap',
    cursor: 'pointer',
  },
  '.sp-completion-row .cm-completionDetail': {
    maxWidth: '160px',
    overflow: 'hidden',
    textOverflow: 'ellipsis',
    textAlign: 'right',
    position: 'relative',
    zIndex: '1',
    color: 'inherit',
    fontSize: '12px',
  },
  '.sp-completion-row::before': {
    content: '""',
    position: 'absolute',
    inset: '4px 6px 0',
    borderRadius: '5px',
    pointerEvents: 'none',
  },
  '.sp-completion-row[aria-selected=true]::before': {
    backgroundColor: 'light-dark(#00000012, #ffffff14)',
  },
  '.sp-completion-row:not([aria-selected=true]):hover::before': {
    backgroundColor: 'light-dark(#00000017, #ffffff14)',
  },
  '.sp-completion-row .cm-completionLabel': {
    position: 'relative',
    zIndex: '1',
    overflow: 'hidden',
    textOverflow: 'ellipsis',
  },
  '.cm-tooltip-autocomplete.sp-completion-wall': {
    padding: '2px 0 6px',
    width: 'min(560px, calc(100vw - 8px))',
    maxWidth: 'calc(100vw - 8px)',
    height: 'min(630px, calc(100vh - 16px), var(--sp-completion-outer-height, 630px))',
  },
  '.sp-completion-wall .sp-completion-viewport': {
    backgroundColor: 'light-dark(#ffffff, #202020)',
    maxHeight: 'min(630px, var(--sp-completion-inner-height, 630px))',
    minWidth: '0',
    position: 'relative',
  },
  '.sp-completion-tile': {
    position: 'absolute',
    boxSizing: 'border-box',
    overflow: 'hidden',
    border: '0',
    backgroundColor: 'light-dark(#ffffffaa, #ffffff0d)',
    cursor: 'pointer',
  },
  '.sp-completion-tile[aria-selected=true]::after': {
    boxShadow: 'inset 0 0 0 2px light-dark(#0067c0dc, #299cffeb)',
  },
  '.sp-completion-tile:not([aria-selected=true]):hover': {
    outline: '1px solid light-dark(#00000050, #ffffff5a)',
    outlineOffset: '-2px',
  },
  '.sp-completion-tile::after': {
    content: '""',
    position: 'absolute',
    inset: '0',
    pointerEvents: 'none',
    background: 'linear-gradient(transparent, #0000000c 42%, #000000b2)',
  },
  '.sp-completion-tile[aria-selected=true]::after, .sp-completion-tile:hover::after': {
    background: 'linear-gradient(transparent, #00000018 42%, #000000dc)',
  },
  '.sp-completion-backing, .sp-completion-backing canvas': {
    position: 'absolute',
    width: '100%',
    height: '100%',
    inset: '0',
    objectFit: 'cover',
  },
  '.sp-completion-initial': {
    position: 'absolute',
    inset: '0',
    display: 'grid',
    placeItems: 'center',
    color: 'light-dark(#00000096, #ffffff96)',
    fontSize: 'inherit',
  },
  '.sp-completion-tile[data-media=ready] .sp-completion-initial': {
    display: 'none',
  },
  // The native 19px font metric is centered in 21px areas, separated by 22px.
  '.sp-completion-tile .cm-completionLabel': {
    position: 'absolute',
    bottom: '26px',
    left: '8px',
    right: '8px',
    lineHeight: '21px',
    fontSize: '14px',
    zIndex: '1',
    color: '#ffffffeb',
    overflow: 'hidden',
    textOverflow: 'ellipsis',
    whiteSpace: 'nowrap',
  },
  '.sp-completion-tile[data-subtitle=false] .cm-completionLabel': {
    bottom: '5px',
  },
  '.sp-completion-title-text': {
    display: 'inline',
    whiteSpace: 'nowrap',
  },
  '.sp-completion-tile .cm-completionLabel[data-marquee=scroll]': {
    textOverflow: 'clip',
  },
  '.sp-completion-tile .cm-completionLabel[data-marquee=scroll] .sp-completion-title-text': {
    display: 'inline-block',
  },
  '.sp-completion-tile .cm-completionLabel[data-marquee=end]': {
    direction: 'rtl',
    textAlign: 'right',
  },
  '.sp-completion-tile[data-subtitle=false] .cm-completionDetail': {
    display: 'none',
  },
  '.sp-completion-tile .cm-completionDetail': {
    position: 'absolute',
    bottom: '4px',
    left: '8px',
    right: '8px',
    margin: '0',
    lineHeight: '21px',
    fontSize: '14px',
    zIndex: '1',
    color: '#ffffffaf',
    overflow: 'hidden',
    textOverflow: 'ellipsis',
    whiteSpace: 'nowrap',
  },
});

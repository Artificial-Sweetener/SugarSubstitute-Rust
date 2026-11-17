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

/** Word rows carry literal labels and native selection feedback; model candidates use the host's own surface. */
export const completionTheme = EditorView.baseTheme({
  '.cm-tooltip.cm-tooltip-autocomplete': {
    overflow: 'hidden',
    boxSizing: 'border-box',
    border: '0',
    padding: '0 1px',
    boxShadow: 'inset 0 0 0 1px light-dark(#00000011, #0000002d)',
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
  '.cm-tooltip-autocomplete .sp-completion-viewport': {
    maxHeight: 'min(338px, var(--sp-completion-inner-height, 338px))',
  },
  '.cm-tooltip-autocomplete .sp-completion-content': {
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
  // The host's picker gallery brings its whole surface: the wrapper adds no panel chrome and only
  // bounds the surface to the space the caret line leaves.
  '.cm-tooltip.sp-model-completions': {
    display: 'flex',
    flexDirection: 'column',
    boxSizing: 'border-box',
    maxWidth: 'calc(100vw - 8px)',
    maxHeight: 'var(--sp-completion-outer-height, none)',
    padding: '0',
    border: '0',
    borderRadius: '0',
    background: 'none',
    boxShadow: 'none',
  },
  '.sp-model-completions > *': { minHeight: '0', maxHeight: '100%' },
});

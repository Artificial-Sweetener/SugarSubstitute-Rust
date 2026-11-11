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
/** Native editor appearance is prepared separately from its editing lifetime. */
import { EditorView } from '@codemirror/view';

/** One application-supplied semantic foreground owns every native error decoration. */
const semanticErrorForeground = 'var(--sugarprompt-error-foreground, #b42318)';
/** Error underlines share the same prepared host color as semantic text foreground. */
const semanticErrorUnderline = Object.freeze({
  textDecoration: 'underline wavy ' + semanticErrorForeground,
  textUnderlineOffset: '3px',
});

/** Native line planes share the same reviewed content-edge geometry; their role-specific stacking remains independently explicit. */
const sourceLinePlane = {
  content: "''",
  position: 'absolute',
  pointerEvents: 'none',
  left: 'calc(-12px - var(--sugarprompt-content-inset, 0px))',
  right: '-7px',
  top: '0',
  bottom: '0',
};

/** Shared native editable surface and emphasis appearance. */
export const editorTheme = EditorView.theme({
  '&': {
    height: '100%',
    cursor: 'text',
    fontSize: '14px',
    fontFamily: "'Segoe UI', sans-serif",
  },
  '.cm-scroller': { overflow: 'auto', fontFamily: 'inherit' },
  '.cm-content': {
    // Retain the document inset when the host provides no control content padding.
    padding: '6px 7px 6px 12px',
    paddingLeft: 'calc(12px + var(--sugarprompt-content-inset, 0px))',
    lineHeight: '19px',
    color: 'inherit',
  },
  '.cm-line': { padding: '0' },
  // CodeMirror's zero-width widget buffer is a bare `img` that places the caret beside a widget and
  // gives empty lines their measurable box. Host pages and user stylesheets restyle bare images
  // (`height: auto`, `display: none`, `max-height: 0`), which collapses that box and makes vertical
  // motion skip every empty line, so the metrics CodeMirror depends on are pinned against them.
  '.cm-widgetBuffer': {
    display: 'inline !important',
    position: 'static !important',
    float: 'none !important',
    width: '0 !important',
    minWidth: '0 !important',
    maxWidth: 'none !important',
    height: '1em !important',
    minHeight: '0 !important',
    maxHeight: 'none !important',
    margin: '0 !important',
    padding: '0 !important',
    border: '0 !important',
    verticalAlign: 'text-top !important',
  },
  '.cm-content::selection, .cm-content ::selection': {
    backgroundColor: 'Highlight',
    color: 'HighlightText',
  },
  '.sp-emphasis-accent .sp-emphasis-parenthesis': {
    color: 'light-dark(#0067c0, #299cff)',
  },
  '.sp-emphasis-parenthesis::selection': { color: 'HighlightText' },
  '.sp-source-line, .sp-scene-fill': { position: 'relative' },
  '.sp-canonical-row-edge': {
    display: 'inline-block',
    width: '0',
    height: '0',
    padding: '0',
    margin: '0',
    border: '0',
    fontSize: '0',
    lineHeight: '0',
    pointerEvents: 'none',
  },
  '.sp-canonical-row-top': { verticalAlign: 'top' },
  '.sp-canonical-row-bottom': { verticalAlign: 'bottom' },
  '.sp-region-row-paint': { position: 'absolute', pointerEvents: 'none' },
  '.sp-scene-fill-paint': {
    position: 'absolute',
    '--sp-scene-fill': 'light-dark(#00000008, #ffffff0a)',
    backgroundColor: 'var(--sp-scene-fill)',
  },
  '.sp-scene-marker': {
    display: 'inline-block',
    width: '0',
    overflow: 'hidden',
  },
  '.sp-scene-title': { fontWeight: '700' },
  '&[data-document-diagnostic-source="ready"] .sp-unsupported-scene': semanticErrorUnderline,
  '&[data-diagnostic-source="ready"] .sp-scene-title[data-scene-error="true"]': {
    color: semanticErrorForeground,
  },
  '.sp-source-line::before': { ...sourceLinePlane, zIndex: '-1' },
  '.sp-source-alternate::before': {
    backgroundColor: 'var(--sugarprompt-line-alternate, #0000000c)',
  },
  '.sp-source-current::before': {
    backgroundImage:
      'linear-gradient(var(--sugarprompt-line-active, #4f6df522), var(--sugarprompt-line-active, #4f6df522))',
  },
  '.sp-emphasis-prefix': { paddingRight: '2.5px' },
  '.sp-emphasis-suffix': { paddingLeft: '2.5px', whiteSpace: 'nowrap' },
  '.sp-emphasis-parenthesis': {
    fontSize: 'max(7px, calc(1em - 3px))',
    // Native Qt resolves its Medium Segoe UI request to the Regular face.
    fontWeight: '400',
    verticalAlign: '1px',
    lineHeight: 'normal',
  },
  '.sp-emphasis-weight': {
    display: 'inline-grid',
    isolation: 'isolate',
    textAlign: 'center',
    fontSize: 'max(6px, calc(1em - 4px))',
    fontWeight: '600',
    fontVariantNumeric: 'tabular-nums',
    verticalAlign: 'top',
    marginLeft: '1px',
    lineHeight: 'normal',
  },
  '.sp-weight-value': {
    gridArea: '1 / 1',
    padding: '0 .5px',
    whiteSpace: 'pre',
  },
  '.sp-wildcard-prefix': { paddingRight: '1px' },
  '&[data-diagnostic-source="ready"] .sp-wildcard-missing[data-diagnostic-active="false"], &[data-diagnostic-source="ready"] .sp-duplicate-segment':
    semanticErrorUnderline,
  '.sp-wildcard-suffix': { paddingLeft: '1px', whiteSpace: 'nowrap' },
  '.sp-spelling-diagnostic[data-diagnostic-active="false"]': semanticErrorUnderline,
  // StyleModule splits every comma before scoping, so feedback selectors stay flat.
  '.sp-wildcard-prefix.sp-wheel-ready:not([data-resolution="missing"]), .sp-wildcard-prefix.sp-adjustment-content:not([data-resolution="missing"]), .sp-wildcard-suffix.sp-wheel-ready:not([data-resolution="missing"]), .sp-wildcard-suffix.sp-adjustment-content:not([data-resolution="missing"])':
    {
      color: 'light-dark(#0067c0b6, #299cffcc)',
    },
  '.sp-wildcard-prefix[data-resolution="missing"], .sp-wildcard-suffix[data-resolution="missing"]':
    {
      color: semanticErrorForeground,
    },
  '.sp-wildcard-brace::selection, .sp-wildcard-tag::selection': {
    color: 'HighlightText',
  },
  '.sp-wildcard-brace': {
    fontSize: 'max(8px, calc(1em - 1px))',
    fontWeight: '600',
    lineHeight: 'normal',
  },
  '.sp-wildcard-tag': {
    fontSize: 'max(8px, calc(1em - 3px))',
    fontStyle: 'italic',
    fontWeight: '400',
    lineHeight: 'normal',
  },
});

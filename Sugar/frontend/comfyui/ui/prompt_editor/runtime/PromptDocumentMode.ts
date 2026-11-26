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
 * A prompt editor showing a whole text file, as the full-window wildcard and list editor does:
 * its document kind, the features it offers, and the line-number gutter beside its lines. The
 * editor itself paints the alternate-line and current-line bands; the gutter numbers them.
 */
import { featureDefinitions } from '../../../../sugarprompt/services/src/index.js';
import type { FeatureId, FeaturePreferences } from '../../../../sugarprompt/services/src/index.js';
import type { PromptDocumentKind } from '../../../../sugarprompt/core/src/index.js';
import type { SourceLineSnapshot } from '../../../../sugarprompt/editor/src/source-line-contracts.js';

/** A wildcard file, or a list of tags whose editor offers tag suggestions and nothing else. */
export type PromptDocument = 'wildcard-txt' | 'wildcard-csv' | 'tag-list';

/** The document kind the editor's semantics read a file as. */
export function promptDocumentKind(document: PromptDocument): PromptDocumentKind {
  return document === 'tag-list' ? 'prompt' : document;
}

/** Every feature off: a tag list is plain lines, completed from tags only. */
const TAG_LIST_PREFERENCES = (preferences: FeaturePreferences): FeaturePreferences => ({
  ...preferences,
  allowed: Object.fromEntries(featureDefinitions.map(({ id }) => [id, false])) as Record<
    FeatureId,
    boolean
  >,
});

/** The preferences a document's editor follows. */
export function promptDocumentPreferences(
  document: PromptDocument,
  preferences: FeaturePreferences,
): FeaturePreferences {
  return document === 'tag-list' ? TAG_LIST_PREFERENCES(preferences) : preferences;
}

/** Number the visible source lines in a strip the editor leaves free at its left edge. */
export class PromptLineGutter {
  readonly #strip: HTMLDivElement;
  readonly #frame: HTMLElement;
  readonly #measure: HTMLSpanElement;
  #digits = 0;
  #inset = 0;

  /** Draw the strip over `frame`, the element the editor's scroller fills. */
  constructor(frame: HTMLElement) {
    const document = frame.ownerDocument;
    this.#frame = frame;
    this.#strip = document.createElement('div');
    this.#strip.className = 'sc-prompt-line-gutter';
    this.#strip.setAttribute('aria-hidden', 'true');
    this.#measure = document.createElement('span');
    this.#measure.className = 'sc-prompt-line-gutter__measure';
    this.#strip.append(this.#measure);
    frame.append(this.#strip);
  }

  /**
   * The strip's width for `lineCount` lines: numbers are padded to at least two digits, as the
   * Python editor padded them, so the strip widens only at 100 lines and each power of ten.
   */
  inset(lineCount: number): number {
    const digits = Math.max(2, String(Math.max(1, lineCount)).length);
    if (digits !== this.#digits) {
      this.#digits = digits;
      this.#measure.textContent = '0'.repeat(digits);
      this.#inset = Math.ceil(this.#measure.getBoundingClientRect().width) + 12;
      this.#strip.style.width = `${String(this.#inset)}px`;
    }
    return this.#inset;
  }

  /** Redraw the numbers of the lines `snapshot` shows. */
  paint(snapshot: SourceLineSnapshot, scroller: HTMLElement | null): void {
    if (snapshot.status !== 'ready' || !scroller) return;
    const offset = scroller.getBoundingClientRect().top - this.#frame.getBoundingClientRect().top;
    const rows = snapshot.lines.map((line) => {
      const row = this.#frame.ownerDocument.createElement('div');
      row.className = 'sc-prompt-line-gutter__row';
      if (line.lineIndex % 2 === 1) row.classList.add('is-alternate');
      if (line.lineIndex === snapshot.currentLineIndex && snapshot.focusActive)
        row.classList.add('is-current');
      row.style.top = `${String(offset + line.top / snapshot.scaleY)}px`;
      row.style.height = `${String(line.height / snapshot.scaleY)}px`;
      row.textContent = String(line.lineIndex + 1).padStart(this.#digits, '0');
      return row;
    });
    this.#strip.replaceChildren(this.#measure, ...rows);
  }

  dispose(): void {
    this.#strip.remove();
  }
}

/** The gutter's look: muted numbers, the current one in the theme colour, shaded like its line. */
export const promptLineGutterCss = [
  '.sc-prompt-document {',
  '  --sugarprompt-line-alternate: light-dark(#0000000c, #ffffff10);',
  '  --sugarprompt-line-active: color-mix(in srgb, var(--sc-accent-indicator, var(--sc-brand-border, Highlight)) 15%, transparent);',
  '}',
  '.sc-prompt-line-gutter {',
  '  position: absolute;',
  '  inset: 0 auto 0 0;',
  '  overflow: hidden;',
  '  pointer-events: none;',
  '  border-right: 1px solid light-dark(#0000001a, #ffffff1a);',
  '  font: 12px/1 ui-monospace, "Cascadia Mono", Consolas, monospace;',
  '  color: light-dark(rgb(118 118 118), rgb(176 176 176));',
  '  z-index: 1;',
  '}',
  '.sc-prompt-line-gutter__measure { position: absolute; visibility: hidden; }',
  '.sc-prompt-line-gutter__row {',
  '  position: absolute;',
  '  left: 0;',
  '  right: 0;',
  '  display: flex;',
  '  justify-content: center;',
  '  padding-top: 3px;',
  '  box-sizing: border-box;',
  '}',
  '.sc-prompt-line-gutter__row.is-alternate { background: var(--sugarprompt-line-alternate); }',
  '.sc-prompt-line-gutter__row.is-current {',
  '  background: var(--sugarprompt-line-active);',
  '  color: var(--sc-accent-indicator, var(--sc-brand-border, Highlight));',
  '}',
].join('\n');

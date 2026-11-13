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
/** Index original parsed spans once per derivation without reparsing source or retaining historical authority. */
import type { SourceRange } from '../offsets.js';
import type { EmphasisSpan, LoraSpan, PromptDocument } from '../prompt/contracts.js';

interface IndexedLora {
  readonly span: LoraSpan;
  readonly order: number;
}

/** Borrow canonical identities through bounded exact-shell and contained-LoRA lookups. */
export class ReorderDerivationSpans {
  readonly #emphasis = new Map<number, Map<number, EmphasisSpan>>();
  readonly #loras: readonly IndexedLora[];

  /** Preserve first exact-shell identity and original LoRA enumeration order independently of query sorting. */
  constructor(document: PromptDocument) {
    for (const span of document.emphasisSpans) {
      let endings = this.#emphasis.get(span.outerRange.from);
      if (!endings) {
        endings = new Map<number, EmphasisSpan>();
        this.#emphasis.set(span.outerRange.from, endings);
      }
      if (!endings.has(span.outerRange.to)) endings.set(span.outerRange.to, span);
    }
    this.#loras = document.loraSpans
      .map((span, order) => ({ span, order }))
      .sort(
        (left, right) =>
          left.span.outerRange.from - right.span.outerRange.from || left.order - right.order,
      );
  }

  /** Resolve only a shell whose complete original endpoints match this trimmed segment. */
  emphasis(range: SourceRange): EmphasisSpan | undefined {
    return this.#emphasis.get(range.from)?.get(range.to);
  }

  /** Stop at the first contained original LoRA without scanning unrelated earlier source. */
  hasLora(range: SourceRange): boolean {
    for (let index = this.#first(range.from); index < this.#loras.length; index++) {
      const entry = this.#loras[index];
      if (!entry || entry.span.outerRange.from > range.to) break;
      if (entry.span.outerRange.to <= range.to) return true;
    }
    return false;
  }

  /** Enumerate contained original spans in exactly the parser's order, including overlap precedence. */
  loras(range: SourceRange): readonly LoraSpan[] {
    const matches: IndexedLora[] = [];
    for (let index = this.#first(range.from); index < this.#loras.length; index++) {
      const entry = this.#loras[index];
      if (!entry || entry.span.outerRange.from > range.to) break;
      if (entry.span.outerRange.to <= range.to) matches.push(entry);
    }
    return matches.sort((left, right) => left.order - right.order).map((entry) => entry.span);
  }

  /** Find the first candidate source start in logarithmic time. */
  #first(position: number): number {
    let from = 0,
      to = this.#loras.length;
    while (from < to) {
      const middle = (from + to) >>> 1;
      if ((this.#loras[middle]?.span.outerRange.from ?? Infinity) < position) from = middle + 1;
      else to = middle;
    }
    return from;
  }
}

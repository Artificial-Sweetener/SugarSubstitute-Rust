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
/** Native range indexes retain authoring provenance while mapping only metadata touched by edits. */
import { RangeSet, RangeValue, type Range, type Transaction } from '@codemirror/state';

/** A generated shell's observed boundaries use native logical UTF-16. */
export interface GeneratedShell {
  readonly from: number;
  readonly to: number;
  readonly contentFrom: number;
  readonly contentTo: number;
  readonly nestingDepth: number;
}
/** Explicit user escape intent covers one raw comma-delimited native interval. */
export interface LiteralIntent {
  readonly from: number;
  readonly to: number;
}
/** Indexed values store content offsets relative to their shell, so unrelated edits reuse actual values. */
export class ShellValue extends RangeValue {
  readonly contentFrom: number;
  readonly contentTo: number;
  readonly nestingDepth: number;
  override startSide = 1;
  override endSide = -1;
  /** Prepare one immutable shell value from native observed boundaries. */
  constructor(shell: GeneratedShell) {
    super();
    this.contentFrom = shell.contentFrom - shell.from;
    this.contentTo = shell.contentTo - shell.from;
    this.nestingDepth = shell.nestingDepth;
    Object.freeze(this);
  }
  /** Structural equivalence compares the complete provenance payload. */
  override eq(other: RangeValue): boolean {
    return (
      other instanceof ShellValue &&
      this.contentFrom === other.contentFrom &&
      this.contentTo === other.contentTo &&
      this.nestingDepth === other.nestingDepth
    );
  }
}
/** Literal ranges carry no independent mutable payload. */
export class LiteralValue extends RangeValue {
  override startSide = 1;
  override endSide = -1;
}
const literalValue = Object.freeze(new LiteralValue());
/** Metadata is authoritative user intent/provenance, never another source or selection buffer. */
export interface Authorship {
  readonly generated: RangeSet<ShellValue>;
  readonly literals: RangeSet<LiteralValue>;
}
/** Prepare canonical indexes outside native input, with one range per generated shell or manual segment. */
export function preparedAuthorship(
  generated: readonly GeneratedShell[],
  literals: readonly LiteralIntent[],
): Authorship {
  return Object.freeze({
    generated: RangeSet.of(
      generated.map((shell) => new ShellValue(shell).range(shell.from, shell.to)),
      true,
    ),
    literals: RangeSet.of(
      literals.map((intent) => literalValue.range(intent.from, intent.to)),
      true,
    ),
  });
}
/** Observe complete generated shell boundaries only during semantic preparation or inspection. */
export function generatedShells(value: Authorship): readonly GeneratedShell[] {
  const shells: GeneratedShell[] = [];
  for (const cursor = value.generated.iter(); cursor.value; cursor.next())
    shells.push(
      Object.freeze({
        from: cursor.from,
        to: cursor.to,
        contentFrom: cursor.from + cursor.value.contentFrom,
        contentTo: cursor.from + cursor.value.contentTo,
        nestingDepth: cursor.value.nestingDepth,
      }),
    );
  return Object.freeze(shells);
}
/** Observe manual intervals outside native input. */
export function literalIntents(value: Authorship): readonly LiteralIntent[] {
  const intents: LiteralIntent[] = [];
  for (const cursor = value.literals.iter(); cursor.value; cursor.next())
    intents.push(Object.freeze({ from: cursor.from, to: cursor.to }));
  return Object.freeze(intents);
}
/** Query only intersecting indexed intervals when native input checks explicit escape intent. */
export function hasLiteralIntent(value: Authorship, from: number, to: number): boolean {
  let protectedRange = false;
  value.literals.between(from, to, (start, end) => {
    if (start <= from && to <= end) {
      protectedRange = true;
      return false;
    }
  });
  return protectedRange;
}
/** Map only touched generated payloads; syntax and authored weight edits release generated identity. */
function changedShell(
  from: number,
  to: number,
  value: ShellValue,
  transaction: Transaction,
): Range<ShellValue> | null {
  const contentFrom = from + value.contentFrom,
    contentTo = from + value.contentTo;
  let valid = true;
  transaction.changes.iterChanges((start, end) => {
    if (start === end && (start === from || start === to)) return;
    if (end <= from || start >= to) return;
    if (start < contentFrom || end > contentTo) valid = false;
  });
  if (!valid) return null;
  const mappedFrom = transaction.changes.mapPos(from, 1),
    mappedTo = transaction.changes.mapPos(to, -1);
  const mapped = new ShellValue({
    from: mappedFrom,
    to: mappedTo,
    contentFrom: transaction.changes.mapPos(contentFrom, -1),
    contentTo: transaction.changes.mapPos(contentTo, 1),
    nestingDepth: value.nestingDepth,
  });
  return (value.eq(mapped) ? value : mapped).range(mappedFrom, mappedTo);
}
/** Use CodeMirror's indexed mapping and limited filters instead of scanning every token on each input transaction. */
export function mappedAuthorship(value: Authorship, transaction: Transaction): Authorship {
  if (!transaction.docChanged || (!value.generated.size && !value.literals.size)) return value;
  const touched = new Map<ShellValue, { readonly from: number; readonly to: number }>();
  const removedLiterals = new Set<string>();
  let affected = false;
  let filterFrom = transaction.newDoc.length,
    filterTo = 0;
  transaction.changes.iterChanges((from, to, a, b, inserted) => {
    /** A change after every indexed range leaves the actual prepared output reusable. */
    const noteAffected = (_start: number, end: number): void | false => {
      if (end > from) {
        affected = true;
        return false;
      }
    };
    value.generated.between(from, transaction.startState.doc.length, noteAffected);
    value.literals.between(from, transaction.startState.doc.length, noteAffected);
    filterFrom = Math.min(filterFrom, a);
    filterTo = Math.max(filterTo, b);
    value.generated.between(from, to, (start, end, shell) => {
      touched.set(shell, { from: start, to: end });
    });
    const commaChanged =
      inserted.toString().includes(',') ||
      transaction.startState.doc.sliceString(from, to).includes(',');
    value.literals.between(from, to, (start, end) => {
      if ((from <= start && end <= to) || commaChanged)
        removedLiterals.add(
          `${transaction.changes.mapPos(start, 1)}:${transaction.changes.mapPos(end, -1)}`,
        );
    });
  });
  if (!affected) return value;
  const removedShells = new Set<ShellValue>(),
    additions: Range<ShellValue>[] = [];
  for (const [shell, range] of touched) {
    const mapped = changedShell(range.from, range.to, shell, transaction);
    if (!mapped || mapped.value !== shell) {
      removedShells.add(shell);
      if (mapped) additions.push(mapped);
    }
  }
  let generated = value.generated.map(transaction.changes),
    literals = value.literals.map(transaction.changes);
  if (removedShells.size)
    generated = generated.update({
      filterFrom,
      filterTo,
      filter: (_from, _to, shell) => !removedShells.has(shell),
      add: additions,
      sort: true,
    });
  if (removedLiterals.size)
    literals = literals.update({
      filterFrom,
      filterTo,
      filter: (from, to) => !removedLiterals.has(`${from}:${to}`),
    });
  return generated === value.generated && literals === value.literals
    ? value
    : Object.freeze({ generated, literals });
}

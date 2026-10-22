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
/** Native marks and derived widgets render genuine regional and punctuated carriers without owning source. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth } from '@codemirror/commands';
import { WidgetType } from '@codemirror/view';
import { ReorderWidget } from '../../../../../../frontend/sugarprompt/editor/src/reorder-widget.ts';
import { publicationFixture, hideReorderPublication } from './reorder-publication-fixture.ts';
import { key } from './reorder-session-fixture.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import {
  reorderPreview,
  installReorderPreview,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import {
  valueReorderLayout,
  emptyValueReorderPartitions,
  hideReorderChip,
  serializeReorderPreview,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import {
  encodeSource,
  sourceToDocument,
} from '../../../../../../frontend/sugarprompt/core/src/line-endings.ts';

/** Literal constructor boundary records only nodes authored by the genuine CodeMirror widget. */
class LiteralElement {
  readonly ownerDocument: Document;
  readonly parts: (string | LiteralElement)[] = [];
  readonly dataset: Record<string, string> = {};
  readonly attributes: Record<string, string> = {};
  /** Native inline CSS writes are retained literally; this constructor transport does not resolve browser style or layout. */
  readonly style: Record<string, string> = {};
  className = '';
  contentEditable = '';
  textContent: string | null = '';
  /** Bind the existing external document whose constructor is controlled for this owner proof. */
  constructor(document: Document) {
    this.ownerDocument = document;
  }
  /** Keep literal source append operations, without interpreting or creating managed editor content. */
  append(...parts: (string | LiteralElement)[]): void {
    for (const part of parts) {
      if (typeof part !== 'string') {
        part.remove();
        part.parent = this;
      }
      this.parts.push(part);
    }
  }
  parent: LiteralElement | null = null;
  /** Native sibling inspection observes the exact literal children already retained by this transport. */
  get childNodes(): readonly (string | LiteralElement)[] {
    return this.parts;
  }
  /** Move one actual existing sibling, preserving every other literal child and occurrence. */
  insertBefore<T extends LiteralElement>(node: T, reference: LiteralElement | null): T {
    if (node === reference) return node;
    node.remove();
    const at = reference === null ? this.parts.length : this.parts.indexOf(reference);
    if (at < 0) throw new Error('literal-insert-reference-missing');
    node.parent = this;
    this.parts.splice(at, 0, node);
    return node;
  }
  /** Native retirement detaches only this exact parent membership. */
  remove(): void {
    const at = this.parent?.parts.indexOf(this) ?? -1;
    if (at >= 0) this.parent!.parts.splice(at, 1);
    this.parent = null;
  }
  /** Native target attributes use the same literal data-attribute adaptation as an external DOM node. */
  /** Native visibility metadata reads actual literal attributes. */
  getAttribute(name: string): string | null {
    return this.attributes[name] ?? null;
  }
  /** Native removal retires the exact literal attribute only. */
  removeAttribute(name: string): void {
    Reflect.deleteProperty(this.attributes, name);
  }
  setAttribute(name: string, value: string): void {
    this.attributes[name] = value;
    if (name.startsWith('data-'))
      this.dataset[
        name.slice(5).replace(/-([a-z])/gu, (_match, letter: string) => letter.toUpperCase())
      ] = value;
  }
}

for (const ending of ['\n', '\r\n'])
  for (const emptyRegion of [false, true])
    for (const hidden of [false, true])
      test(`genuine shared regional carrier renders every partition-owned blank identity, ending=${ending.length}, empty=${emptyRegion}, hidden=${hidden}`, () => {
        const source = [
            'a',
            '',
            ...(emptyRegion ? ['[SEP|Empty]'] : []),
            '[SEP|Face]',
            '',
            'b, c',
          ].join(ending),
          encoded = encodeSource(source),
          transport = publicationFixture(
            source,
            sourceToDocument(encoded.logical, encoded.endings, source.indexOf('b, c')),
          ),
          { f } = transport,
          document = f.view.dom.ownerDocument,
          createElement = document.createElement;
        try {
          f.owner.keydown(key('Alt'));
          transport.settle();
          const held = f.state.field(reorderPreview).value!.order.order[1]!;
          if (hidden) {
            const current = f.state.field(reorderPreview).value!,
              order = hideReorderChip(current.capture.prepared, current.order, held)!;
            assert.ok(order);
            const output = serializeReorderPreview(current.capture.prepared, order)!;
            assert.ok(output);
            f.view.dispatch({
              effects: installReorderPreview.of({ ...current, order, output }),
            });
          }
          const published = f.state.field(reorderPreview).value!,
            layout = valueReorderLayout(published.order),
            shared = layout.gaps.filter((gap) => gap.carrier === 'slot' && gap.slot === 0);
          assert.equal(shared.length, 2);
          assert.deepEqual(
            shared.map((gap) => gap.partition),
            [0, emptyRegion ? 2 : 1],
          );
          document.createElement = (() =>
            new LiteralElement(document)) as unknown as Document['createElement'];
          const field = f.state.field(reorderPreview),
            gaps: { gap: number; blank: number; position: number }[] = [],
            partitions: { partition: number; position: number }[] = [];
          let literal = '';
          /** Observe only genuine literal nodes; both target kinds are source-neutral. */
          const observe = (node: LiteralElement): void => {
            if (node.dataset.reorderGap !== undefined) {
              gaps.push({
                gap: Number(node.dataset.reorderGap),
                blank: Number(node.dataset.reorderBlank),
                position: literal.length,
              });
              return;
            }
            if (node.dataset.reorderPartition !== undefined) {
              partitions.push({
                partition: Number(node.dataset.reorderPartition),
                position: literal.length,
              });
              return;
            }
            if (node.className !== 'sp-reorder-native-empty') literal += node.textContent ?? '';
            for (const part of node.parts) {
              if (typeof part === 'string') literal += part;
              else observe(part);
            }
          };
          if (hidden) {
            assert.equal(field.native, false);
            const widget: unknown = field.decorations.iter().value!.spec.widget;
            assert.ok(widget instanceof WidgetType);
            observe(widget.toDOM(f.view) as unknown as LiteralElement);
          } else {
            assert.equal(field.native, true);
            // Marks retain native text; only genuine zero-width target widgets are mounted here.
            literal = readExactSource(f.state);
            const mapping = published.capture.target.entry.semanticMapping!;
            field.decorations.between(0, f.state.doc.length, (from, to, decoration) => {
              const widget: unknown = decoration.spec.widget;
              if (widget === undefined) return;
              assert.ok(widget instanceof WidgetType);
              assert.equal(from, to);
              const position = mapping.logicalToDecoded[from - mapping.logicalRange.from];
              assert.ok(position !== null && position !== undefined);
              const rendered = widget.toDOM(f.view) as unknown as LiteralElement;
              assert.equal(rendered.className, 'sp-reorder-native-empty');
              assert.equal(rendered.textContent, '\u200b');
              assert.equal(rendered.parts.length, 1);
              const target = rendered.parts[0];
              assert.ok(target instanceof LiteralElement);
              assert.equal(target.className, 'sp-reorder-empty');
              assert.equal(target.textContent, '');
              assert.equal(target.parts.length, 0);
              if (target.dataset.reorderGap !== undefined) {
                assert.equal(target.dataset.reorderPartition, undefined);
                gaps.push({
                  gap: Number(target.dataset.reorderGap),
                  blank: Number(target.dataset.reorderBlank),
                  position,
                });
              } else {
                assert.notEqual(target.dataset.reorderPartition, undefined);
                assert.equal(target.dataset.reorderBlank, undefined);
                partitions.push({
                  partition: Number(target.dataset.reorderPartition),
                  position,
                });
              }
            });
          }
          /** Canonical serialized chip ranges locate each carrier without parsing rendered text. */
          const start = (carrier: 'prefix' | 'slot' | 'suffix', slot: number): number =>
            carrier === 'prefix'
              ? 0
              : published.output.renderedRanges[
                  published.order.order[
                    carrier === 'slot' ? slot : published.order.order.length - 1
                  ]!
                ]!.to;
          assert.equal(literal, published.output.text);
          assert.deepEqual(
            gaps,
            layout.gaps
              .flatMap((gap) =>
                gap.blankOffsets.map((offset, blank) => ({
                  gap: gap.index,
                  blank,
                  position: start(gap.carrier, gap.slot) + offset,
                })),
              )
              .sort((a, b) => a.position - b.position),
          );
          assert.deepEqual(
            partitions,
            emptyValueReorderPartitions(published.capture.prepared, published.order)
              .map((partition) => ({
                partition: partition.partition,
                position: start(partition.carrier, partition.slot) + partition.offset,
              }))
              .sort((a, b) => a.position - b.position),
          );
          assert.equal(readExactSource(f.state), source);
          assert.equal(undoDepth(f.state), 0);
        } finally {
          document.createElement = createElement;
          f.destroy();
        }
      });

test('genuine projected widget renders exact punctuated carrier bytes and current gap targets', () => {
  const transport = publicationFixture('a\nb,  \n', 2),
    { f } = transport,
    document = f.view.dom.ownerDocument,
    createElement = document.createElement;
  try {
    f.owner.keydown(key('Alt'));
    transport.settle();
    f.owner.keydown(key('ArrowUp'));
    const collision = hideReorderPublication(transport);
    document.createElement = (() =>
      new LiteralElement(document)) as unknown as Document['createElement'];
    const widget: unknown = new ReorderWidget(collision, null);
    assert.ok(widget instanceof WidgetType);
    const rendered = widget.toDOM(f.view) as unknown as LiteralElement,
      markers: { gap: number; blank: number; position: number }[] = [];
    let source = '';
    /** Walk literal owned nodes, excluding only their source-neutral zero-width target content. */
    const observe = (node: LiteralElement): void => {
      if (node.dataset.reorderGap !== undefined) {
        markers.push({
          gap: Number(node.dataset.reorderGap),
          blank: Number(node.dataset.reorderBlank),
          position: source.length,
        });
        return;
      }
      if (node.className !== 'sp-reorder-native-empty') source += node.textContent ?? '';
      for (const part of node.parts) {
        if (typeof part === 'string') source += part;
        else observe(part);
      }
    };
    observe(rendered);
    assert.equal(source, 'a\n,  \n');
    assert.deepEqual(markers, [
      { gap: 0, blank: 0, position: 1 },
      { gap: 0, blank: 1, position: 2 },
      { gap: 0, blank: 2, position: 6 },
    ]);
    assert.equal(readExactSource(f.state), 'a\nb,  \n');
    assert.equal(undoDepth(f.state), 0);
  } finally {
    document.createElement = createElement;
    f.destroy();
  }
});

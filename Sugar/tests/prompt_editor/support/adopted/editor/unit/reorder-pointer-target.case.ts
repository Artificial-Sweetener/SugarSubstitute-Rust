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
/** Actual capture semantics constrain native/current-scope pointer fallback independently of measured hit precedence. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { captureReorder, reorderState } from '../transaction/reorder-fixture.ts';
import { readReorderPointerTarget } from '../../../../../../frontend/sugarprompt/editor/src/reorder-pointer-target.ts';

/** Only the native instanceof/closest/attribute surface is substituted; Core membership remains actual prepared output. */
class PointerElement {
  readonly dataset: { readonly reorderChip: string };
  private readonly value: string | null;
  /** Literal numeric metadata is supplied independently by each external DOM specimen. */
  constructor(id: string, value: string | null) {
    this.value = value;
    this.dataset = { reorderChip: id };
  }
  /** This specimen is its own closest semantic chip boundary. */
  closest(): this {
    return this;
  }
  /** Presence reflects only the supplied physical value metadata. */
  hasAttribute(name: string): boolean {
    return this.getAttribute(name) !== null;
  }
  /** Native membership reads the external physical scope key without semantic derivation in the fixture. */
  getAttribute(name: string): string | null {
    return name === 'data-reorder-native-value' ? this.value : null;
  }
}
/** This nominal external event boundary contains only the independently specified target reference. */
function eventFor(target: PointerElement): Pick<PointerEvent, 'target'> {
  return { target: target as unknown as Element };
}

test('pointer target admits only current native scope IDs while preserving measured and canonical target precedence', () => {
  const capture = captureReorder(reorderState('a, b, c').state),
    previous = Object.getOwnPropertyDescriptor(globalThis, 'Element'),
    causes: unknown[] = [];
  try {
    Object.defineProperty(globalThis, 'Element', {
      configurable: true,
      value: PointerElement,
    });
    assert.equal(
      readReorderPointerTarget(capture, eventFor(new PointerElement('1', '0')), () => null),
      1,
    );
    assert.equal(
      readReorderPointerTarget(capture, eventFor(new PointerElement('1', '1')), () => null),
      null,
    );
    assert.equal(
      readReorderPointerTarget(capture, eventFor(new PointerElement('2', null)), () => null),
      2,
    );
    assert.equal(
      readReorderPointerTarget(capture, eventFor(new PointerElement('1', '1')), () => 0),
      0,
    );
    assert.equal(
      readReorderPointerTarget(capture, eventFor(new PointerElement('3', '0')), () => null),
      null,
    );
    assert.equal(
      readReorderPointerTarget(capture, eventFor(new PointerElement('1', '0')), () => Number.NaN),
      null,
    );
  } catch (cause) {
    causes.push(cause);
  }
  try {
    if (previous) Object.defineProperty(globalThis, 'Element', previous);
    else Reflect.deleteProperty(globalThis, 'Element');
  } catch (cause) {
    causes.push(cause);
  }
  if (causes.length === 1) throw causes[0];
  if (causes.length) throw new AggregateError(causes, 'pointer-target-owner-retirement-failed');
});

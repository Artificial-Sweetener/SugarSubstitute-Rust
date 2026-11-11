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
import { reorderPaintStyleFixture } from './reorder-paint-style-fixture.ts';
/** Native proof owns glyph and raster equivalence; this boundary proves resolved visual output and omitted CSS traversal. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  prepareReorderPaintStyle,
  prepareReorderWrapperPaintStyle,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-paint-style.ts';

test('resolved glyph, box, theme color and SVG paint survive without enumerating browser properties', () => {
  const values = new Map([
    ['font', 'italic 600 14px Georgia'],
    ['font-feature-settings', '"liga" 0'],
    ['font-variation-settings', '"wght" 630'],
    ['color', 'rgb(11, 22, 33)'],
    ['color-scheme', 'dark'],
    ['border-top', '1px solid rgb(44, 55, 66)'],
    ['width', '42.375px'],
    ['white-space', 'pre-wrap'],
    ['grid-template', 'none / 1fr auto'],
    ['fill', 'rgb(11, 22, 33)'],
    ['background-image', 'url("https://example.invalid/private.png")'],
  ]);
  const copied = new Map<string, string>();
  // The nominal adaptation models only the computed/inline declaration boundary consumed by passive paint.
  const original = {
    get length(): never {
      throw new Error('unbounded-css-enumeration');
    },
    item(): never {
      throw new Error('unbounded-css-enumeration');
    },
    getPropertyValue(name: string): string {
      return values.get(name) ?? '';
    },
  } as unknown as CSSStyleDeclaration;
  const destination = reorderPaintStyleFixture(copied) as unknown as CSSStyleDeclaration;
  destination.cssText = prepareReorderPaintStyle(original).cssText;
  assert.deepEqual(copied, new Map([...values, ['background-image', 'none']]));
  assert.equal(copied.has('animation'), false);
});

test('unexpected style failures remain observable without reporting complete paint', () => {
  const failure = new Error('controlled-native-style-failure');
  const original = {
    getPropertyValue(): never {
      throw failure;
    },
  } as unknown as CSSStyleDeclaration;
  const destination = {
    set cssText(_value: string) {
      throw new Error('write-after-read-failure');
    },
  } as unknown as CSSStyleDeclaration;
  assert.throws(() => (destination.cssText = prepareReorderPaintStyle(original).cssText), failure);
});

test('an unserializable font shorthand preserves explicit typography and pseudo-element theme inputs', () => {
  const values = new Map([
    ['font-family', '"Georgia", serif'],
    ['font-size', '17.5px'],
    ['font-weight', '630'],
    ['font-style', 'italic'],
    ['font-stretch', '87.5%'],
    ['--sp-lora-accent', 'rgb(0, 120, 212)'],
    ['--sp-lora-height', '21px'],
  ]);
  const copied = new Map<string, string>();
  const original = {
    getPropertyValue(name: string): string {
      return values.get(name) ?? '';
    },
  } as unknown as CSSStyleDeclaration;
  const destination = reorderPaintStyleFixture(copied) as unknown as CSSStyleDeclaration;
  destination.cssText = prepareReorderPaintStyle(original).cssText;
  assert.deepEqual(copied, values);
  assert.equal(copied.has('font'), false);
});

/** The prepared CSSOM serialization is published atomically without property-by-property mutation. */
test('resolved declaration batch preserves quoted CSSOM serialization in one write', () => {
  const values: Readonly<Record<string, string>> = {
    'font-family': '"A; B\\" C", serif',
    color: 'rgb(11, 22, 33)',
    'background-image': 'url("https://example.invalid/private.png")',
    '--sp-region-color': '44, 55, 66',
    '--sp-region-hover-color': '77, 88, 99',
    '--sp-region-rail-width': '3.5px',
  };
  let writes = 0,
    serialized = '';
  const original = {
    getPropertyValue: (name: string) => values[name] ?? '',
  } as unknown as CSSStyleDeclaration;
  const destination = {
    set cssText(value: string) {
      ++writes;
      serialized = value;
    },
    setProperty(): never {
      throw new Error('per-property-copy-unexpected');
    },
  } as unknown as CSSStyleDeclaration;
  destination.cssText = prepareReorderPaintStyle(original).cssText;
  assert.equal(writes, 1);
  assert.equal(
    serialized,
    `background-image:none;color:rgb(11, 22, 33);font-family:${values['font-family']};--sp-region-color:44, 55, 66;--sp-region-hover-color:77, 88, 99;--sp-region-rail-width:3.5px;`,
  );
});

/** A failed native batch setter remains the primary failure at the style boundary. */
test('batch write failure remains raw and quoted values cannot split declarations', () => {
  const cause = new Error('controlled-cssom-batch-write'),
    original = { getPropertyValue: () => '' } as unknown as CSSStyleDeclaration,
    destination = {
      set cssText(_value: string) {
        throw cause;
      },
    } as unknown as CSSStyleDeclaration;
  assert.throws(
    () => (destination.cssText = prepareReorderPaintStyle(original).cssText),
    (error) => error === cause,
  );
  const properties = new Map<string, string>(),
    style = reorderPaintStyleFixture(properties);
  style.cssText = 'font-family:"A; B\\" C", serif;color:rgb(11, 22, 33);';
  assert.deepEqual(
    properties,
    new Map([
      ['font-family', '"A; B\\" C", serif'],
      ['color', 'rgb(11, 22, 33)'],
    ]),
  );
  style.cssText = 'width:42px;';
  assert.deepEqual(properties, new Map([['width', '42px']]));
  assert.equal(style['font-family'], '');
});

/** Opacity comes from the original single resolved-property read, including masked zero and missing-value fallback. */
test('prepared style freezes opacity from the same original serialized read', () => {
  for (const opacity of ['0', '0.73', '']) {
    let reads = 0;
    const original = {
      getPropertyValue(name: string) {
        if (name === 'opacity') {
          ++reads;
          return opacity;
        }
        return '';
      },
    } as unknown as CSSStyleDeclaration;
    const prepared = prepareReorderPaintStyle(original);
    assert.equal(reads, 1);
    assert.equal(prepared.opacity, opacity || '1');
    assert.equal(prepared.cssText, opacity ? `opacity:${opacity};` : '');
    assert.equal(Object.isFrozen(prepared), true);
  }
});

test('canonical wrappers preserve inline text-fill intent and priority without freezing inherited currentColor', () => {
  const resolved = {
    getPropertyValue(name: string): string {
      if (name === '-webkit-text-fill-color') throw new Error('wrapper-read-resolved-text-fill');
      return name === 'color' ? 'rgb(0, 0, 0)' : name === 'opacity' ? '0' : '';
    },
  } as unknown as CSSStyleDeclaration;
  for (const [value, priority] of [
    ['', ''],
    ['currentColor', ''],
    ['rgb(0, 0, 0)', ''],
    ['rgb(12, 34, 56)', ''],
    ['var(--host-fill)', 'important'],
    ['currentColor', 'important'],
  ] as const) {
    const authored = reorderPaintStyleFixture();
    authored.setProperty('-webkit-text-fill-color', value, priority);
    const prepared = prepareReorderWrapperPaintStyle(
        resolved,
        authored as unknown as CSSStyleDeclaration,
        '0.73',
      ),
      copied = reorderPaintStyleFixture();
    copied.cssText = prepared.cssText;
    assert.equal(copied.getPropertyValue('-webkit-text-fill-color'), value);
    assert.equal(copied.getPropertyPriority('-webkit-text-fill-color'), priority);
    assert.equal(copied.getPropertyValue('color'), 'rgb(0, 0, 0)');
    assert.equal(prepared.opacity, '0.73');
    assert.equal(copied.getPropertyValue('opacity'), '0.73');
  }
});

test('inline text-fill absence, value and priority are distinct prepared wrapper reuse operands', () => {
  const resolved = {
    getPropertyValue: () => '',
  } as unknown as CSSStyleDeclaration;
  const authored = reorderPaintStyleFixture(),
    outputs: string[] = [];
  for (const [value, priority] of [
    ['', ''],
    ['currentColor', ''],
    ['rgb(0, 0, 0)', ''],
    ['rgb(0, 0, 0)', 'important'],
  ] as const) {
    authored.setProperty('-webkit-text-fill-color', value, priority);
    outputs.push(
      prepareReorderWrapperPaintStyle(resolved, authored as unknown as CSSStyleDeclaration).cssText,
    );
  }
  assert.equal(new Set(outputs).size, outputs.length);
});

test('resolved atomic snapshots retain exact computed fill independently of wrapper inheritance policy', () => {
  const computed = {
    getPropertyValue(name: string): string {
      return name === '-webkit-text-fill-color'
        ? 'rgb(0, 0, 0)'
        : name === 'color'
          ? 'rgb(255, 255, 255)'
          : '';
    },
  } as unknown as CSSStyleDeclaration;
  const copied = reorderPaintStyleFixture();
  copied.cssText = prepareReorderPaintStyle(computed).cssText;
  assert.equal(copied.getPropertyValue('color'), 'rgb(255, 255, 255)');
  assert.equal(copied.getPropertyValue('-webkit-text-fill-color'), 'rgb(0, 0, 0)');
});

test('raw inline text-fill access failures remain observable before wrapper serialization', () => {
  for (const target of ['value', 'priority'] as const) {
    const cause = new Error('controlled-inline-text-fill-failure');
    const authored = {
      getPropertyValue(): string {
        if (target === 'value') throw cause;
        return 'currentColor';
      },
      getPropertyPriority(): string {
        throw cause;
      },
    } as unknown as CSSStyleDeclaration;
    const computed = {
      getPropertyValue(): never {
        throw new Error('read-after-inline-failure');
      },
    } as unknown as CSSStyleDeclaration;
    assert.throws(
      () => prepareReorderWrapperPaintStyle(computed, authored),
      (actual) => actual === cause,
    );
  }
});

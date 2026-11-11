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
/** Canonical normal and readonly LoRA constructors preserve primary separators independently of secondary versions. */
import assert from 'node:assert/strict';
import test from 'node:test';
import type { EditorView } from '@codemirror/view';
import { LoraWidget } from '../../../../../../frontend/sugarprompt/editor/src/lora-widget.ts';
import { SourceToken } from '../../../../../../frontend/sugarprompt/editor/src/projection-widgets.ts';
import { ReadonlyWidgetMedia } from '../../../../../../frontend/sugarprompt/editor/src/readonly-widget-media.ts';
import { readLoraResources } from '../../../../../../frontend/sugarprompt/services/src/lora/validation.ts';

/** Literal DOM transport retains actual production children and never reproduces label segmentation. */
class LabelElement {
  className = '';
  textContent = '';
  title = '';
  readonly dataset: Record<string, string> = {};
  readonly attributes = new Map<string, string>();
  readonly children: LabelElement[] = [];
  /** Retain actual accessible attributes without interpreting label text. */
  setAttribute(name: string, value: string): void {
    this.attributes.set(name, value);
  }
  /** Production constructors alone choose the literal child order. */
  append(...children: LabelElement[]): void {
    this.children.push(...children);
  }
}

for (const specimen of [
  { subtitle: null, expected: '' },
  { subtitle: '   ', expected: '' },
  { subtitle: ' Version A ', expected: 'Version A' },
])
  test(`normal and readonly label segments preserve version input length${specimen.subtitle?.length ?? 0}`, () => {
    const [resource] = readLoraResources(
      [
        {
          name: 'segmented',
          resolution: 'found',
          model: {
            promptName: 'segmented',
            backendValue: 'segmented.safetensors',
            basename: 'segmented',
          },
          modelPageUrl: null,
          title: 'Model',
          subtitle: specimen.subtitle,
          triggerWords: [],
          thumbnail: { status: 'unavailable' },
        },
      ],
      ['segmented'],
    );
    assert.ok(resource);
    const previous = Object.getOwnPropertyDescriptor(globalThis, 'document'),
      media = new ReadonlyWidgetMedia(() => true),
      failures: unknown[] = [];
    try {
      Object.defineProperty(globalThis, 'document', {
        configurable: true,
        value: { createElement: () => new LabelElement() },
      });
      const widget = new LoraWidget(new SourceToken(), resource, '0.50');
      assert.equal(widget.subtitleLabel, specimen.expected);
      for (const readonly of [false, true]) {
        const chip = (readonly
          ? widget.toReadonlyDOM({} as EditorView, media)
          : widget.toDOM({} as EditorView)) as unknown as LabelElement;
        assert.deepEqual(
          chip.children.map((child) => child.className),
          [
            'sp-lora-backing',
            'sp-lora-initial',
            'sp-lora-title',
            ...(specimen.expected ? ['sp-lora-separator'] : []),
            'sp-lora-subtitle',
            'sp-lora-weight',
          ],
        );
        assert.equal(
          chip.children.find((child) => child.className === 'sp-lora-title')?.textContent,
          'Model',
        );
        assert.equal(
          chip.children.find((child) => child.className === 'sp-lora-subtitle')?.textContent,
          specimen.expected,
        );
        const separator = chip.children.find((child) => child.className === 'sp-lora-separator');
        assert.equal(separator?.textContent, specimen.expected ? ' - ' : undefined);
        const weight = chip.children.find((child) => child.className === 'sp-lora-weight');
        assert.equal(weight?.children[0]?.textContent, '0.50');
      }
    } catch (cause) {
      failures.push(cause);
    }
    for (const release of [
      () => media.destroy(),
      () => {
        if (previous) Object.defineProperty(globalThis, 'document', previous);
        else Reflect.deleteProperty(globalThis, 'document');
      },
    ])
      try {
        release();
      } catch (cause) {
        failures.push(cause);
      }
    if (failures.length === 1) throw failures[0];
    if (failures.length) throw new AggregateError(failures, 'lora-label-proof-retirement');
  });

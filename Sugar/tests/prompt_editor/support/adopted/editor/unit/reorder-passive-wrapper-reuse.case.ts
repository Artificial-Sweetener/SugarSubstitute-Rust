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
/** Actual Stage and canonical media owners distinguish discarded wrapper inputs from retained paint. Native CSS cascade/layout is proved separately. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { preparePassiveMotion } from '../../../../../../frontend/sugarprompt/editor/src/reorder-passive-context.ts';
import { ReorderPassiveStage } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-stage.ts';
import { motionPresentationFixture } from './reorder-motion-presentation-fixture.ts';
import { preparedLoraState } from './reorder-passive-reuse-fixture.ts';
import { preparePassiveWrapperStyles } from '../../../../../../frontend/sugarprompt/editor/src/reorder-passive-wrapper-style.ts';

/** The existing native transport exposes resolved styles without changing literal DOM appearance stamps. */
type MotionFixture = ReturnType<typeof motionPresentationFixture>;
/** Only the fixture's external CSSOM operands are controlled; canonical rendering and reuse stay production-owned. */
type NativeNode = Parameters<MotionFixture['document']['defaultView']['getComputedStyle']>[0];

/** Compose real prepared rasters, canonical widgets and Stage with one finite external computed-style boundary. */
function wrapperFixture() {
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'document'),
    painted: HTMLCanvasElement[] = [];
  let acquired: MotionFixture | null = null,
    acquiredStage: ReorderPassiveStage | null = null;
  /** Independent teardown also covers failures before the complete fixture is returned. */
  const destroy = (): void => {
    const causes: unknown[] = [];
    for (const release of [
      () => acquiredStage?.destroy(),
      () => acquired?.destroy(),
      () => {
        if (previous) Object.defineProperty(globalThis, 'document', previous);
        else Reflect.deleteProperty(globalThis, 'document');
      },
    ])
      try {
        release();
      } catch (cause) {
        causes.push(cause);
      }
    if (causes.length === 1) throw causes[0];
    if (causes.length) throw new AggregateError(causes, 'wrapper-reuse-retirement-failed');
  };
  try {
    const f = motionPresentationFixture(
      preparedLoraState((canvas) => painted.push(canvas)),
      (document) =>
        Object.defineProperty(globalThis, 'document', {
          configurable: true,
          value: document,
        }),
    );
    acquired = f;
    const read = f.document.defaultView.getComputedStyle.bind(f.document.defaultView),
      changes = new Map<NativeNode, Map<string, string>>();
    f.document.defaultView.getComputedStyle = (node) => {
      // Native CSSOM returns resolved zero geometry even when the declaration was not authored.
      const original = read(node),
        wrapper = node === f.content || node === f.scroller || node === f.line,
        resolved = new Map<string, string>(
          wrapper
            ? [
                ['margin', '0px'],
                ['padding', '0px'],
              ]
            : [],
        );
      for (const [name, value] of changes.get(node) ?? []) resolved.set(name, value);
      if (!resolved.size) return original;
      return {
        ...original,
        ...Object.fromEntries(resolved),
        getPropertyValue: (name: string) => resolved.get(name) ?? original.getPropertyValue(name),
      };
    };
    const prepared = preparePassiveMotion(f.view, 0);
    assert.ok(prepared);
    const stage = new ReorderPassiveStage(f.view, prepared);
    acquiredStage = stage;
    const first = stage.mount(prepared);
    assert.ok(first);
    const shell = first.node,
      children = [...shell.querySelectorAll('[data-reorder-chip]')].map(
        (node) => node.childNodes[0],
      ),
      rich = [...shell.querySelectorAll('.sp-lora-chip')],
      buffers = [...shell.querySelectorAll<HTMLCanvasElement>('.sp-lora-media')],
      styles = [...shell.querySelectorAll('.cm-scroller, .cm-content, .cm-line')].map((node) =>
        node.getAttribute('style'),
      ),
      paintCount = painted.length;
    assert.equal(buffers.length, 1);
    assert.ok(children.length > 0 && children.every((node) => node !== undefined));
    assert.ok(buffers[0]!.width > 0 && buffers[0]!.height > 0);
    first.destroy();
    return {
      f,
      stage,
      prepared,
      shell,
      styles,
      /** Resolved CSS changes do not silently rewrite native source/selection or authority. */
      change(node: NativeNode, values: Readonly<Record<string, string>>): void {
        changes.set(node, new Map(Object.entries(values)));
      },
      /** Actual resource identities and paint callbacks prove reuse rather than a reconstruction counter. */
      expectReused(): void {
        const next = preparePassiveMotion(f.view, 0);
        assert.ok(next);
        assert.equal(next.context.key === prepared.context.key, true);
        const second = stage.mount(next);
        assert.ok(second);
        assert.equal(second.node === shell, true);
        const nextChildren = [...shell.querySelectorAll('[data-reorder-chip]')].map(
            (node) => node.childNodes[0],
          ),
          nextRich = [...shell.querySelectorAll('.sp-lora-chip')],
          nextBuffers = [...shell.querySelectorAll('.sp-lora-media')];
        assert.equal(nextChildren.length, children.length);
        assert.equal(nextRich.length, rich.length);
        assert.equal(nextBuffers.length, buffers.length);
        nextChildren.forEach((node, index) => assert.equal(node === children[index], true));
        nextRich.forEach((node, index) => assert.equal(node === rich[index], true));
        nextBuffers.forEach((node, index) => assert.equal(node === buffers[index], true));
        assert.equal(painted.length, paintCount);
        const nextStyles = [...shell.querySelectorAll('.cm-scroller, .cm-content, .cm-line')].map(
          (node) => node.getAttribute('style'),
        );
        assert.equal(JSON.stringify(nextStyles) === JSON.stringify(styles), true);
        assert.equal(second.current(), true);
        second.destroy();
      },
      destroy,
    };
  } catch (primary) {
    try {
      destroy();
    } catch (cleanup) {
      throw new AggregateError([primary, cleanup], 'wrapper-fixture-acquisition-failed');
    }
    throw primary;
  }
}

test('identical complete wrapper operands retain actual canonical paint without native allocation', () => {
  const fixture = wrapperFixture(),
    create = fixture.f.document.createElement;
  try {
    fixture.f.document.createElement = () => {
      throw new Error('UNEXPECTED_WARM_NATIVE_ALLOCATION');
    };
    fixture.expectReused();
  } finally {
    fixture.f.document.createElement = create;
    fixture.destroy();
  }
});

for (const layer of ['scroller', 'content', 'line'] as const)
  test(`resolved ${layer} used height preserves the same shell canonical children and decoded buffer`, () => {
    const fixture = wrapperFixture();
    try {
      const node = fixture.f[layer];
      fixture.change(node, { height: '37px' });
      // Native used dimensions and returned geometry change together, independently of the wrapper writer.
      Object.assign(node.box, { height: 37 });
      fixture.expectReused();
      const create = fixture.f.document.createElement;
      try {
        fixture.f.document.createElement = () => {
          throw new Error('UNEXPECTED_EQUIVALENT_INPUT_RENORMALIZATION');
        };
        fixture.expectReused();
      } finally {
        fixture.f.document.createElement = create;
      }
    } finally {
      fixture.destroy();
    }
  });

for (const layer of ['scroller', 'content'] as const)
  test(`discarded ${layer} width margin and padding retain exact canonical output identities`, () => {
    const fixture = wrapperFixture();
    try {
      fixture.change(fixture.f[layer], {
        width: '901px',
        margin: '11px 12px 13px 14px',
        padding: '15px 16px 17px 18px',
      });
      fixture.expectReused();
    } finally {
      fixture.destroy();
    }
  });

test('retained content color rejects idle output reuse while retaining the valid native Stage', () => {
  const fixture = wrapperFixture();
  try {
    fixture.change(fixture.f.content, { color: 'rgb(21, 32, 43)' });
    const next = preparePassiveMotion(fixture.f.view, 0);
    assert.ok(next);
    assert.equal(next.context.key === fixture.prepared.context.key, true);
    const second = fixture.stage.mount(next);
    assert.ok(second);
    assert.equal(second.node !== fixture.shell, true);
    assert.equal(fixture.shell.isConnected, false);
    assert.equal(
      second.node.querySelector<HTMLElement>('.cm-content')!.style.color,
      'rgb(21, 32, 43)',
    );
  } finally {
    fixture.destroy();
  }
});

for (const input of [
  { layer: 'content', name: 'font', value: '17px monospace' },
  { layer: 'line', name: 'padding-left', value: '3px' },
  { layer: 'line', name: 'border-top-width', value: '2px' },
] as const)
  test(`retained ${input.layer} ${input.name} invalidates the native coordinate context`, () => {
    const fixture = wrapperFixture();
    try {
      fixture.change(fixture.f[input.layer], { [input.name]: input.value });
      const next = preparePassiveMotion(fixture.f.view, 0);
      assert.ok(next);
      assert.equal(next.context.key !== fixture.prepared.context.key, true);
      assert.equal(fixture.stage.matches(next), false);
      assert.equal(fixture.stage.mount(next) === null, true);
    } finally {
      fixture.destroy();
    }
  });

test('prepared line-local fill variables survive normalization and invalidate only their own wrapper reuse operands', () => {
  const f = motionPresentationFixture();
  const scratch = f.document.createElement('span');
  try {
    const prepared = preparePassiveMotion(f.view, 0);
    assert.ok(prepared);
    const context = {
      ...prepared.context,
      content: [['--host-fill', 'rgb(1, 2, 3)']] as const,
      line: [['--host-fill', 'rgb(12, 34, 56)']] as const,
    };
    const styles = {
      ...prepared,
      lineCss: `${prepared.lineCss}-webkit-text-fill-color:var(--host-fill);`,
    };
    const first = preparePassiveWrapperStyles(
      f.document as unknown as Document,
      context,
      styles,
      'partial',
    );
    scratch.style.cssText = first.lineCss;
    assert.equal(scratch.style.getPropertyValue('-webkit-text-fill-color'), 'var(--host-fill)');
    assert.equal(scratch.style.getPropertyValue('--host-fill'), 'rgb(12, 34, 56)');
    scratch.style.cssText = first.contentCss;
    assert.equal(scratch.style.getPropertyValue('--host-fill'), 'rgb(1, 2, 3)');
    assert.equal(
      preparePassiveWrapperStyles(
        f.document as unknown as Document,
        context,
        styles,
        'partial',
        first,
      ),
      first,
    );
    assert.equal(first.policy, 'partial');
    const whole = preparePassiveWrapperStyles(
      f.document as unknown as Document,
      context,
      styles,
      'whole',
      first,
    );
    assert.notEqual(whole, first);
    assert.equal(whole.policy, 'whole');
    for (const css of [whole.contentCss, whole.lineCss]) {
      scratch.style.cssText = css;
      assert.equal(scratch.style.visibility, 'visible');
    }
    for (const css of [first.contentCss, first.lineCss]) {
      scratch.style.cssText = css;
      assert.equal(scratch.style.visibility, 'hidden');
    }
    assert.equal(whole.scrollerCss, first.scrollerCss);
    const create = f.document.createElement;
    try {
      f.document.createElement = () => {
        throw new Error('unexpected-whole-wrapper-renormalization');
      };
      assert.equal(
        preparePassiveWrapperStyles(
          f.document as unknown as Document,
          context,
          styles,
          'whole',
          whole,
        ),
        whole,
      );
    } finally {
      f.document.createElement = create;
    }
    const partial = preparePassiveWrapperStyles(
      f.document as unknown as Document,
      context,
      styles,
      'partial',
      whole,
    );
    assert.notEqual(partial, whole);
    assert.equal(partial.contentCss, first.contentCss);
    assert.equal(partial.lineCss, first.lineCss);
    const nextContext = {
      ...context,
      line: [['--host-fill', 'rgb(65, 43, 21)']] as const,
    };
    const next = preparePassiveWrapperStyles(
      f.document as unknown as Document,
      nextContext,
      styles,
      'partial',
      first,
    );
    assert.notEqual(next, first);
    assert.equal(next.contentCss, first.contentCss);
    assert.equal(next.scrollerCss, first.scrollerCss);
    scratch.style.cssText = next.lineCss;
    assert.equal(scratch.style.getPropertyValue('--host-fill'), 'rgb(65, 43, 21)');
    assert.equal(scratch.style.getPropertyValue('-webkit-text-fill-color'), 'var(--host-fill)');
  } finally {
    scratch.remove();
    f.destroy();
  }
});

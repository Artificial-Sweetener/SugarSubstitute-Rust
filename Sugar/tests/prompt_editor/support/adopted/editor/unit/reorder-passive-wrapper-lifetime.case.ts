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
/** Actual passive slots fence normalization reentry; detached CSSOM scratch retains raw failure ownership. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { redoDepth, undoDepth } from '@codemirror/commands';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { preparePassiveMotion } from '../../../../../../frontend/sugarprompt/editor/src/reorder-passive-context.ts';
import { ReorderPassiveStage } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-stage.ts';
import { preparePassiveWrapperStyles } from '../../../../../../frontend/sugarprompt/editor/src/reorder-passive-wrapper-style.ts';
import { motionPresentationFixture } from './reorder-motion-presentation-fixture.ts';

/** The existing fixture transports only external native declarations and allocation identities. */
type MotionFixture = ReturnType<typeof motionPresentationFixture>;
/** A scratch and canonical shell share the native element transport, without another rendering implementation. */
type NativeNode = ReturnType<MotionFixture['document']['createElement']>;

for (const warm of [false, true])
  for (const frontier of ['create', 'style', 'remove'] as const)
    test(`same-capture publication at scratch ${frontier} rejects ${warm ? 'warm' : 'cold'} paint before wrapper writes`, () => {
      const f = motionPresentationFixture();
      let stage: ReorderPassiveStage | null = null;
      try {
        const source = readExactSource(f.state),
          selection = f.state.selection,
          prepared = preparePassiveMotion(f.view, 0);
        assert.ok(prepared);
        let requested = prepared;
        if (warm) {
          const alternative = f.document.createElement('span');
          try {
            alternative.style.cssText = prepared.contentCss;
            alternative.style.setProperty('width', '1px');
            const contentCss = alternative.style.cssText;
            assert.equal(contentCss !== prepared.contentCss, true);
            requested = { ...prepared, contentCss };
          } finally {
            alternative.remove();
          }
        }
        stage = new ReorderPassiveStage(f.view, prepared);
        let shell: HTMLElement | null = null;
        if (warm) {
          const first = stage.mount(prepared);
          assert.ok(first);
          shell = first.node;
          first.destroy();
        }
        const create = f.document.createElement.bind(f.document);
        let scratch: NativeNode | null = null,
          allocations = 0,
          removals = 0,
          published = false,
          wrapperWrites = 0;
        /** One genuine readonly issue retains capture/token/source while superseding the exact pending publication. */
        const supersede = (): void => {
          if (published) return;
          published = true;
          f.publish();
        };
        if (shell)
          Object.defineProperty(shell.style, 'left', {
            configurable: true,
            set() {
              wrapperWrites++;
            },
          });
        f.document.createElement = (name) => {
          allocations++;
          const node = create(name);
          if (scratch) return node;
          assert.equal(name === 'span', true);
          scratch = node;
          const remove = node.remove.bind(node),
            declaration = Object.getOwnPropertyDescriptor(node.style, 'cssText');
          assert.ok(declaration?.set);
          const set = declaration.set;
          Object.defineProperty(node.style, 'cssText', {
            ...declaration,
            /** Only the real scratch declaration setter triggers the style-frontier publication. */
            set(value: string) {
              set.call(node.style, value);
              if (frontier === 'style') supersede();
            },
          });
          node.remove = () => {
            remove();
            removals++;
            if (frontier === 'remove') supersede();
          };
          if (frontier === 'create') supersede();
          return node;
        };
        // A complete native declaration with discarded width forces warm normalization before final-plan comparison.
        const child = stage.mount(requested);
        assert.equal(child === null, true);
        assert.equal(published, true);
        assert.equal(allocations, 1);
        assert.equal(removals, 1);
        assert.equal(wrapperWrites, 0);
        assert.equal(scratch !== null, true);
        assert.equal(f.view.dom.querySelectorAll('.sp-reorder-motion-paint').length, 0);
        assert.equal(readExactSource(f.state) === source, true);
        assert.equal(f.state.selection === selection, true);
        assert.equal(undoDepth(f.state), 0);
        assert.equal(redoDepth(f.state), 0);
        const next = preparePassiveMotion(f.view, 0);
        assert.ok(next);
        assert.equal(next.preview.capture === prepared.preview.capture, true);
        assert.equal(next.preview.token === prepared.preview.token, true);
        assert.equal(next.preview === prepared.preview, false);
      } finally {
        try {
          stage?.destroy();
        } finally {
          f.destroy();
        }
      }
    });

for (const primary of [null, undefined])
  for (const failure of ['none', 'null', 'undefined'] as const)
    test(`normalization keeps raw ${String(primary)} setter failure and ${failure} cleanup failure in ownership order`, () => {
      const f = motionPresentationFixture();
      try {
        const prepared = preparePassiveMotion(f.view, 0);
        assert.ok(prepared);
        const create = f.document.createElement.bind(f.document),
          cleanup = failure === 'null' ? null : undefined;
        let allocations = 0,
          removals = 0,
          detached = false;
        f.document.createElement = (name) => {
          allocations++;
          const node = create(name),
            remove = node.remove.bind(node);
          Object.defineProperty(node.style, 'cssText', {
            configurable: true,
            /** Unexpected native declaration failures preserve even null and undefined exactly. */
            set() {
              // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
              throw primary;
            },
          });
          node.remove = () => {
            detached = node.parentElement === null;
            removals++;
            remove();
            // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
            if (failure !== 'none') throw cleanup;
          };
          return node;
        };
        let failed = false,
          actual: unknown;
        try {
          preparePassiveWrapperStyles(
            f.view.dom.ownerDocument,
            prepared.context,
            prepared,
            'partial',
          );
        } catch (cause) {
          failed = true;
          actual = cause;
        }
        assert.equal(failed, true);
        assert.equal(allocations, 1);
        assert.equal(removals, 1);
        assert.equal(detached, true);
        if (failure === 'none') assert.equal(actual === primary, true);
        else {
          assert.equal(actual instanceof AggregateError, true);
          if (!(actual instanceof AggregateError)) assert.fail('normalization-aggregate-required');
          assert.equal(actual.errors.length, 2);
          assert.equal(actual.errors[0] === primary, true);
          assert.equal(actual.errors[1] === cleanup, true);
        }
        assert.equal(f.view.dom.querySelectorAll('.sp-reorder-motion-paint').length, 0);
        assert.equal(undoDepth(f.state), 0);
        assert.equal(redoDepth(f.state), 0);
      } finally {
        f.destroy();
      }
    });

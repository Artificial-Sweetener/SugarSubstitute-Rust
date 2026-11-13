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
/** Shared finite measured-refresh fixture delegates mutations to actual CM transactions. */
import { mock } from 'node:test';
import {
  Compartment,
  EditorState,
  type Transaction,
  type TransactionSpec,
} from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { SemanticRefresh } from '../../../../../../frontend/sugarprompt/editor/src/semantic-refresh.ts';
import { nativeConditioning } from '../../../../../../frontend/sugarprompt/editor/src/conditioning-state.ts';
import { documentDiagnostics } from '../../../../../../frontend/sugarprompt/editor/src/document-diagnostics.ts';
import { richProjectionExtension } from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { loraPresentation } from '../../../../../../frontend/sugarprompt/editor/src/lora-presentation.ts';
import { loraMedia } from '../../../../../../frontend/sugarprompt/editor/src/lora-media-state.ts';
import { sceneErrors } from '../../../../../../frontend/sugarprompt/editor/src/scene-errors.ts';
import {
  nativeInputSettlement,
  type NativeInputSettlement,
} from '../../../../../../frontend/sugarprompt/editor/src/input-settlement.ts';
import {
  defaultPreferences,
  resolveFeatureProfile,
} from '../../../../../../frontend/sugarprompt/services/src/index.ts';
import { featureProfile } from '../../../../../../frontend/sugarprompt/editor/src/features.ts';
import { documentKind } from '../../../../../../frontend/sugarprompt/editor/src/document-kind.ts';
import { structureState } from '../transaction/document-structure-fixture.ts';

/** Derive the supported request type from the public method; no private CodeMirror type is imported. */
type NativeMeasureRequest = NonNullable<Parameters<EditorView['requestMeasure']>[0]>;

/** A controlled callback record preserves actual measurement results. */
interface MeasuredJob {
  read(): void;
  write(): void;
}

/** An unmounted supported view boundary delegates every mutation to actual CM transactions. */
export function refreshFixture(
  kind: 'prompt' | 'wildcard-csv' = 'prompt',
  initialComposition = false,
  source = kind === 'prompt' ? '(soft:1.20), {one}' : 'h\r\n"{one}"',
) {
  const microtasks: (() => void)[] = [],
    background: (() => void)[] = [],
    jobs: MeasuredJob[] = [];
  const backgroundScheduling = mock.method(globalThis, 'setTimeout', (callback: () => void) => {
    background.push(callback);
    return 0;
  });
  const scheduling = mock.method(globalThis, 'queueMicrotask', (callback: () => void) => {
    microtasks.push(callback);
  });
  let remainingObservations = 0;
  let held = false,
    reentry: (() => void) | null = null;
  let composition = initialComposition,
    compositionReentry: (() => void) | null = null,
    remainingCompositionObservations = 0;
  let nativeCompositionReentry: (() => void) | null = null,
    remainingNativeCompositionObservations = 0;
  const settlement: NativeInputSettlement = {
    /** Controlled observations do not simulate trusted composition. */
    hasCompositionInput() {
      if (remainingCompositionObservations-- <= 0) {
        const callback = compositionReentry;
        compositionReentry = null;
        callback?.();
      }
      return composition;
    },
    /** No input resource exists in this state-only boundary. */
    cancelCompositionInput() {},
    /** One optional callback challenges publication after preparation. */
    hasHeldKey() {
      if (remainingObservations-- > 0) return held;
      const callback = reentry;
      reentry = null;
      callback?.();
      return held;
    },
    /** Semantic refresh never reserves a command; unexpected use fails observably. */
    reserveCommand() {
      throw new Error('fixture-unexpected-reservation');
    },
  };
  const profile = new Compartment(),
    inputSlot = new Compartment();
  const fixture = structureState(source, kind, [
    profile.of([]),
    nativeConditioning,
    documentDiagnostics,
    loraPresentation,
    loraMedia,
    sceneErrors,
    richProjectionExtension,
    inputSlot.of(nativeInputSettlement.of(settlement)),
  ]);
  let state = fixture.state,
    publications = 0;
  // Exact external test boundary: the prototype supplies nominal view identity, with no DOM/editor construction.
  const boundary: unknown = Object.create(EditorView.prototype);
  if (!(boundary instanceof EditorView)) throw new Error('fixture-view-boundary');
  const view = boundary;
  Object.defineProperties(view, {
    state: { get: () => state },
    compositionStarted: {
      /** A native composition getter may synchronously retire the captured view/state. */
      get() {
        if (remainingNativeCompositionObservations-- <= 0) {
          const callback = nativeCompositionReentry;
          nativeCompositionReentry = null;
          callback?.();
        }
        return false;
      },
    },
    dispatch: {
      value: (spec: TransactionSpec) => {
        state = state.update(spec).state;
        publications++;
      },
    },
    requestMeasure: {
      value: (request: NativeMeasureRequest) => {
        let measured: ReturnType<NativeMeasureRequest['read']>;
        jobs.push({
          /** Run only the owner's actual measured read callback. */
          read() {
            measured = request.read(view);
          },
          /** Run only the owner's actual measured write callback. */
          write() {
            request.write?.(measured, view);
          },
        });
      },
    },
  });
  const owner = new SemanticRefresh(view);
  return {
    owner,
    jobs,
    /** Expose actual state without another mutable source buffer. */
    state: () => state,
    /** Count supported publications, never parser calls. */
    publications: () => publications,
    /** Observe that CPU preparation yielded beyond the measurement frame's microtasks. */
    backgroundTasks: () => background.length,
    /** State-only update exercises source/context retirement between measured phases. */
    apply(transaction: Transaction) {
      const startState = state;
      state = transaction.state;
      owner.update({ startState });
    },
    /** Reconfigure actual permission/profile/kind inputs across a pending measured callback. */
    reconfigure(kind: 'permission' | 'profile' | 'kind-return' | 'input') {
      const startState = state;
      if (kind === 'input')
        state = state.update({
          effects: inputSlot.reconfigure(nativeInputSettlement.of({ ...settlement })),
        }).state;
      if (kind === 'permission')
        state = state.update({
          effects: fixture.permission.reconfigure(EditorState.readOnly.of(true)),
        }).state;
      if (kind === 'profile')
        state = state.update({
          effects: profile.reconfigure(
            featureProfile.of(resolveFeatureProfile(defaultPreferences, { mode: 'library' })),
          ),
        }).state;
      if (kind === 'kind-return') {
        state = state.update({
          effects: fixture.kindSlot.reconfigure(documentKind.of('wildcard-txt')),
        }).state;
        owner.update({ startState });
        const changed = state;
        state = state.update({
          effects: fixture.kindSlot.reconfigure(documentKind.of('prompt')),
        }).state;
        owner.update({ startState: changed });
        return;
      }
      owner.update({ startState });
    },
    /** Control the existing input owner's observation. */
    hold(value: boolean) {
      held = value;
    },
    /** Observe composition exclusively through the admitted input owner, without notifying the refresh event flag. */
    compose(value: boolean) {
      composition = value;
    },
    /** Challenge the authoritative composition observation with synchronous current-state retirement. */
    reenterComposition(callback: () => void, afterObservations = 0) {
      compositionReentry = callback;
      remainingCompositionObservations = afterObservations;
    },
    /** Challenge CodeMirror's native composition observation without creating another input owner. */
    reenterNativeComposition(callback: () => void, afterObservations = 0) {
      nativeCompositionReentry = callback;
      remainingNativeCompositionObservations = afterObservations;
    },
    /** Challenge an input observation with synchronous native-state reentry. */
    reenter(callback: () => void, afterObservations = 0) {
      reentry = callback;
      remainingObservations = afterObservations;
    },
    /** Drain a microtask first, otherwise execute the deferred background task without sleeping. */
    task() {
      const callback = microtasks.shift() ?? background.shift();
      if (!callback) throw new Error('fixture-task-missing');
      callback();
    },
    /** Dispose and restore the sole controlled global boundary. */
    destroy() {
      owner.destroy();
      scheduling.mock.restore();
      backgroundScheduling.mock.restore();
    },
  };
}

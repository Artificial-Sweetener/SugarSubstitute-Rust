import { editorPresentations } from '../../../EditorPresentationFixture.js';
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
/** Actual native host adoption proof uses CodeMirror state/history and an inert view transport without browser certification. */
import { EditorState, Transaction, type TransactionSpec } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';
import { createNativeState } from '../../../../../../frontend/sugarprompt/editor/src/native-state.ts';
import { NativeConfiguration } from '../../../../../../frontend/sugarprompt/editor/src/native-configuration.ts';
import { NativeEditingPermission } from '../../../../../../frontend/sugarprompt/editor/src/native-editing-permission.ts';
import { NativeHostUpdates } from '../../../../../../frontend/sugarprompt/editor/src/native-host-updates.ts';
import { historyTransactions } from '../../../../../../frontend/sugarprompt/editor/src/prompt-history.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import type { EditorOptions } from '../../../../../../frontend/sugarprompt/editor/src/editor-options.ts';
import type { NativeDocumentSeed } from '../../../../../../frontend/sugarprompt/editor/src/document-state.ts';

/** Only this fixture transports native immutable states; no test-owned editing or history implementation exists. */
export function hostFixture(
  source = 'old',
  readOnly = false,
  accepted?: EditorOptions['onHostConfigurationAccepted'],
) {
  const options: EditorOptions = {
    ...editorPresentations,
    source,
    readOnly,
    label: 'Test',
    placeholder: '',
    onUpdate: () => undefined,
  };
  const configuration = new NativeConfiguration(options),
    permission = new NativeEditingPermission(readOnly);
  let state: EditorState,
    revision = 0,
    alive = true;
  let callback: (() => void) | undefined;
  let publications = 0,
    focuses = 0,
    retirements = 0;
  const admissions: ('preserve-input' | 'replace-input' | undefined)[] = [];
  /** State construction is the production baseline owner with all real derived native fields. */
  function baseline(
    value: string,
    values = configuration.values,
    nextRevision = revision,
    permissionValue = state?.readOnly ?? readOnly,
    document?: NativeDocumentSeed,
  ): EditorState {
    return createNativeState(value, {
      ...(document ? { document } : {}),
      changeRichRendering: (enabled) => {
        updates.apply({
          mode: 'preserve',
          configuration: { richRendering: enabled },
        });
      },
      label: 'Test',
      placeholder: values.placeholder,
      configuration: [
        ...configuration.extensions(values, nextRevision),
        permission.extension(permissionValue),
      ],
      view: () => view,
      isAlive: () => alive,
      lexicon: () => configuration.values.tagLexicon,
      publish: () => undefined,
      update: () => undefined,
      onLayout: undefined,
      onSourceLines: undefined,
      revision: () => revision,
    });
  }
  state = baseline(source);
  const transport = {
    get state() {
      return state;
    },
    plugin: () => null,
    composing: false,
    compositionStarted: false,
    /** Production history mapping consumes the actual prepared transaction batch. */
    dispatch(input: Transaction | TransactionSpec) {
      const transaction = input instanceof Transaction ? input : state.update(input);
      state = historyTransactions([transaction]).at(-1)!.state;
    },
  };
  // The transport seam provides only state dispatch and composition admission; DOM/input contracts remain native proof.
  const view = transport as unknown as EditorView;
  const updates = new NativeHostUpdates({
    surface: {
      get view() {
        return view;
      },
      setState(value) {
        state = value;
        return value;
      },
      retireHandoff(admission) {
        retirements++;
        admissions.push(admission);
      },
      retireInput() {
        throw new Error('Native input retirement requires browser proof');
      },
      handoff() {
        return {
          restore() {
            focuses++;
            return Promise.resolve(true);
          },
          cancel() {},
        };
      },
    },
    configuration,
    permission,
    createBaseline: baseline,
    alive: () => alive,
    revision: () => revision,
    adoptBaseline: () => {
      revision++;
    },
    publish: () => {
      publications++;
      callback?.();
    },
    ...(accepted ? { accepted } : {}),
  });
  return {
    updates,
    configuration,
    permission,
    transport,
    state: () => state,
    source: () => readExactSource(state),
    revision: () => revision,
    counts: () => ({ publications, focuses, retirements }),
    /** Observe only the actual surface admission classification; source/selection/history remain native. */
    admissions: () => [...admissions],
    callback: (value: () => void) => {
      callback = value;
    },
    destroy: () => {
      alive = false;
    },
  };
}

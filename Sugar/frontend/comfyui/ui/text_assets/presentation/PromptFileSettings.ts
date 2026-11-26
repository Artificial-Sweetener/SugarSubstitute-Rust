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
/**
 * Prompt Editing's two file rows: Manage wildcards and Manage autocomplete lists. Each opens the
 * full-window editor over its files, with SugarCubes' own name and confirmation dialogs.
 */
import { ConfirmDialog } from '../../dialogs/ConfirmDialog.js';
import { InputModal } from '../../dialogs/InputModal.js';
import { settingsCatalogFor } from '../../settings/workspace/SettingsCatalog.js';
import { registerWildcardSettings } from '../../wildcards/presentation/WildcardSettings.js';
import type { TextAssetSource } from '../domain/TextAsset.js';
import { TextAssetEditorModal, type TextAssetEditorModalOptions } from './TextAssetEditorModal.js';

export interface PromptFileSettingsOptions {
  readonly app: unknown;
  readonly document: Document;
  readonly wildcards: TextAssetSource;
  readonly autocompleteLists: TextAssetSource;
  readonly mountEditor: TextAssetEditorModalOptions['mountEditor'];
  openWildcardFolder(): Promise<void>;
  refreshWildcards(): Promise<void>;
}

/** Give both rows their buttons; the returned function withdraws them. */
export function registerPromptFileSettings(options: PromptFileSettingsOptions): () => void {
  const adapter = {
    getDocument: () => options.document,
    getWindow: () => options.document.defaultView,
  };
  // Each dialog is made once and reused, so repeated openings leave nothing behind in the page.
  const names = new InputModal({ adapter });
  const confirmation = new ConfirmDialog({ adapter });
  const editor = (source: TextAssetSource): TextAssetEditorModal =>
    new TextAssetEditorModal({
      document: options.document,
      source,
      mountEditor: options.mountEditor,
      prompt: (prompt) => names.open(prompt),
      confirm: (title, message, confirmLabel) =>
        confirmation.open({ title, message, confirmLabel }),
    });
  const editors = new Map([
    [options.wildcards, editor(options.wildcards)],
    [options.autocompleteLists, editor(options.autocompleteLists)],
  ]);
  const open = (source: TextAssetSource): Promise<void> =>
    editors.get(source)?.open() ?? Promise.resolve();
  const wildcards = registerWildcardSettings({
    app: options.app,
    manage: () => open(options.wildcards),
    openFolder: options.openWildcardFolder,
    refresh: options.refreshWildcards,
  });
  const lists = settingsCatalogFor(options.app).contribute(
    'prompt_editing.autocomplete.manage_lists',
    {
      controls: [
        {
          kind: 'button',
          button: { label: 'Manage', run: () => open(options.autocompleteLists) },
        },
      ],
    },
  );
  return () => {
    wildcards();
    lists();
  };
}

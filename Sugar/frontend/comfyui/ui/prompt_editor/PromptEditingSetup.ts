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
 * Set up prompt editing for the page: the editors' services (preferences, spellchecker,
 * autocomplete lists, Danbooru paste) and Prompt Editing's file rows, whose full-window editor
 * shows each file in the same prompt editor.
 */
import type {
  CubePromptEditorHost,
  PromptCanvasHost,
  PromptRegionLinks,
} from './CubePromptEditorHost.js';
import type { PromptEditorPreferences } from './PromptEditorPreferences.js';
import { startPageSpellcheck } from './spellcheck/PageSpellcheck.js';
import type { BooruPostLookup } from '../../../sugarprompt/services/src/booru/links.js';
import { registerPromptFileSettings } from '../text_assets/presentation/PromptFileSettings.js';
import {
  wildcardTextAssets,
  type WildcardFileActions,
} from '../wildcards/application/WildcardTextAssets.js';
import { autocompleteListTextAssets } from '../autocomplete_lists/application/AutocompleteListTextAssets.js';
import type { AutocompleteListLibrary } from '../autocomplete_lists/application/AutocompleteListLibrary.js';

export interface PromptEditingSetupOptions {
  readonly app: unknown;
  readonly document: Document;
  readonly logger: Pick<Console, 'error' | 'warn'>;
  readonly editors: CubePromptEditorHost;
  readonly regions: PromptRegionLinks | null;
  readonly prompts: NonNullable<PromptCanvasHost['prompts']> & {
    readonly preferences: PromptEditorPreferences;
    readonly tagLists: AutocompleteListLibrary;
    /** The wildcard folder's files, for the full-window editor and Open folder. */
    readonly wildcardFiles: WildcardFileActions & { openFolder(): Promise<void> };
    readonly booruPostLookup: BooruPostLookup;
  };
  readonly models: PromptCanvasHost['models'];
}

/** Start the editors' services, read the autocomplete lists, and offer the file rows. */
export function setupPromptEditing(options: PromptEditingSetupOptions): void {
  const { prompts } = options;
  options.editors.setup(options.regions, {
    app: options.app,
    logger: options.logger,
    prompts,
    preferences: prompts.preferences,
    tagLists: prompts.tagLists,
    booruPostLookup: prompts.booruPostLookup,
    spellcheck: startPageSpellcheck(options.app, (error) =>
      options.logger.warn('SugarCubes spellchecker failed', { error }),
    ),
    ...(options.models ? { models: options.models } : {}),
  });
  void prompts.tagLists.refresh();
  registerPromptFileSettings({
    app: options.app,
    document: options.document,
    wildcards: wildcardTextAssets(prompts.wildcardFiles, () => prompts.wildcards.refresh()),
    autocompleteLists: autocompleteListTextAssets(prompts.tagLists),
    mountEditor: (parent, file) => options.editors.mountDocument(parent, file),
    openWildcardFolder: () => prompts.wildcardFiles.openFolder(),
    refreshWildcards: () => prompts.wildcards.refresh(),
  });
}

import { startPageSpellcheck } from './spellcheck/PageSpellcheck.js';
import { registerPromptFileSettings } from '../text_assets/presentation/PromptFileSettings.js';
import { wildcardTextAssets, } from '../wildcards/application/WildcardTextAssets.js';
import { autocompleteListTextAssets } from '../autocomplete_lists/application/AutocompleteListTextAssets.js';
/** Start the editors' services, read the autocomplete lists, and offer the file rows. */
export function setupPromptEditing(options) {
    const { prompts } = options;
    options.editors.setup(options.regions, {
        app: options.app,
        logger: options.logger,
        prompts,
        preferences: prompts.preferences,
        tagLists: prompts.tagLists,
        booruPostLookup: prompts.booruPostLookup,
        spellcheck: startPageSpellcheck(options.app, (error) => options.logger.warn('SugarCubes spellchecker failed', { error })),
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

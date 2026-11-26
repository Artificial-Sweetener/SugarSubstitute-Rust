import { ComfySettingStore } from '../../settings/workspace/ComfySettingStore.js';
import { WorkerSpellcheckGateway } from './WorkerSpellcheckGateway.js';
/** The ComfyUI setting that keeps the words the person added to the dictionary. */
export const SPELLING_WORDS_SETTING = 'SugarCubes.PromptEditor.SpellingWords';
/** The added words, kept in ComfyUI's settings. */
class ComfySpellingWords {
    #store;
    constructor(store) {
        this.#store = store;
    }
    read() {
        const stored = this.#store.get(SPELLING_WORDS_SETTING);
        return Array.isArray(stored) ? stored.filter((word) => typeof word === 'string') : [];
    }
    async add(word) {
        const words = this.read();
        if (!words.includes(word))
            await this.#store.set(SPELLING_WORDS_SETTING, [...words, word]);
    }
}
/**
 * Start the spellchecker for this page, or null where no Web Worker can run. The worker and the
 * files it reads are deployed beside this module; the dictionary downloads on the first check.
 */
export function startPageSpellcheck(app, report) {
    if (typeof Worker === 'undefined')
        return null;
    const store = new ComfySettingStore(app);
    store.register({
        id: SPELLING_WORDS_SETTING,
        name: 'Prompt editor dictionary words',
        type: 'hidden',
        defaultValue: [],
    });
    const deployed = (path) => new URL(path, import.meta.url).href;
    const worker = new Worker(deployed('./SpellcheckWorker.js'), {
        type: 'module',
        name: 'SugarCubes spellchecker',
    });
    return new WorkerSpellcheckGateway(worker, {
        module: deployed('../../../../spellcheck/sugarcubes-spellcheck.wasm'),
        aff: deployed('../../../../spellcheck/en_US.aff'),
        dic: deployed('../../../../spellcheck/en_US.dic'),
    }, new ComfySpellingWords(store), report);
}

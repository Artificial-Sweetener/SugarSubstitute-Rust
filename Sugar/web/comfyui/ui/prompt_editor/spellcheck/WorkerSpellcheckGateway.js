import { spellcheckReply } from './SpellcheckProtocol.js';
/**
 * Ask the worker for checks and suggestions. The dictionary loads on the first check, so a page
 * whose prompts are never checked never downloads it. A worker that fails to load reports the
 * dictionary unavailable from then on, and the editor shows no spelling marks.
 */
export class WorkerSpellcheckGateway {
    identity = Symbol('SugarCubes spellchecker');
    languageTag = 'en-US';
    #port;
    #files;
    #words;
    #report;
    #pending = new Map();
    #next = 0;
    #loaded = null;
    #failed = false;
    constructor(port, files, words, report) {
        this.#port = port;
        this.#files = files;
        this.#words = words;
        this.#report = report;
        port.addEventListener('message', (event) => {
            const reply = spellcheckReply(event.data);
            const settle = reply ? this.#pending.get(reply.id) : undefined;
            if (!reply || !settle)
                return;
            this.#pending.delete(reply.id);
            if (reply.status === 'failed')
                this.#report(new Error(reply.message));
            settle({
                ok: reply.status === 'ready',
                value: reply.status === 'ready' ? reply.value : null,
            });
        });
        port.addEventListener('error', (event) => {
            this.#failed = true;
            this.#report(new Error(`The spellchecker stopped: ${event.type}`));
            for (const settle of this.#pending.values())
                settle({ ok: false, value: null });
            this.#pending.clear();
        });
    }
    availability() {
        return this.#failed
            ? { status: 'unavailable', code: 'dictionary-unavailable' }
            : { status: 'available', dictionaryAdd: true, sessionIgnore: false };
    }
    async checkWords(words, signal) {
        return this.#ask(signal, { kind: 'check', words: [...words] });
    }
    async suggest(word, limit, signal) {
        return this.#ask(signal, { kind: 'suggest', word, limit });
    }
    async addToDictionary(word, signal) {
        const outcome = await this.#ask(signal, { kind: 'add', word });
        if (outcome.status === 'ready' && outcome.value === true)
            await this.#words.add(word);
        return outcome;
    }
    /** Stop the worker; later questions are answered unavailable. */
    dispose() {
        this.#failed = true;
        this.#port.terminate();
        for (const settle of this.#pending.values())
            settle({ ok: false, value: null });
        this.#pending.clear();
    }
    async #ask(signal, request) {
        if (signal.aborted)
            return { status: 'cancelled' };
        if (!(await this.#load()) || this.#failed)
            return { status: 'unavailable', code: 'dictionary-unavailable' };
        if (signal.aborted)
            return { status: 'cancelled' };
        const reply = await this.#send(request, signal);
        if (reply === 'cancelled')
            return { status: 'cancelled' };
        return reply.ok
            ? { status: 'ready', value: reply.value }
            : { status: 'unavailable', code: 'backend-unavailable' };
    }
    /** Load the dictionary once; every question waits for it. */
    #load() {
        this.#loaded ??= this.#send({ kind: 'load', load: { ...this.#files, words: this.#words.read() } }, null).then((reply) => {
            const loaded = reply !== 'cancelled' && reply.ok;
            if (!loaded)
                this.#failed = true;
            return loaded;
        });
        return this.#loaded;
    }
    #send(request, signal) {
        const id = ++this.#next;
        return new Promise((resolve) => {
            const abort = () => {
                // The worker still answers; its reply is no longer wanted.
                this.#pending.delete(id);
                resolve('cancelled');
            };
            signal?.addEventListener('abort', abort, { once: true });
            this.#pending.set(id, (reply) => {
                signal?.removeEventListener('abort', abort);
                resolve(reply);
            });
            this.#port.postMessage({ id, ...request });
        });
    }
}

/** Retain only transient state; decoders and GPU resources belong to the visible attachment. */
export class BrowserCanvasMediaSessions {
    #videos = new Map();
    #models = new Map();
    /** Read an exact output revision's playback position and preferences. */
    video(key) {
        return this.#videos.get(key);
    }
    /** Remember bounded playback state without retaining the media element. */
    rememberVideo(key, state) {
        this.#set(this.#videos, key, state);
    }
    /** Read host camera state belonging to one output revision. */
    model(key) {
        return this.#models.get(key);
    }
    /** Remember the host's camera snapshot without retaining its renderer. */
    rememberModel(key, state) {
        this.#set(this.#models, key, state);
    }
    /** Release all session data at host teardown. */
    dispose() {
        this.#videos.clear();
        this.#models.clear();
    }
    /** Bound inspection history even when a workflow produces many output revisions. */
    #set(entries, key, value) {
        entries.delete(key);
        entries.set(key, value);
        while (entries.size > 64) {
            const oldest = entries.keys().next().value;
            if (oldest !== undefined)
                entries.delete(oldest);
        }
    }
}

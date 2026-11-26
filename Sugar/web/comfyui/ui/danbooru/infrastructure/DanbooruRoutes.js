/** Ask SugarCubes' Danbooru route; every request has a time limit and can be cancelled. */
export class DanbooruRoutes {
    fetchApi;
    constructor(fetchApi) {
        this.fetchApi = fetchApi;
    }
    /**
     * The post a pasted link names, in the shape the editor's paste import validates. A backend
     * that cannot be reached is Danbooru being unavailable, so the link is still pasted.
     */
    async pastedPost(key, signal) {
        const query = key.kind === 'post' ? { id: key.id } : { md5: key.md5 };
        const controller = new AbortController();
        const timeout = setTimeout(() => controller.abort(), 30_000);
        const abort = () => controller.abort();
        signal.addEventListener('abort', abort, { once: true });
        try {
            const response = await this.fetchApi(`/sugarcubes/v1/danbooru/pasted-post?${new URLSearchParams(query).toString()}`, { signal: controller.signal });
            return response.ok ? await response.json() : { status: 'unavailable' };
        }
        catch (error) {
            if (signal.aborted)
                throw error;
            return { status: 'unavailable' };
        }
        finally {
            clearTimeout(timeout);
            signal.removeEventListener('abort', abort);
        }
    }
}

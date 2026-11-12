/** Keep source discovery and history outside settings presentation. */
export class SourceSavingControls {
    configuration;
    dialog;
    #port = null;
    #pending = null;
    /** Bind committed settings and the shared source modal. */
    constructor(configuration, dialog) {
        this.configuration = configuration;
        this.dialog = dialog;
    }
    /** Lease the active source provider without allowing teardown to clear a replacement binding. */
    bind(port) {
        this.#port = port;
        return () => {
            if (this.#port === port)
                this.#port = null;
        };
    }
    /** Open one deliberate edit while revalidating shared settings before capture. */
    open() {
        if (this.#pending)
            return this.#pending;
        const port = this.#port;
        if (!port)
            return Promise.reject(new Error('The workflow output controls are unavailable.'));
        let session;
        try {
            session = port.begin();
        }
        catch (error) {
            return Promise.reject(error instanceof Error
                ? error
                : new Error('Output source settings could not be read.', { cause: error }));
        }
        this.#pending = this.configuration
            .load()
            .then((snapshot) => {
            if (this.#port !== port || !session.current())
                throw new Error('The workflow changed before output settings opened.');
            return this.dialog.open(session, snapshot.preferences);
        })
            .finally(() => {
            this.#pending = null;
        });
        return this.#pending;
    }
}

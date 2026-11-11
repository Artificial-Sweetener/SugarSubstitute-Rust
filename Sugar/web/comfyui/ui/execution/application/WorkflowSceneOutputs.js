/** Preserve accepted-scene handoff while a newer ordinary run still awaits its first final. */
export class WorkflowSceneOutputs {
    #scenes = null;
    /** Follow only a current accepted scene run; a pending ordinary run retains existing scene pixels. */
    follow(run, frontier) {
        const member = run.scene;
        if (!member || run.order < frontier)
            return;
        if (this.#scenes?.run.id !== member.run.id)
            this.#scenes = { run: member.run, outputs: new Map(), running: member.key, order: run.order };
        else {
            this.#scenes.running = member.key;
            this.#scenes.order = Math.max(this.#scenes.order, run.order);
        }
    }
    /** Retain every scene's latest source output, retiring its group only on an ordinary final handoff. */
    retain(run, output, frontier) {
        if (run?.scene) {
            this.follow(run, frontier);
            if (this.#scenes?.run.id === run.scene.run.id) {
                const outputs = this.#scenes.outputs.get(run.scene.key) ?? new Map();
                outputs.set(output.source.id, output);
                this.#scenes.outputs.set(run.scene.key, outputs);
            }
        }
        else if (run && this.#scenes && run.order > this.#scenes.order)
            this.#scenes = null;
    }
    /** Import already validated scene membership without marking any scene as executing or replaying a job. */
    restore(snapshot, frontier) {
        this.#scenes = snapshot
            ? {
                run: Object.freeze({
                    id: snapshot.id,
                    scenes: Object.freeze(snapshot.scenes.map(({ key, title, order }) => Object.freeze({ key, title, order }))),
                }),
                outputs: new Map(snapshot.scenes.map((scene) => [
                    scene.key,
                    new Map(scene.outputs.map((output) => [output.source.id, output])),
                ])),
                running: null,
                order: frontier,
            }
            : null;
    }
    /** Publish authored scene order while preserving reference identity for unchanged subscriptions. */
    snapshot(previous) {
        const scenes = this.#scenes;
        if (!scenes)
            return null;
        const next = scenes.run.scenes.map((scene) => Object.freeze({
            ...scene,
            outputs: Object.freeze([...(scenes.outputs.get(scene.key)?.values() ?? [])].sort((a, b) => a.source.order - b.source.order)),
        }));
        if (previous?.id === scenes.run.id &&
            previous.running === scenes.running &&
            previous.scenes.length === next.length &&
            previous.scenes.every((scene, index) => {
                const current = next[index];
                return (!!current &&
                    scene.key === current.key &&
                    scene.outputs.length === current.outputs.length &&
                    scene.outputs.every((output, at) => output === current.outputs[at]));
            }))
            return previous;
        return Object.freeze({
            id: scenes.run.id,
            running: scenes.running,
            scenes: Object.freeze(next),
        });
    }
}

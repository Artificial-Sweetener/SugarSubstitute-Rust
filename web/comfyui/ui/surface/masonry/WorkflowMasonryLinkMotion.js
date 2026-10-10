import { fluentMotion } from '../appearance/FluentThemeRuntime.js';
/** Own temporary strokes while keeping reroute segments continuous along each complete noodle. */
export class WorkflowMasonryLinkMotion {
    #snapshot;
    #element;
    #routes = [];
    #effects = new Map();
    #generation = 0;
    /** Measure each exact vector once; retain no screenshots, node clones, or per-frame route work. */
    constructor(container, snapshot) {
        this.#snapshot = snapshot;
        const document = container.ownerDocument;
        this.#element = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
        this.#element.setAttribute('aria-hidden', 'true');
        this.#element.classList.add('sugarcubes-masonry-link-motion');
        Object.assign(this.#element.style, {
            position: 'absolute',
            inset: '0',
            width: '100%',
            height: '100%',
            pointerEvents: 'none',
            zIndex: '0',
            overflow: 'hidden',
        });
        const routes = new Map();
        for (const stroke of snapshot.strokes) {
            const element = document.createElementNS('http://www.w3.org/2000/svg', 'path');
            element.setAttribute('d', stroke.path);
            element.setAttribute('transform', stroke.transform);
            element.setAttribute('stroke', stroke.color);
            element.setAttribute('stroke-width', String(stroke.width));
            element.setAttribute('stroke-linecap', stroke.lineCap);
            element.setAttribute('stroke-linejoin', 'round');
            element.setAttribute('fill', 'none');
            element.setAttribute('opacity', String(stroke.opacity));
            element.setAttribute('pathLength', '1');
            element.style.strokeDasharray = '1 1';
            this.#element.append(element);
            let route = routes.get(stroke.link);
            if (!route) {
                route = new Map();
                routes.set(stroke.link, route);
            }
            const key = `${stroke.transform}:${stroke.path}`;
            const segment = route.get(key) ?? { length: 0, elements: [] };
            segment.elements.push(element);
            route.set(key, segment);
        }
        const nodeLayer = [...container.children].find((element) => element.matches('.ph-no-capture, .isolate'));
        container.insertBefore(this.#element, nodeLayer ?? null);
        try {
            for (const route of routes.values()) {
                for (const segment of route.values())
                    segment.length = segment.elements[0]?.getTotalLength() ?? 0;
                const total = [...route.values()].reduce((sum, segment) => sum + segment.length, 0);
                let offset = 0;
                const segments = [];
                for (const segment of route.values()) {
                    const start = total > 0 ? offset / total : 0;
                    offset += segment.length;
                    const end = total > 0 ? offset / total : 1;
                    segments.push({ elements: segment.elements, start, end });
                }
                this.#routes.push(segments);
            }
        }
        catch (error) {
            this.#element.remove();
            throw error;
        }
    }
    /** Retract from the input toward the output and replug in the output-to-input graph direction. */
    frames(entering, interrupted) {
        const effects = this.#routes.flatMap((route) => {
            const offsets = route.map(({ elements }) => {
                const first = elements[0];
                return interrupted && first
                    ? Math.max(0, Math.min(1, Number.parseFloat(getComputedStyle(first).strokeDashoffset) || 0))
                    : entering
                        ? 0
                        : 1;
            });
            const visible = route.reduce((sum, segment, index) => sum + (segment.end - segment.start) * (1 - (offsets[index] ?? 0)), 0);
            const remaining = entering ? visible : 1 - visible;
            const time = (point) => remaining > 0
                ? Math.max(0, Math.min(1, (entering ? visible - point : point - visible) / remaining))
                : 0;
            return route.flatMap(({ elements, start, end }, index) => elements.map((element) => {
                const from = entering ? 0 : 1, to = entering ? 1 : 0;
                const current = interrupted ? (offsets[index] ?? from) : from;
                const frames = [
                    { strokeDashoffset: current, offset: 0 },
                    { strokeDashoffset: current, offset: time(entering ? end : start) },
                    { strokeDashoffset: to, offset: time(entering ? start : end) },
                    { strokeDashoffset: to, offset: 1 },
                ];
                return { element, frames };
            }));
        });
        for (const { element } of effects)
            element.style.strokeDashoffset = entering ? '1' : '0';
        return effects;
    }
    /** Freeze a reversal at its current tip, or keep every noodle hidden during node travel. */
    hold(hidden = false) {
        this.#generation += 1;
        if (hidden)
            this.#element.style.visibility = 'hidden';
        for (const route of this.#routes)
            for (const segment of route)
                for (const element of segment.elements)
                    element.style.strokeDashoffset = hidden
                        ? '1'
                        : getComputedStyle(element).strokeDashoffset;
        for (const effect of this.#effects.values())
            effect.cancel();
        this.#effects.clear();
    }
    /** Animate only paint along settled native routes, independently of card layout effects. */
    async run(entering, reducedMotion) {
        this.hold();
        const generation = this.#generation;
        const requests = this.frames(entering, true);
        this.#element.style.visibility = 'visible';
        try {
            if (!reducedMotion)
                for (const request of requests) {
                    if (typeof request.element.animate !== 'function')
                        continue;
                    const effect = request.element.animate(request.frames, {
                        duration: fluentMotion.fasterDuration,
                        easing: fluentMotion.easing,
                        fill: 'both',
                    });
                    this.#effects.set(request.element, effect);
                }
            await Promise.allSettled([...this.#effects.values()].map((effect) => effect.finished));
        }
        finally {
            if (generation === this.#generation) {
                for (const effect of this.#effects.values())
                    effect.cancel();
                this.#effects.clear();
                if (entering)
                    this.#element.style.visibility = 'hidden';
            }
        }
    }
    /** Restore native paint before removing the temporary trajectories. */
    dispose() {
        this.hold();
        try {
            this.#snapshot.restore();
        }
        finally {
            this.#element.remove();
            this.#routes.length = 0;
        }
    }
}

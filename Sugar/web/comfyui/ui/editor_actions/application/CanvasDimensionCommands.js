import { canvasSizingProposal } from '../../core/embedded/CanvasRuntime.js';
/** Keep arithmetic, captured context and discovery sequencing independent of editor-specific UI. */
export class CanvasDimensionCommands {
    host;
    discovery;
    /** Borrow live capability discovery while keeping probes entirely opt-in. */
    constructor(host, discovery) {
        this.host = host;
        this.discovery = discovery;
    }
    /** Advertise only an actual connected geometry capability. */
    get available() {
        return this.host() !== null;
    }
    /** Freeze intent at command entry and refuse stale documents or replaced fields before applying. */
    async execute(action, binding, cancellation, configure) {
        const host = this.host();
        if (!host)
            throw new Error('Connect a canvas that supports dimensions.');
        if (cancellation?.cancelled())
            return;
        if (binding.current?.() === false)
            throw new Error('The workflow changed while configuring dimensions.');
        const toCanvas = action.startsWith('canvas-');
        const width = binding.width.read(), height = binding.height.read();
        if (binding.width.disabled() ||
            binding.height.disabled() ||
            typeof width !== 'number' ||
            typeof height !== 'number' ||
            ![width, height].every((value) => Number.isSafeInteger(value) && value >= 0))
            throw new Error('The dimension fields are unavailable.');
        if (toCanvas && (width === 0 || height === 0))
            throw new Error('Set a positive size before resizing the canvas.');
        const context = await host.execute({ type: 'context' }, cancellation);
        if (context.kind === 'failure' && context.status === 'cancelled')
            return;
        if (context.kind !== 'context' || !context.supported || !context.document)
            throw new Error(context.kind === 'failure' ? context.message : 'Open a document in the connected canvas.');
        const selection = action.includes('-selection');
        const scaled = action.endsWith('-scaled');
        if (toCanvas && !context.resize)
            throw new Error('This document does not support canvas resizing.');
        if (selection && (!context.selectionSupported || !context.selection))
            throw new Error('Select an area in the connected canvas.');
        const reference = selection && context.selection ? context.selection : context.document;
        const pixels = { minimum: 1, maximum: 300000, multiple: 1 };
        let request = {
            document: context.document,
            direction: toCanvas ? 'to-canvas' : 'from-target',
            reference: { width: reference.width, height: reference.height },
            generation: { width, height },
            widthLimits: toCanvas ? pixels : (binding.width.limits?.() ?? pixels),
            heightLimits: toCanvas ? pixels : (binding.height.limits?.() ?? pixels),
            stage: 'factor',
            scale: '1',
            outputs: [],
            output: null,
        };
        let probe = null;
        try {
            if (scaled) {
                if (!configure)
                    throw new Error('Scaling requires an active dimension menu.');
                for (;;) {
                    if (cancellation?.cancelled())
                        return;
                    const choice = await configure(request, cancellation);
                    if (choice.choice === 'cancel')
                        return;
                    request = { ...request, scale: choice.scale, output: choice.output };
                    if (choice.choice === 'apply')
                        break;
                    if (!probe) {
                        probe = await this.discovery.prepare(binding, cancellation);
                        if (!probe.outputs.length)
                            throw new Error('No measurable output is connected to these dimension fields.');
                        request = { ...request, outputs: [...probe.outputs] };
                    }
                    const output = choice.output ?? (probe.outputs.length === 1 ? probe.outputs[0]?.id : null);
                    if (!output)
                        continue;
                    if (!probe.outputs.some((candidate) => candidate.id === output))
                        throw new Error('The selected output path is unavailable.');
                    const factor = await probe.discover(output, cancellation);
                    if (!Number.isFinite(factor) || factor <= 0 || factor > 1000)
                        throw new Error('The workflow did not yield a supported scale factor.');
                    request = {
                        ...request,
                        scale: factor.toFixed(8).replace(/0+$/, '').replace(/\.$/, ''),
                        output,
                        stage: 'confirm',
                    };
                }
            }
            if (cancellation?.cancelled())
                return;
            if (binding.current?.() === false)
                throw new Error('The workflow changed while configuring dimensions.');
            const current = await host.execute({ type: 'context' }, cancellation);
            if (current.kind === 'failure' && current.status === 'cancelled')
                return;
            if (current.kind !== 'context' ||
                current.document?.id !== context.document.id ||
                current.document.geometryRevision !== context.document.geometryRevision)
                throw new Error('The canvas document changed while configuring dimensions.');
            if (selection && JSON.stringify(current.selection) !== JSON.stringify(context.selection))
                throw new Error('The canvas selection changed while configuring dimensions.');
            if (binding.width.disabled() ||
                binding.height.disabled() ||
                binding.width.read() !== width ||
                binding.height.read() !== height)
                throw new Error('The dimension fields changed while configuring dimensions.');
            const size = canvasSizingProposal(request);
            if (toCanvas) {
                const result = await host.execute({ type: 'resize', document: context.document, size }, cancellation);
                if (result.kind === 'failure' && result.status === 'cancelled')
                    return;
                if (result.kind !== 'resized')
                    throw new Error(result.kind === 'failure'
                        ? result.message
                        : 'The canvas did not confirm its dimensions.');
            }
            else {
                if (binding.width.write(size.width) === false ||
                    binding.height.write(size.height) === false) {
                    binding.width.write(width);
                    binding.height.write(height);
                    throw new Error('The native dimension fields refused this change.');
                }
            }
        }
        catch (error) {
            if (!cancellation?.cancelled())
                throw error;
        }
        finally {
            probe?.dispose();
        }
    }
}

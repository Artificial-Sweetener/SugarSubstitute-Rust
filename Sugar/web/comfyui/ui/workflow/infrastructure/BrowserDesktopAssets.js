import { DesktopSessionRefusal } from '../domain/DesktopSession.js';
/** Keep retained bytes and digest verification with the native owner; expose only current-page presentation. */
export class BrowserDesktopAssets {
    transport;
    request;
    clock;
    /** Inject current-origin HTTP and page timers without creating another asset store or resolver. */
    constructor(transport, request, clock) {
        this.transport = transport;
        this.request = request;
        this.clock = clock;
    }
    /** Probe verified current asset facts; WebP animation is a byte-derived fact, never a filename heuristic. */
    async presentation(output, binding, cancellation) {
        if (output.assetDigest === null ||
            output.assetDigest !== output.digest ||
            binding.entryId !== output.entryId)
            throw new TypeError('Retained output does not belong to this admitted desktop occurrence.');
        const url = this.transport.assetUrl?.(output.id, binding);
        if (!url)
            throw new DesktopSessionRefusal('unavailable', 'Retained output presentation capability is unavailable.');
        const controller = new AbortController();
        const stop = cancellation.subscribe(() => controller.abort());
        const timer = this.clock.setTimeout(() => controller.abort(), 15000);
        try {
            const response = await this.request(url, { method: 'HEAD', signal: controller.signal });
            if (cancellation.cancelled())
                throw new DesktopSessionRefusal('cancelled', 'Retained output metadata request was cancelled.');
            if (!response.ok)
                throw new DesktopSessionRefusal('unavailable', `Retained output metadata is unavailable (${response.status}).`);
            const bytes = response.headers.get('content-length');
            const mime = response.headers.get('content-type')?.split(';')[0]?.trim().toLowerCase();
            const animated = response.headers.get('x-sugarsubstitute-animated');
            if (bytes === null ||
                !/^[0-9]+$/.test(bytes) ||
                Number(bytes) !== output.bytes ||
                mime !== output.mime ||
                (animated !== '0' && animated !== '1'))
                throw new DesktopSessionRefusal('identity-mismatch', 'Retained output metadata does not match the exact recorded artifact.');
            return Object.freeze({ url, animated: animated === '1' });
        }
        finally {
            this.clock.clearTimeout(timer);
            stop();
        }
    }
}

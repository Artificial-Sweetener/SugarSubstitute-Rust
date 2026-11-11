import { readDocumentVisualStyle, subscribeVisualStyle } from './VisualStyleBoundary.js';
/** Keep owned editors themed when the native renderer suspends its card styling. */
export class OwnedSurfaceAppearance {
    document;
    native;
    /** Borrow the existing style and palette owners without retaining a second preference. */
    constructor(document, native) {
        this.document = document;
        this.native = native;
    }
    /** Read selected appearance independently of native-card renderer support. */
    get appearance() {
        return { style: readDocumentVisualStyle(this.document), dark: this.native.appearance.dark };
    }
    /** Release both authoritative appearance subscriptions with the owning surface. */
    subscribeAppearance(listener) {
        const releaseStyle = subscribeVisualStyle(this.document, listener);
        const releasePalette = this.native.subscribeAppearance(listener);
        return () => {
            releaseStyle();
            releasePalette();
        };
    }
}

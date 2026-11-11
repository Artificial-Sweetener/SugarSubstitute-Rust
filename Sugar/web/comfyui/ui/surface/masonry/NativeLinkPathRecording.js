/** Accept the same drawing commands as native paths and retain only their vector representation. */
export class NativeLinkPathRecording {
    #commands = [];
    /** Begin a native segment at its output socket. */
    moveTo(x, y) {
        this.#record('M', [x, y]);
    }
    /** Preserve native straight or orthogonal segments. */
    lineTo(x, y) {
        this.#record('L', [x, y]);
    }
    /** Preserve both native spline controls and its input endpoint. */
    bezierCurveTo(a, b, c, d, x, y) {
        this.#record('C', [a, b, c, d, x, y]);
    }
    /** Preserve native single-control curves. */
    quadraticCurveTo(a, b, x, y) {
        this.#record('Q', [a, b, x, y]);
    }
    /** Expose validated vector commands after the native builder finishes. */
    get path() {
        return this.#commands.join(' ');
    }
    /** Reject invalid external coordinates before they reach the SVG presentation. */
    #record(command, coordinates) {
        if (!coordinates.every(Number.isFinite))
            throw new Error('Native link path contains invalid coordinates.');
        this.#commands.push(`${command}${coordinates.join(' ')}`);
    }
}

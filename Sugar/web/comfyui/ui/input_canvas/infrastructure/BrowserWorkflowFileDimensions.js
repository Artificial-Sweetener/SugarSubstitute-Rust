/** Load each file through Comfy's `/view` route on a detached image and keep only its size. */
export class BrowserWorkflowFileDimensions {
    document;
    /** Create detached images in the page that owns the Comfy origin. */
    constructor(document) {
        this.document = document;
    }
    /** Resolve the decoded size, or null when the file cannot be decoded. */
    async read(file) {
        const image = this.document.createElement('img');
        image.decoding = 'async';
        image.src =
            '/view?' +
                new URLSearchParams({
                    filename: file.filename,
                    subfolder: file.subfolder,
                    type: file.type,
                }).toString();
        const decoded = await image.decode().then(() => true, 
        // A missing or undecodable file has no verifiable size; callers report null as unverified.
        () => false);
        const size = decoded && image.naturalWidth > 0 && image.naturalHeight > 0
            ? { width: image.naturalWidth, height: image.naturalHeight }
            : null;
        image.removeAttribute('src');
        return size;
    }
}

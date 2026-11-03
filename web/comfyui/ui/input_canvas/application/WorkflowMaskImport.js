/**
 * Convert every picture to the loader's surface size before anything is added, so a failed
 * picture adds no region. Each staged file becomes a new region's initial mask; the surface
 * provisioning then creates and binds its layer.
 */
export class WorkflowMaskImport {
    surfaces;
    rasters;
    assets;
    /** Share the staging owner used for queue-time captures. */
    constructor(surfaces, rasters, assets) {
        this.surfaces = surfaces;
        this.rasters = rasters;
        this.assets = assets;
    }
    /** Whether the loader's masks live on a surface that can receive imports. */
    available(endpoint) {
        return this.#surface(endpoint) !== null;
    }
    /** Stage each picture and hand the staged files to the loader in order. */
    async import(endpoint, files, append) {
        const surface = this.#surface(endpoint);
        if (!surface)
            throw new Error('The mask has no input canvas.');
        const staged = [];
        for (const file of files)
            staged.push(await this.assets.stage(await this.rasters.raster(file, surface.document.width, surface.document.height)));
        if (staged.length)
            append(staged);
    }
    /** Find the surface holding the loader's masks. */
    #surface(endpoint) {
        return (this.surfaces.state?.surfaces.find((surface) => surface.masks.some((mask) => mask.endpoint === endpoint)) ?? null);
    }
}

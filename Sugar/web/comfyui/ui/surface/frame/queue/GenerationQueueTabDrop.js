/** ComfyUI keeps a job's workflow only once the job has left the queue. */
export function jobOpensAsTab(job) {
    return job !== null && job.status !== 'pending' && job.status !== 'running';
}
/** The title the job's workflow tab will have. */
export function jobTabTitle(job) {
    return job.workflowName ?? `Job ${job.id.slice(0, 8)}`;
}
/**
 * The tab bar as a place to drop a finished job. While the job hangs over the bar, a dashed tab
 * with the title it would have shows where it would open; dropping it opens the workflow there.
 */
export function queueTabDropTarget(host) {
    return {
        hit: (x, y, jobId) => {
            if (!jobOpensAsTab(host.job(jobId)))
                return null;
            const index = host.tabBar.hit(x, y);
            return index === null ? null : { index };
        },
        show: (jobId, hit) => {
            const job = host.job(jobId);
            const index = readIndex(hit);
            host.tabBar.preview(index !== null && jobOpensAsTab(job) ? jobTabTitle(job) : null, index ?? 0);
        },
        accept: (jobId, hit) => {
            const job = host.job(jobId);
            const index = readIndex(hit);
            if (index === null || !jobOpensAsTab(job))
                return;
            host.run('Workflow not opened', async () => {
                await host.open(job);
                host.tabBar.place(index);
            });
        },
    };
}
function readIndex(hit) {
    return typeof hit === 'object' && hit !== null && 'index' in hit && typeof hit.index === 'number'
        ? hit.index
        : null;
}

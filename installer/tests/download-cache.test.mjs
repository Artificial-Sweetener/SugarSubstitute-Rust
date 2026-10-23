// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
import {test} from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import {build} from 'esbuild'
import {fileURLToPath} from 'node:url'
const artifact=process.env.SUBSTITUTE_INSTALLER_BUILD
if (!artifact || !path.isAbsolute(artifact)) throw new Error('Explicit test artifact directory required')
const review=fileURLToPath(new URL('..',import.meta.url))
const receipt=JSON.parse(fs.readFileSync(path.join(artifact,'worker/build-receipt.json'),'utf8'))
const entry=receipt.inputs.find(file=>file.endsWith('/src/main/lib/download.ts'))
async function downloader(queued) {
 const directory=fs.mkdtempSync(path.join(artifact,'download-policy-'))
 const downloads=path.join(directory,'downloads'), metadata=path.join(directory,'metadata')
 fs.mkdirSync(downloads);fs.mkdirSync(metadata)
 const globals={queued, writes:0, request:undefined, response:undefined, chunk:Buffer.alloc(16), headers:16}
 const binding='__cache_download_fixture_'+Math.random().toString(16).slice(2)
 globalThis[binding]=globals
 const {outputFiles}=await build({entryPoints:[entry],bundle:true,write:false,platform:'node',format:'esm',target:'node22',plugins:[{
  name:'external-boundaries',setup(builder){
   builder.onResolve({filter:/^electron$/},()=>({path:'electron',namespace:'fixture'}))
   builder.onLoad({filter:/.*/,namespace:'fixture'},()=>({contents:`
    import {EventEmitter} from 'node:events'
    const fixture=globalThis[${JSON.stringify(binding)}]
    export const net={request(){
      const request=new EventEmitter();fixture.request=request
      request.setHeader=()=>{};request.abort=()=>{}
      request.end=()=>queueMicrotask(()=>{
        const response=new EventEmitter();fixture.response=response
        response.statusCode=200;response.headers={'content-length':String(fixture.headers)}
        request.emit('response',response)
        response.emit('data',fixture.chunk);response.emit('end')
      });return request
    }}
   `,loader:'js'}))
   builder.onResolve({filter:/^fs$/},()=>({path:'writer',namespace:'fixture-fs'}))
   builder.onLoad({filter:/.*/,namespace:'fixture-fs'},()=>({contents:`
    import fs from 'node:fs'
    import {EventEmitter} from 'node:events'
    const fixture=globalThis[${JSON.stringify(binding)}]
    export default {...fs,createWriteStream(file,options){
      const stream=new EventEmitter();stream.fd=options.fd;stream.writableLength=fixture.queued
      let closed=false
      const close=()=>{if(!closed){closed=true;fs.closeSync(stream.fd);stream.emit('close')}}
      stream.write=chunk=>{fixture.writes++;fs.writeSync(stream.fd,chunk);return true}
      stream.end=()=>queueMicrotask(close)
      stream.close=callback=>queueMicrotask(()=>{close();callback?.()})
      stream.destroy=()=>queueMicrotask(close)
      return stream
    }}
   `,loader:'js'}))
   builder.onResolve({filter:/settings$/},()=>({path:path.join(review,'worker/settings.ts')}))
   builder.onResolve({filter:/cache-policy$/},()=>({path:path.join(review,'worker/cache-policy.ts')}))
   builder.onResolve({filter:/^\.\/arguments$/},()=>({path:'arguments',namespace:'fixture-args'}))
   builder.onLoad({filter:/.*/,namespace:'fixture-args'},()=>({contents:`
    const args=${JSON.stringify({'cache-downloads':downloads,'cache-download-bytes':'4096','cache-metadata':metadata,'cache-metadata-bytes':'4096'})}
    export const argument=name=>args[name]
   `,loader:'js'}))
  }
 }]})
 const module=await import('data:text/javascript;base64,'+Buffer.from(outputFiles[0].text).toString('base64'))
 return {module,globals,directory,downloads,dispose:()=>delete globalThis[binding]}
}
test('patched downloader preserves resumable partials when its bounded write queue is saturated',async()=>{
 const fixture=await downloader(1024*1024)
 try {
  const file=path.join(fixture.downloads,'archive')
  await assert.rejects(fixture.module.download('https://example.test/archive',file,null),/write queue capacity/)
  await new Promise(resolve=>setImmediate(resolve))
  assert.equal(fixture.globals.writes,0)
  assert.equal(fs.statSync(file).size,0)
  assert.equal(JSON.parse(fs.readFileSync(file+'.dl-meta','utf8')).expectedSize,16)
 } finally {fixture.dispose()}
})
test('patched downloader commits a bounded exact body without requiring unsupported response methods',async()=>{
 const fixture=await downloader(0)
 try {
  const file=path.join(fixture.downloads,'archive')
  assert.equal(await fixture.module.download('https://example.test/archive',file,null),file)
  assert.deepEqual(fs.readFileSync(file),fixture.globals.chunk)
  assert.equal(fs.existsSync(file+'.dl-meta'),false)
  assert.equal(fixture.globals.response.pause,undefined)
 } finally {fixture.dispose()}
})

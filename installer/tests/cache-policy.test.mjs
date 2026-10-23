// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
import {test} from 'node:test'
import assert from 'node:assert/strict'
import fs from 'node:fs'
import path from 'node:path'
import {build} from 'esbuild'
import {fileURLToPath} from 'node:url'
const source=fileURLToPath(new URL('../worker/cache-policy.ts',import.meta.url))
const {outputFiles}=await build({entryPoints:[source],bundle:true,write:false,platform:'node',format:'esm'})
const {CacheGrant}=await import('data:text/javascript;base64,'+Buffer.from(outputFiles[0].text).toString('base64'))
const artifact=process.env.SUBSTITUTE_INSTALLER_BUILD
if (!artifact || !path.isAbsolute(artifact)) throw new Error('Explicit test artifact directory required')
const root=()=>fs.mkdtempSync(path.join(artifact,'cache-policy-'))
test('metadata replacement admits the old file and scratch before mutation',()=>{
 const directory=root(), grant=new CacheGrant(directory,64), record=path.join(directory,'record')
 grant.write(record,'x'.repeat(40))
 assert.throws(()=>grant.write(record,'y'.repeat(40)),/exhausted/)
 assert.equal(fs.readFileSync(record,'utf8'),'x'.repeat(40))
 grant.write(record,'z'.repeat(8))
 assert.equal(fs.readFileSync(record,'utf8'),'z'.repeat(8))
})
test('queued bytes prevent concurrent overcommit and settled removal releases capacity',()=>{
 const directory=root(), grant=new CacheGrant(directory,64), file=path.join(directory,'first')
 const stream=grant.download(file,false)
 stream.reserve(48)
 assert.throws(()=>grant.download(path.join(directory,'second'),false,32),/exhausted/)
 assert.equal(fs.existsSync(path.join(directory,'second')),false)
 fs.writeSync(stream.fd,Buffer.alloc(48));fs.closeSync(stream.fd);stream.settled()
 fs.unlinkSync(file);grant.validate()
 const next=grant.download(path.join(directory,'second'),false,32)
 fs.closeSync(next.fd);next.settled()
})
test('hard links cannot truncate authoritative bytes before or after grant creation',()=>{
 const parent=root(), directory=path.join(parent,'cache'), authored=path.join(parent,'authored')
 fs.mkdirSync(directory);fs.writeFileSync(authored,'preserve')
 const grant=new CacheGrant(directory,64), link=path.join(directory,'link')
 fs.linkSync(authored,link)
 assert.throws(()=>grant.download(link,false),/directory link/)
 assert.throws(()=>new CacheGrant(directory,64),/directory link/)
 assert.equal(fs.readFileSync(authored,'utf8'),'preserve')
})
test('junctions and unsafe paths cannot escape granted storage',()=>{
 const parent=root(), directory=path.join(parent,'cache'), external=path.join(parent,'external')
 fs.mkdirSync(directory);fs.mkdirSync(external)
 const grant=new CacheGrant(directory,64)
 fs.symlinkSync(external,path.join(directory,'link'),'junction')
 assert.throws(()=>grant.write(path.join(directory,'link','file'),'bad'),/links/)
 assert.equal(fs.existsSync(path.join(external,'file')),false)
 for(const name of ['../escape','NUL','trailing.']) assert.throws(()=>grant.write(path.join(directory,name),'bad'))
})

test('pressure is reported once before rejected writes and preserves existing bytes',()=>{
 const directory=root(), events=[], grant=new CacheGrant(directory,8,()=>events.push('pressure'))
 grant.write(path.join(directory,'first'),'12345678')
 for(let i=0;i<2;i++) assert.throws(()=>grant.write(path.join(directory,'other'),'1'),cause=>cause.statusCode==='cacheFull' && /exhausted/.test(cause.message))
 assert.deepEqual(events,['pressure'])
 assert.equal(fs.readFileSync(path.join(directory,'first'),'utf8'),'12345678')
 assert.equal(fs.existsSync(path.join(directory,'other')),false)
})

test('native file identity comparison preserves all 64 bits before truncation',async()=>{
 const directory=root(), file=path.join(directory,'archive')
 fs.writeFileSync(file,'preserve')
 const {outputFiles}=await build({entryPoints:[source],bundle:true,write:false,platform:'node',format:'esm',plugins:[{
  name:'native-identity-boundary',setup(builder){
   builder.onResolve({filter:/^node:fs$/},args=>args.namespace==='identity-fixture'
    ? {path:'node:fs',external:true} : {path:'fs',namespace:'identity-fixture'})
   builder.onLoad({filter:/.*/,namespace:'identity-fixture'},()=>({contents:`
    import fs from 'node:fs'
    const target=${JSON.stringify(file)}
    export default {...fs,
      lstatSync(file,options){
        const actual=fs.lstatSync(file,options)
        if(file===target){actual.dev=options?.bigint?1n:1;actual.ino=options?.bigint?9007199254740993n:Number(9007199254740993n)}
        return actual
      },
      fstatSync(fd,options){
        const actual=fs.fstatSync(fd,options)
        actual.dev=options?.bigint?1n:1;actual.ino=options?.bigint?9007199254740992n:Number(9007199254740992n)
        return actual
      }
    }
   `,loader:'js'}))
  }
 }]})
 const {CacheGrant}=await import('data:text/javascript;base64,'+Buffer.from(outputFiles[0].text).toString('base64'))
 const grant=new CacheGrant(directory,64)
 assert.throws(()=>grant.download(file,false),/identity changed/)
 assert.equal(fs.readFileSync(file,'utf8'),'preserve')
})

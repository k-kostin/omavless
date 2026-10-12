// SPDX-License-Identifier: MIT
// Developer-only, credential-free screenshot transport. No VPN/store/service API.
import net from 'node:net';
import fs from 'node:fs';
import path from 'node:path';
import {pathToFileURL} from 'node:url';
import {response, revision} from './backend.mjs';

export function reply(request) {
  if (!request || Array.isArray(request) || request.api !== 'omavless.control'
      || request.version !== 1 || typeof request.id !== 'string'
      || !/^[\x21-\x7e]{1,64}$/.test(request.id)
      || !request.params || Array.isArray(request.params)
      || typeof request.params !== 'object'
      || Object.keys(request).some(k => !['api','version','id','method','params'].includes(k)))
    return null;
  const envelope = result => ({api:request.api, version:1, id:request.id, ok:true, revision, result});
  if (request.method === 'system.hello' && JSON.stringify(request.params) === '{"versions":[1]}')
    return envelope({version:1, instanceId:'marketing-demo', runtimeOwnership:true});
  if (Object.keys(request.params).length === 0) {
    if (request.method === 'capabilities.get')
      return envelope({runtimeOwnership:true, mutations:true, methods:[
        'ui.snapshot','runtime.observation','connection.connect','connection.disconnect',
        'routing.set_mode','backup.create']});
    if (request.method === 'ui.snapshot') return {...response('status'), id:request.id};
    if (request.method === 'runtime.observation') return {...response('native-observation'), id:request.id};
  }
  // Advertised controls render normally; EVERY effect-bearing request refuses.
  // Public fixed fallback only: never reflect method, params or error fragments.
  return {api:request.api, version:1, id:request.id, ok:false, revision,
    error:{code:'capability_unavailable', message:'Unavailable in screenshot fixture.', retryable:false}};
}

export function serve(capture) {
  if (!/^\/tmp\/omavless-marketplace\.[A-Za-z0-9]+$/.test(capture)) throw new Error('fixture path refused');
  for (const dir of [capture, path.join(capture,'runtime')]) {
    const s=fs.lstatSync(dir);
    if (!s.isDirectory() || s.isSymbolicLink() || s.uid !== process.getuid()
        || (s.mode & 0o7777) !== 0o700) throw new Error('fixture parent refused');
  }
  const dir=path.join(capture,'runtime','omavless');
  fs.mkdirSync(dir,{mode:0o700});
  const socket=path.join(dir,'control.sock');
  const clients=new Set();
  const server=net.createServer({allowHalfOpen:true}, client => {
    if (clients.size>=16) return client.destroy();
    clients.add(client);
    let chunks=[], bytes=0;
    client.setTimeout(3000,()=>client.destroy());
    client.on('error',()=>{});
    client.on('close',()=>clients.delete(client));
    client.on('data',chunk=>{
      bytes+=chunk.length;
      if (bytes>65536) client.destroy(); else chunks.push(chunk);
    });
    client.on('end',()=>{
      const frame=Buffer.concat(chunks);
      if (!frame.length || frame.at(-1)!==10 || frame.subarray(0,-1).includes(10)) return client.destroy();
      try {
        const decoded=new TextDecoder('utf-8',{fatal:true}).decode(frame);
        const value=reply(JSON.parse(decoded));
        if (!value) return client.destroy();
        client.end(JSON.stringify(value)+'\n');
      } catch { client.destroy(); }
    });
  });
  server.on('error',()=>{console.error('Screenshot fixture unavailable.');process.exitCode=1;});
  server.listen(socket,()=>{fs.chmodSync(socket,0o600);console.log('Read-only screenshot fixture ready.');});
  const finish=()=>{for (const c of clients) c.destroy();server.close();};
  process.once('SIGTERM',finish);
  process.once('SIGINT',finish);
  const lifetime=setTimeout(finish,1800000);lifetime.unref();
  return server;
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  if (process.argv.length!==3) process.exitCode=64;
  else try { serve(process.argv[2]); } catch { console.error('Screenshot fixture refused.');process.exitCode=64; }
}

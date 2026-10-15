#!/usr/bin/env node
// Production successor functions; inert filesystem/processes only. No real sudo,
// package manager, service manager, resolver, network or desktop operations.
const fs=require('node:fs'), os=require('node:os'), path=require('node:path');
const assert=require('node:assert/strict');
const {spawnSync,spawn}=require('node:child_process');
const source=fs.readFileSync(path.join(__dirname,'../tools/uninstall.sh'),'utf8');
assert.equal(spawnSync('bash',['-n'],{input:source}).status,0);
const functions=source.slice(source.indexOf('fail()'),source.indexOf('\n[[ $# == 0 ]]'));
assert(functions.includes('take_setup_lock()')&&functions.includes('check_supported_installation()'));
const section=name=>{const m=source.match(new RegExp("<<'"+name+"' \\|\\| true\\n([\\s\\S]*?)\\n"+name));assert(m,name);return m[1]};
const uid=process.getuid(), boot='11111111-1111-1111-1111-111111111111', newer='22222222-2222-2222-2222-222222222222';
const token='a'.repeat(32), nextToken='b'.repeat(32);
const parent=path.join(os.homedir(),'.cache/ovt');
fs.mkdirSync(parent,{recursive:true,mode:0o700});
assert(!fs.lstatSync(parent).isSymbolicLink());
let count=0;
function fixture(){
 const dir=fs.mkdtempSync(path.join(parent,'uninstaller.'));
 for(const p of ['home/user/.cache/omavless-maintenance/uninstall','home/user/.config/systemd/user','run/user/'+uid,'proc/sys/kernel/random'])fs.mkdirSync(path.join(dir,p),{recursive:true,mode:0o700});
 return dir;
}
function record(changes={}){return {schema:2,uid,attempt:token,boot,operator:'99999999:1',lock:'',lockToken:'',phase:'prepared',masks:[],...changes}}
function writeRecord(dir,value){const f=path.join(dir,'home/user/.cache/omavless-maintenance/uninstall/progress.json');fs.writeFileSync(f,JSON.stringify(value),{mode:0o600});return f}
function userScript(dir,body){return [
 'set -euo pipefail','umask 077',functions,
 'HOME='+JSON.stringify(dir+'/home/user'),
 'folder="$HOME/.cache/omavless-maintenance/uninstall"',
 'record="$folder/progress.json"','lease="$folder/operation.lock"',
 'runtime='+JSON.stringify(dir+'/run/user/'+uid),'setup_lock="$runtime/omavless-first-run.lock"',
 'boot_id='+boot,'attempt='+nextToken,'operator="$$:$(process_birth "$$")"',
 'systemctl() { case "$*" in *LoadState*) printf "masked\\n";; *) :;; esac; }',
 body
 ].join('\n')}
function userTest(label,options,body,success,inspect=()=>{}){
 const dir=fixture();
 try{
  if(options.record)writeRecord(dir,options.record);
  const r=spawnSync('bash',['--noprofile','--norc'],{input:userScript(dir,body),encoding:'utf8',timeout:4000});
  assert(!r.error,label+': '+r.error);assert.equal(r.status===0,success,label+': '+r.stdout+r.stderr);
  inspect(dir,r);count++;
 }finally{fs.rmSync(dir,{recursive:true})}
}
userTest('no record is new removal',{},'read_progress; [[ $phase == none ]]',true);
for(const phase of ['preparing','prepared','removing'])userTest('interrupted '+phase+' remains reconcilable',{record:record({phase})},'read_progress; [[ $phase == '+phase+' ]]',true);
for(const changes of [{uid:uid+1},{attempt:'broken'},{operator:'0:0'},{lock:'123:456:0:700'},{lockToken:'bad'},{phase:'forced'},{masks:[{unit:'other.service',identity:'1:2'}]}]){
 userTest('reject invalid progress '+Object.keys(changes)[0],{record:record(changes)},'read_progress',false);
}
userTest('malformed or duplicate record refuses',{},'printf \'{"schema":2,"schema":2}\' > "$record"; read_progress',false);
userTest('wrong writable record refuses',{record:record()},'chmod 644 "$record"; read_progress',false);
userTest('symlink record refuses',{},'printf x > "$folder/target"; ln -s target "$record"; read_progress',false);
userTest('hardlink record refuses',{record:record()},'ln "$record" "$folder/other"; read_progress',false);
userTest('cached helper survives installed source removal',{},'original="$folder/original.sh"; printf "printf example" > "$original"; stage_resume_helper "$original"; unlink "$original"; safe_file "$helper_path"; [[ $(<"$helper_path") == "printf example" ]]; previous=$helper_path; stage_resume_helper "$helper_path"; [[ $helper_path == "$previous" ]]',true);
userTest('cached helper mismatch is never overwritten',{},'original="$folder/original.sh"; printf example > "$original"; stage_resume_helper "$original"; printf changed > "$helper_path"; stage_resume_helper "$original"',false,dir=>assert(fs.readdirSync(dir+'/home/user/.cache/omavless-maintenance/uninstall').some(p=>p.startsWith('uninstall-')&&fs.readFileSync(dir+'/home/user/.cache/omavless-maintenance/uninstall/'+p,'utf8')==='changed')));
userTest('cached helper symlink is never followed',{},'original="$folder/original.sh"; printf example > "$original"; digest=$(sha256sum "$original"); destination="$folder/uninstall-${digest%% *}.sh"; ln -s "$original" "$destination"; stage_resume_helper "$original"',false);
userTest('initial publication is durable and private',{},'read_progress; save_progress preparing; [[ $phase == preparing && $(stat -c %a "$record") == 600 ]]',true);
userTest('failed sync never reaches later effect',{},'sync() { return 1; }; read_progress; save_progress preparing; touch "$folder/FORBIDDEN"',false,dir=>assert(!fs.existsSync(dir+'/home/user/.cache/omavless-maintenance/uninstall/FORBIDDEN')));
userTest('fresh attempt rotates queued authorization token',{record:record()},'read_progress; save_progress "$phase"; [[ $(jq -r .attempt "$record") == '+nextToken+' ]]',true);
userTest('live exact operator cannot be mistaken for old PID',{record:record()},'read_progress; previous_operator="$$:$(process_birth "$$")"; operator_gone',false);
userTest('reused PID is not the previous operator',{record:record()},'read_progress; previous_operator="$$:0"; operator_gone',true);
userTest('record setup inode before shared publication',{},'read_progress; save_progress preparing; take_setup_lock; [[ $(jq -r .lock "$record") == $(stat -c %d:%i:%u:%a "$setup_lock") ]]; [[ -z $(find "$setup_lock" -mindepth 1 -print -quit) ]]',true);
userTest('resume crash after record but before shared publication',{},[
 'read_progress; save_progress preparing',
 'lock_token='+token,'temporary="$runtime/.omavless-uninstall-$lock_token"',
 'mkdir "$temporary"; recorded_lock=$(stat -c %d:%i:%u:%a "$temporary"); save_progress preparing',
 'original=$recorded_lock; take_setup_lock; [[ $recorded_lock == "$original" && ! -e "$temporary" ]]'
].join('\n'),true);
userTest('unpublished unrecorded directory never blocks shared lock',{},[
 'mkdir "$runtime/.omavless-uninstall-'+token+'"',
 'read_progress; save_progress preparing; take_setup_lock',
 '[[ -d "$runtime/.omavless-uninstall-'+token+'" && -d "$setup_lock" ]]'
].join('\n'),true);
userTest('foreign shared setup dir is not reclaimed',{},'mkdir "$setup_lock"; read_progress; save_progress preparing; take_setup_lock',false,dir=>assert(fs.existsSync(dir+'/run/user/'+uid+'/omavless-first-run.lock')));
userTest('unchanged own setup dir is reused without inode churn',{},'read_progress; save_progress preparing; take_setup_lock; old=$recorded_lock; take_setup_lock; [[ $recorded_lock == "$old" ]]',true);
userTest('nonempty setup dir is not reclaimed',{},'read_progress; save_progress preparing; take_setup_lock; touch "$setup_lock/other"; take_setup_lock',false,dir=>assert(fs.existsSync(dir+'/run/user/'+uid+'/omavless-first-run.lock/other')));
userTest('completion release preserves identifying record until successful',{},'read_progress; save_progress preparing; take_setup_lock; touch "$setup_lock/other"; release_setup_lock; unlink "$record"',false,dir=>assert(fs.existsSync(dir+'/home/user/.cache/omavless-maintenance/uninstall/progress.json')));
userTest('completion interruption after release remains resumable',{},'read_progress; save_progress preparing; take_setup_lock; release_setup_lock; read_progress; [[ -f "$record" && ! -e "$setup_lock" ]]; take_setup_lock; release_setup_lock; unlink "$record"',true);
userTest('created masks are removed after package cleanup',{},'read_progress; save_progress preparing; mask_future_start; [[ $(jq length <<< "$masks") == 2 ]]; clear_created_masks; [[ ! -L "$HOME/.config/systemd/user/omavless-runtime.service" ]]',true);
userTest('preexisting mask is kept',{},'ln -s /dev/null "$HOME/.config/systemd/user/omavless-runtime.service"; read_progress; save_progress preparing; mask_future_start; [[ $(jq length <<< "$masks") == 1 ]]; clear_created_masks; [[ -L "$HOME/.config/systemd/user/omavless-runtime.service" ]]',true);
userTest('custom service override is never overwritten',{},'printf original > "$HOME/.config/systemd/user/omavless-runtime.service"; read_progress; save_progress preparing; mask_future_start',false,dir=>assert.equal(fs.readFileSync(dir+'/home/user/.config/systemd/user/omavless-runtime.service','utf8'),'original'));
userTest('mask inode recorded before interrupted publish',{},[
 'read_progress; save_progress preparing',
 'ln() { if [[ "$*" == *"-P -T"* ]]; then return 7; else /usr/bin/ln "$@"; fi; }',
 'mask_future_start'
].join('\n'),false,dir=>{
 const progress=JSON.parse(fs.readFileSync(dir+'/home/user/.cache/omavless-maintenance/uninstall/progress.json'));
 assert.equal(progress.masks.length,1);
 const stat=fs.lstatSync(dir+'/home/user/.cache/omavless-maintenance/uninstall/mask-omavless-runtime.service');
 assert.equal(progress.masks[0].identity,stat.dev+':'+stat.ino);
 assert(!fs.existsSync(dir+'/home/user/.config/systemd/user/omavless-runtime.service'));
});
userTest('resume a recorded mask before publication',{},[
 'read_progress; save_progress preparing',
 'temporary="$folder/mask-omavless-runtime.service"; ln -s /dev/null "$temporary"',
 'identity=$(stat -c %d:%i "$temporary")',
 'masks=$(jq -cn --arg identity "$identity" \'[{unit:"omavless-runtime.service",identity:$identity}]\')',
 'save_progress preparing; mask_future_start; [[ -L "$HOME/.config/systemd/user/omavless-runtime.service" ]]'
].join('\n'),true);
userTest('replaced mask is never unlinked',{},'read_progress; save_progress preparing; mask_future_start; unlink "$HOME/.config/systemd/user/omavless-runtime.service"; printf replacement > "$HOME/.config/systemd/user/omavless-runtime.service"; clear_created_masks',false,dir=>assert.equal(fs.readFileSync(dir+'/home/user/.config/systemd/user/omavless-runtime.service','utf8'),'replacement'));
userTest('partial completed mask cleanup resumes',{},'read_progress; save_progress preparing; mask_future_start; unlink "$HOME/.config/systemd/user/omavless-runtime.service"; clear_created_masks; [[ ! -L "$HOME/.config/systemd/user/omavless-login-prepare.service" ]]',true);
for(const target of ['/usr/lib/systemd/user/omavless-runtime.service','../omavless-runtime.service'])userTest('remove only packaged runtime enablement '+target,{},'mkdir "$HOME/.config/systemd/user/default.target.wants"; ln -s '+JSON.stringify(target)+' "$HOME/.config/systemd/user/default.target.wants/omavless-runtime.service"; ln -s /other "$HOME/.config/systemd/user/default.target.wants/other.service"; clear_runtime_enablement; [[ ! -L "$HOME/.config/systemd/user/default.target.wants/omavless-runtime.service" && -L "$HOME/.config/systemd/user/default.target.wants/other.service" ]]',true);
userTest('custom runtime dependency target is retained',{},'mkdir "$HOME/.config/systemd/user/default.target.wants"; ln -s /other "$HOME/.config/systemd/user/default.target.wants/omavless-runtime.service"; clear_runtime_enablement',false,dir=>assert(fs.lstatSync(dir+'/home/user/.config/systemd/user/default.target.wants/omavless-runtime.service').isSymbolicLink()));
userTest('redirected user-service ancestor never unlinks target',{},'mv "$HOME/.config/systemd/user" "$folder/foreign"; ln -s "$folder/foreign" "$HOME/.config/systemd/user"; mkdir "$folder/foreign/default.target.wants"; ln -s /usr/lib/systemd/user/omavless-runtime.service "$folder/foreign/default.target.wants/omavless-runtime.service"; clear_runtime_enablement',false,dir=>assert(fs.lstatSync(dir+'/home/user/.cache/omavless-maintenance/uninstall/foreign/default.target.wants/omavless-runtime.service').isSymbolicLink()));
userTest('REMOVE does not ask connection history',{},'confirm_removal <<< REMOVE',true,(_,r)=>assert(!/never connected|used it before|ask for help|support/.test(r.stdout)));
userTest('cancel REMOVE has no removal phase',{},'confirm_removal <<< ""',false,dir=>assert(!fs.existsSync(dir+'/home/user/.cache/omavless-maintenance/uninstall/progress.json')));

// Fixed root checks are path-rewritten into a private fixture. Actual root
// ownership metadata is supplied by inert stat projection; no privilege used.
function rootScript(dir,body,{attempt=nextToken,phase='removing',originalBoot=boot}={}){
 const rewrite=s=>s.replace(/(?<![A-Za-z0-9_/.-])\/(usr|etc|run|proc|sys|var)(?=\/|\b)/g,(_,c)=>dir+'/'+c);
 const checks=rewrite(section('ROOT_CHECKS')),custody=rewrite(section('ROOT_CUSTODY'));
 const prefix=[
 'set -euo pipefail','umask 077','SUDO_UID='+uid,'removal_attempt='+attempt,'removal_phase='+phase,'removal_boot='+originalBoot,'invocation_boot='+newer,
 checks,
 'timeout() { shift; "$@"; }',
 'systemctl() { printf FORBIDDEN_SYSTEMCTL >&2; return 99; }; pacman() { printf FORBIDDEN_PACMAN >&2; return 99; }; ip() { printf FORBIDDEN_IP >&2; return 99; }; resolvectl() { printf FORBIDDEN_RESOLVER >&2; return 99; }',
 'getent() { printf "user:x:'+uid+':'+uid+':User:'+dir+'/home/user:/bin/bash\\n"; }',
 'stat() { local last="${@: -1}"; if [[ "$last" == '+JSON.stringify(dir+'/run/omavless-uninstall.lock')+' && "$2" == "%u:%a:%h" ]]; then printf "0:600:1\\n"; elif [[ "$last" == '+JSON.stringify(dir)+'/proc/*/fd/* ]]; then last=${last#'+JSON.stringify(dir)+'}; /usr/bin/stat "$1" "$2" "$last"; else /usr/bin/stat "$@"; fi; }',
 custody
 ];
 return prefix.join('\n')+'\n'+rewrite(body)+'\n';
}
function rootFixture(){const dir=fixture();writeRecord(dir,record({attempt:nextToken,phase:'removing'}));fs.writeFileSync(dir+'/proc/sys/kernel/random/boot_id',newer);return dir}
function rootTest(label,changes,body,success){const dir=rootFixture();try{
 if(changes.record)writeRecord(dir,changes.record);
 if(changes.before)changes.before(dir);
 const r=spawnSync('bash',['--noprofile','--norc'],{input:rootScript(dir,body,changes),encoding:'utf8',timeout:4000});
 assert(!r.error,label+': '+r.error);assert.equal(r.status===0,success,label+': '+r.stdout+r.stderr);count++;
}finally{fs.rmSync(dir,{recursive:true})}}
rootTest('current root epoch admitted',{},'printf ADMITTED',true);
rootTest('old queued authorization token is refused',{attempt:token},'printf FORBIDDEN',false);
rootTest('wrong root phase is refused',{phase:'prepared'},'printf FORBIDDEN',false);
rootTest('wrong originating boot is refused',{originalBoot:newer},'printf FORBIDDEN',false);
rootTest('missing completion record revokes late authorization',{before:dir=>fs.unlinkSync(dir+'/home/user/.cache/omavless-maintenance/uninstall/progress.json')},'printf FORBIDDEN',false);
rootTest('pending old sudo is not inferred absent from free lock',{before:dir=>{fs.mkdirSync(dir+'/proc/123');fs.writeFileSync(dir+'/proc/123/cmdline','sudo\0bash\0-c\0# OmaVLESS removal attempt '+token+'\n');}},'printf FORBIDDEN',false);
for(const bad of ['[]','{}','[{"servers":[]}]','[{"resolvConfMode":false,"fallbackServers":[],"scopes":[]}]','[{"ifname":"Meta","ifindex":7}]','[{"ifname":"omavless0","ifindex":8}]']){
 // Missing link/global schema and named old links are distinct from a healthy view.
 rootTest('resolver schema/link admission '+bad,{},'ip() { printf \'[]\'; }; resolvectl() { printf \'%s\' '+JSON.stringify(bad)+'; }; network_empty',false);
}
for(const family of ['-4','-6'])rootTest('residual '+family+' rule is not deleted',{},[
 'ip() { case "$*" in *"'+family+' rule"*) printf \'[{"priority":9000,"table":2022}]\';; *) printf \'[]\';; esac; }',
 'resolvectl() { printf \'[{"resolvConfMode":"stub","fallbackServers":[],"scopes":[]},{"ifname":"eth0","ifindex":2}]\'; }','network_empty'
].join('\n'),false);
// Positive schema measured on the actual guest, not invented from the parser.
// Address/domain/real scope values are replaced with empty synthetic values.
rootTest('actual installed resolver global/link schema permits clean view',{},'ip() { printf \'[]\'; }; resolvectl() { printf \'[{"fallbackServers":[],"negativeTrustAnchors":[],"dnssecSupported":false,"dnssec":"no","dnsOverTLS":"no","llmnr":"no","mDNS":"no","resolvConfMode":"stub","scopes":[]},{"ifname":"eth0","ifindex":2,"defaultRoute":true,"currentServer":{},"servers":[],"dnssecSupported":false,"dnssec":"no","dnsOverTLS":"no","llmnr":"no","mDNS":"no","scopes":[]}]\'; }; network_empty',true);
rootTest('resolver failure is not an empty view',{},'ip() { printf \'[]\'; }; resolvectl() { return 5; }; network_empty',false);
rootTest('genuinely absent journal is empty',{},'stat() { if [[ "$2" == "%u %a" ]]; then printf "0 755\\n"; else /usr/bin/stat "$@"; fi; }; empty_runtime_journal',true);
for(const kind of ['file','dangling','redirect','writable','nonempty'])rootTest('unexpected runtime journal '+kind,{},[
 'mkdir -p /run/omavless-dns',
 'stat() { if [[ "$2" == "%u %a" ]]; then printf "0 '+(kind==='writable'?'777':'700')+'\\n"; else /usr/bin/stat "$@"; fi; }',
 kind==='file'?'touch /run/omavless-dns/private':kind==='dangling'?'ln -s /missing /run/omavless-dns/private':kind==='redirect'?'mkdir /run/other; ln -s ../other /run/omavless-dns/private':'mkdir /run/omavless-dns/private',
 kind==='nonempty'?'touch /run/omavless-dns/private/journal':':',
 'empty_runtime_journal'
].join('\n'),false);

assert(source.indexOf('run_root \':\'')<source.indexOf('\ntake_setup_lock\n'));
assert(section('ROOT_CUSTODY').indexOf('flock -n')<section('ROOT_CUSTODY').indexOf('current=$(<"$record")'));
assert(section('ROOT_REMOVE').includes('pacman -R -- "${packages[@]}"'));
assert(!source.includes('sudo pacman'));
assert(!/never.connected|ask for help|send .*support|kill -|reset-failed|--nodeps|--overwrite|--noconfirm/.test(source));
assert(source.includes('"$phase" == preparing && "$prepared_boot" != "$boot_id"'));
assert(source.indexOf('save_progress "$phase"\n  mv -T --no-clobber')>0);
assert(source.lastIndexOf('release_setup_lock')<source.lastIndexOf('unlink -- "$record"'));

async function rootMutex(){
 const dir=rootFixture();
 try{
  // Dedicated original process with a real FIFO, not sleep-as-effect evidence.
  const fifo=dir+'/continue';assert.equal(spawnSync('mkfifo',[fifo]).status,0);
  const filename=dir+'/root-worker.sh';fs.writeFileSync(filename,rootScript(dir,'printf "ADMITTED\\n"; read -r answer < '+JSON.stringify(fifo)+'; printf "ORIGINAL_COMPLETED\\n"'));
  const owner=spawn('bash',[filename],{stdio:['ignore','pipe','pipe']});let live='';owner.stdout.on('data',c=>live+=c);let error='';owner.stderr.on('data',c=>error+=c);
  const deadline=Date.now()+3000;while(!live.includes('ADMITTED')){assert(Date.now()<deadline,'original root fixture did not admit: '+error);await new Promise(r=>setTimeout(r,10));}
  writeRecord(dir,record({attempt:token,phase:'removing'}));
  const concurrent=spawnSync('bash',['--noprofile','--norc'],{input:rootScript(dir,'printf FORBIDDEN',{attempt:token}),encoding:'utf8',timeout:2000});
  assert.notEqual(concurrent.status,0);assert(!concurrent.stdout.includes('FORBIDDEN'));assert(concurrent.stderr.includes('still running'));
  const complete=new Promise(resolve=>owner.once('exit',resolve));
  const writer=spawn('bash',['-c','printf "go\\n" > "$1"','fixture',fifo]);await new Promise(r=>writer.once('exit',r));
  assert.equal(await complete,0,error);assert(live.includes('ORIGINAL_COMPLETED'));
  const fresh=spawnSync('bash',['--noprofile','--norc'],{input:rootScript(dir,'printf RECONCILED',{attempt:token}),encoding:'utf8',timeout:2000});
  assert.equal(fresh.status,0,fresh.stderr);assert.equal(fresh.stdout,'RECONCILED');count+=2;
 }finally{fs.rmSync(dir,{recursive:true})}
}
rootMutex().then(()=>console.log('Uninstaller successor: '+count+' inert progress/mask/custody/network cases PASS; no host effects.')).catch(e=>{console.error(e);process.exitCode=1});

#!/usr/bin/env node
// Production successor functions; inert filesystem/processes only. No real sudo,
// package manager, service manager, resolver, network or desktop operations.
const fs=require('node:fs'), os=require('node:os'), path=require('node:path');
const assert=require('node:assert/strict');
const {spawnSync,spawn}=require('node:child_process');
const {gzipSync}=require('node:zlib'),{createHash}=require('node:crypto');
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
// Bind removal admission to the selected delivery, not only old hand-written
// public fixtures. Exercise the shipped successor function without executing
// the app, package manager, service manager, native Quit or any root branch.
const selected=JSON.parse(fs.readFileSync(path.join(__dirname,'../plugin/component-updates.json'),'utf8'));
const published=JSON.parse(fs.readFileSync(path.join(__dirname,'../plugin/published-pairs.json'),'utf8'));
const selectedVersion=JSON.parse(fs.readFileSync(path.join(__dirname,'../manifest.json'),'utf8')).version+'-1';
const classification=source.slice(source.indexOf('check_supported_installation()'),source.indexOf('\n[[ $# == 0 ]]'));
assert.deepEqual(Object.keys(selected.identities).sort(),['aarch64','x86_64']);
const removalIdentities=[
 ...Object.entries(selected.identities).map(([arch,identity])=>['selected '+arch,identity.appSha]),
 ...published.pairs.map(identity=>['predecessor '+identity.sourceCommit+' '+identity.architecture,identity.appSha]),
 ['public082 x86_64','3ff795316aa331463aff4e26571546433bdfd9317f348712d135be2f0a8d28a1'],
 ['public082 aarch64','62874def77df9e4f099baf74c57420a7ab1dab7b83935676828f6d445a5f357f']
];
function classify(label,digest,expected,options={}){
 const dir=fs.mkdtempSync(path.join(parent,'uninstall-admission.'));
 const app=path.join(dir,'omavless');
 if(!options.appAbsent)fs.writeFileSync(app,'inert fixture');
 if(options.symlink){fs.renameSync(app,app+'.target');fs.symlinkSync(app+'.target',app)}
 const appVersion=options.version||'0.9.8-1';
 const dns=options.dnsAbsent||appVersion==='0.8.2-1'?'return 1':"printf '%s\\n' 'omavless-dns "+(options.dnsVersion||appVersion)+"'";
 const script=`set -euo pipefail
fail() { printf 'REFUSED:%s\\n' "$*"; exit 1; }
${classification.replaceAll('/usr/bin/omavless',app)}
pacman() { case "$*" in '-Q omavless') ${options.appAbsent?'return 1':"printf '%s\\n' 'omavless "+appVersion+"'"} ;; '-Q omavless-dns') ${dns} ;; *) exit 91 ;; esac; }
stat() { [[ "$*" == '-c %u:%a ${app}' ]] || exit 92; printf '%s\\n' '${options.metadata||'0:755'}'; }
sha256sum() { [[ "$*" == '${app}' ]] || exit 93; printf '%s  %s\\n' '${digest}' "$1"; }
check_supported_installation
`;
 try{
  const result=spawnSync('bash',['--noprofile','--norc'],{input:script,encoding:'utf8',timeout:3000});
  assert(!result.error,label+': '+result.error);
  assert.equal(result.status,expected?0:1,label+': '+result.stdout+result.stderr);
  count++;
 }finally{fs.rmSync(dir,{recursive:true})}
}
for(const [label,digest] of removalIdentities){
 assert.match(digest,/^[a-f0-9]{64}$/);
 classify(label,digest,true,label.startsWith('public082')?{version:'0.8.2-1'}:label.startsWith('selected ')?{version:selectedVersion}:{});
}
// Keep the exact previous installed development pair removable for rollback;
// this is not generic admission for another 0.9.9 rebuild.
if(selectedVersion==='0.9.9-1')for(const [arch,digest] of [
 ['x86_64','35b188585a39c18b1a65417a79303212ddab50e66dc777ef608227376aa9f46a'],
 ['aarch64','6698fd6fbb8763e469b70be70670896e6b19d804785da95fd006762bef1afe24'],
 ['x86_64','3f68837894e84256e622b9e33013ef6da0c9389d5f48069771e4ec9d916161f1'],
 ['aarch64','79184dfe954582d046f53e220a085ac335646ef0d8fe6a8dd5174a911f24f440'],
 ['x86_64','dc23f5694265439439c35ec7977bb5544369421c31caeb0f167cab2845c219eb'],
 ['aarch64','566adcabcf3a045fb2f8398dd010f6a9083cd2531827f9f45dd8fa1c820a807f'],
 ['x86_64','572e960a3fe1dde061a1cc0352c52a05a3b6b42ea7978f0bddc1d330c247c2ab'],
 ['aarch64','3aea4094ee1788ab716407df0566299d96d34eaad9d4ef79658c33118d5baebd'],
 ['x86_64','765bf199cc3bf5f64c10ccbb45400799d7524cf7f0b52fe6ba9e97e37613178a'],
 ['aarch64','3b992c5fbe3cd58525a6f2f38e58a727e42a4075b39b371b0e4259b17278c20a'],
 ['x86_64','5a6ce1d4fb84c8ff1686d66a5ec754bbd09848ca26adc4c5afe45e5f43a49df3'],
 ['aarch64','e0d008c5a1b6d065900f36eb07b5346cab870ebb291334f843a3180a6a0f2ec0'],
 ['x86_64','4254775ce5b4902d02dbb104056f1b201d8f6fa3b6a4ffce1aa8f3df50acd75b'],
 ['aarch64','6e8245e60751c5484728bde5cbe1d5d795269ef27bf5cbfefc45cee2ace8390d'],
 ['x86_64','b0cca9a8e6b86eb5b842d62c9732c104bf0663e4db20b2abb4da88393add670c'],
 ['aarch64','c0306c0f7ccee558ba4cbe7b2bd70d6c38e4126f25741174ad4900a8cde31788'],
 ['x86_64','09059067789ccc0ce29d0cdcdaef319afde992221137fdffd117e57ff2847565'],
 ['aarch64','9c9d616cacf4c9c0143e59f045454babbcb33d515930e83ff9f062af016953e4']
]){
 classify('retained inspected nonpublic app '+arch,digest,true,{version:selectedVersion});
 classify('retained nonpublic app cannot acquire public label '+arch,digest,false,{version:'0.9.8-1'});
}
for(const [arch,identity] of Object.entries(selected.identities)){
 const changed=identity.appSha.slice(0,-1)+(identity.appSha.endsWith('0')?'1':'0');
 assert(!removalIdentities.some(([,digest])=>digest===changed));
 classify('modified selected '+arch,changed,false,{version:selectedVersion});
 classify('selected hash under development label '+arch,identity.appSha,false,{version:'0.9.8.rtest-1'});
 classify('selected symlink '+arch,identity.appSha,false,{version:selectedVersion,symlink:true});
 for(const metadata of ['1000:755','0:777'])
  classify('selected unsafe metadata '+arch+' '+metadata,identity.appSha,false,{version:selectedVersion,metadata});
 if(selectedVersion==='0.9.9-1'){
  classify('099 selected app without DNS '+arch,identity.appSha,true,{version:selectedVersion,dnsAbsent:true});
  classify('099 DNS without app '+arch,identity.appSha,true,{version:selectedVersion,appAbsent:true});
  classify('099 app with public DNS '+arch,identity.appSha,false,{version:selectedVersion,dnsVersion:'0.9.8-1'});
  classify('099 ELF under public label '+arch,identity.appSha,false,{version:'0.9.8-1'});
  classify('099 ELF under unknown label '+arch,identity.appSha,false,{version:'0.9.10-1'});
  for(const predecessor of published.pairs)
   classify('public ELF under099 label '+arch+' '+predecessor.sourceCommit,predecessor.appSha,false,{version:selectedVersion});
  for(const predecessor of published.pairs)
   classify('public app with099 DNS '+arch+' '+predecessor.sourceCommit,predecessor.appSha,false,{dnsVersion:selectedVersion});
 }
}
// A superseded nonpublic producer is not this candidate's selected 0.9.9 ELF.
// Public predecessor admission remains covered independently above.
if(selectedVersion==='0.9.9-1')for(const [arch,digest] of [
 ['x86_64','f0c5771d60a024328c89b246a10c65c72e66d207a4d519b45c62c0568cab505a'],
 ['aarch64','6f57fdf6e67196bcb22ca4024bfb3265644c93ab14a182d79fe575f76045a404']
])classify('superseded nonpublic082 app under099 label '+arch,digest,false,{version:selectedVersion});
if(selectedVersion==='0.9.9-1')for(const [arch,digest] of [
 ['x86_64','3b35e526a6bf19dbd35c7918b006c397cde0e8dce7a65d4247abc7e72dbe728d'],
 ['aarch64','14c75c3ca3848b5d67b1d2965fb13b1d8e00bef2b82a13a74d6ed07f8b4298fd']
])classify('superseded nonpublic6021 app under099 label '+arch,digest,false,{version:selectedVersion});
if(selectedVersion==='0.9.9-1')for(const [arch,digest] of [
 ['x86_64','9d4aa2d13fabb8330409ddef3438dde5870e20cb40d248cfdd12cc8adbbd2cec'],
 ['aarch64','08b83d3ef71b46c41521f5ad190e6b2fb6f613b3b92422d6c984d9e4da1076b2']
])classify('superseded nonpublicf788 app under099 label '+arch,digest,false,{version:selectedVersion});
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
 'timeout() { shift; "$@"; }',
 'omarchy() { printf FORBIDDEN_DESKTOP >&2; return 99; }',
 'function /usr/bin/omavless { printf FORBIDDEN_NATIVE >&2; return 99; }',
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
userTest('literal old bootstrap parent755 admits private child700',{},'chmod 755 "$HOME/.cache/omavless-maintenance"; rmdir "$folder"; prepare_maintenance_folders; [[ $(stat -c %a "$folder") == 700 ]]',true);
userTest('shared writable download parent is not admitted',{},'chmod 777 "$HOME/.cache/omavless-maintenance"; prepare_maintenance_folders',false);
userTest('uninstall private child755 remains refused',{},'chmod 755 "$folder"; prepare_maintenance_folders',false);
userTest('redirected maintenance parent is left unchanged',{},'mv "$HOME/.cache/omavless-maintenance" "$HOME/.cache/other"; ln -s other "$HOME/.cache/omavless-maintenance"; prepare_maintenance_folders',false);
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
 assert.match(progress.masks[0].witness,/^u[a-f0-9]{32}$/);
 const stat=fs.lstatSync(dir+'/home/user/.config/systemd/user/.omavless-uninstall-'+progress.masks[0].witness.slice(1)+'-omavless-runtime.service.link');
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
const adjacentWitness='"$HOME/.config/systemd/user/.omavless-uninstall-'+nextToken+'-omavless-runtime.service.link"';
userTest('new witness is retained as the exact second hardlink',{},'read_progress; save_progress preparing; mask_future_start; witness='+adjacentWitness+'; [[ $(stat -c %h "$witness") == 2 && $(stat -c %d:%i "$witness") == $(stat -c %d:%i "$HOME/.config/systemd/user/omavless-runtime.service") ]]',true);
const deviceDrift='stat() { if [[ "$2" == "%d:%i" ]]; then printf "%s:%s\\n" "$(( $(/usr/bin/stat -c %d "${@: -1}") + 1 ))" "$(/usr/bin/stat -c %i "${@: -1}")"; else /usr/bin/stat "$@"; fi; }';
userTest('new witnessed masks survive changed device id across reboot',{},'read_progress; save_progress preparing; mask_future_start; boot_id='+newer+'; '+deviceDrift+'; mask_future_start; clear_created_masks; [[ ! -L "$HOME/.config/systemd/user/omavless-runtime.service" && ! -L '+adjacentWitness+' ]]',true);
userTest('new witness replacement refuses before either unlink',{},'read_progress; save_progress preparing; mask_future_start; witness='+adjacentWitness+'; unlink "$witness"; ln -s /dev/null "$witness"; clear_created_masks',false,dir=>assert(fs.lstatSync(dir+'/home/user/.config/systemd/user/omavless-runtime.service').isSymbolicLink()));
userTest('missing new witness never downgrades into legacy cleanup',{},'read_progress; save_progress preparing; mask_future_start; unlink '+adjacentWitness+'; phase=removing; prepared_boot='+boot+'; boot_id='+newer+'; mask_cleanup_admitted=true; clear_created_masks',false,dir=>assert(fs.lstatSync(dir+'/home/user/.config/systemd/user/omavless-runtime.service').isSymbolicLink()));
userTest('interruption after destination unlink preserves remaining witness',{},'read_progress; save_progress preparing; mask_future_start; unlink() { if [[ "${@: -1}" == *".omavless-uninstall-"* ]]; then return 7; else /usr/bin/unlink "$@"; fi; }; clear_created_masks',false,dir=>{
 assert(!fs.existsSync(dir+'/home/user/.config/systemd/user/omavless-runtime.service'));
 assert.equal(fs.lstatSync(dir+'/home/user/.config/systemd/user/.omavless-uninstall-'+nextToken+'-omavless-runtime.service.link').nlink,1);
});
userTest('unrecorded prepublication private witness never blocks next attempt',{},'orphan=$(mask_witness_path u'+token+' omavless-runtime.service); ln -s /dev/null "$orphan"; read_progress; save_progress preparing; mask_future_start; [[ -L "$orphan" ]]; clear_created_masks; [[ -L "$orphan" ]]',true);
userTest('separate cache filesystem does not require a cross-device hardlink',{},'ln() { if [[ "$1" == -P && "$*" == *"$folder"* ]]; then printf EXDEV >&2; return 18; else /usr/bin/ln "$@"; fi; }; read_progress; save_progress preparing; mask_future_start; clear_created_masks',true);
userTest('old dd cache witness remains supported',{},[
 'read_progress; save_progress preparing; temporary="$folder/mask-'+token+'-omavless-runtime.service"; ln -s /dev/null "$temporary"',
 'identity=$(stat -c %d:%i "$temporary"); masks=$(jq -cn --arg identity "$identity" --arg witness '+token+' \'[{unit:"omavless-runtime.service",identity:$identity,witness:$witness}]\'); save_progress preparing',
 'mask_future_start; clear_created_masks; [[ ! -L "$temporary" ]]'
].join('\n'),true);
userTest('mask cleanup sync failure keeps progress after unlink',{},'read_progress; save_progress preparing; mask_future_start; sync() { if [[ "$*" == *".config/systemd/user"* ]]; then return 7; else /usr/bin/sync "$@"; fi; }; clear_created_masks; unlink "$record"',false,dir=>assert(fs.existsSync(dir+'/home/user/.cache/omavless-maintenance/uninstall/progress.json')));
userTest('existing-pair retry must earn unit-filesystem durability',{},'read_progress; save_progress preparing; mask_future_start; sync() { if [[ "$*" == *".config/systemd/user"* ]]; then return 7; else /usr/bin/sync "$@"; fi; }; mask_future_start; touch "$folder/FORBIDDEN"',false,dir=>assert(!fs.existsSync(dir+'/home/user/.cache/omavless-maintenance/uninstall/FORBIDDEN')));
const legacyMask=[
 'destination="$HOME/.config/systemd/user/omavless-runtime.service"; ln -s /dev/null "$destination"',
 'identity="$(( $(stat -c %d "$destination") + 1 )):$(stat -c %i "$destination")"',
 'read_progress; masks=$(jq -cn --arg identity "$identity" \'[{unit:"omavless-runtime.service",identity:$identity}]\'); save_progress preparing'
].join('\n');
for(const [label,admitted,phase,changedBoot,success] of [
 ['actual b4 postboot removing recovery',true,'removing',true,true],
 ['without fresh root absence',false,'removing',true,false],
 ['same boot',true,'removing',false,false],
 ['still preparing',true,'preparing',true,false]
])userTest('legacy device drift '+label,{},legacyMask+'\nmask_cleanup_admitted='+admitted+'; phase='+phase+'; boot_id='+(changedBoot?newer:boot)+'; clear_created_masks',success);
userTest('legacy changed inode is not removable after reboot',{},legacyMask+'\nphase=removing; boot_id='+newer+'; mask_cleanup_admitted=true; mv "$destination" "$folder/retained-original"; ln -s /dev/null "$destination"; clear_created_masks',false,dir=>assert(fs.lstatSync(dir+'/home/user/.config/systemd/user/omavless-runtime.service').isSymbolicLink()));
userTest('witness token cannot name a path',{record:record({masks:[{unit:'omavless-runtime.service',identity:'1:2',witness:'../../other'}]})},'read_progress',false);
for(const target of ['/usr/lib/systemd/user/omavless-runtime.service','../omavless-runtime.service'])userTest('remove only packaged runtime enablement '+target,{},'mkdir "$HOME/.config/systemd/user/default.target.wants"; ln -s '+JSON.stringify(target)+' "$HOME/.config/systemd/user/default.target.wants/omavless-runtime.service"; ln -s /other "$HOME/.config/systemd/user/default.target.wants/other.service"; clear_runtime_enablement; [[ ! -L "$HOME/.config/systemd/user/default.target.wants/omavless-runtime.service" && -L "$HOME/.config/systemd/user/default.target.wants/other.service" ]]',true);
userTest('custom runtime dependency target is retained',{},'mkdir "$HOME/.config/systemd/user/default.target.wants"; ln -s /other "$HOME/.config/systemd/user/default.target.wants/omavless-runtime.service"; clear_runtime_enablement',false,dir=>assert(fs.lstatSync(dir+'/home/user/.config/systemd/user/default.target.wants/omavless-runtime.service').isSymbolicLink()));
userTest('redirected user-service ancestor never unlinks target',{},'mv "$HOME/.config/systemd/user" "$folder/foreign"; ln -s "$folder/foreign" "$HOME/.config/systemd/user"; mkdir "$folder/foreign/default.target.wants"; ln -s /usr/lib/systemd/user/omavless-runtime.service "$folder/foreign/default.target.wants/omavless-runtime.service"; clear_runtime_enablement',false,dir=>assert(fs.lstatSync(dir+'/home/user/.cache/omavless-maintenance/uninstall/foreign/default.target.wants/omavless-runtime.service').isSymbolicLink()));
userTest('REMOVE does not ask connection history',{},'confirm_removal <<< REMOVE',true,(_,r)=>assert(!/never connected|used it before|ask for help|support/.test(r.stdout)));
userTest('cancel REMOVE has no removal phase',{},'confirm_removal <<< ""',false,dir=>assert(!fs.existsSync(dir+'/home/user/.cache/omavless-maintenance/uninstall/progress.json')));

// Measured ARM64 failure: Quickshell exits0 but prefixes JSON with Qt's locale
// warning under LC_ALL=C. Fix the desktop child's environment, not the parser.
const localeList='export LC_ALL=C; omarchy() { [[ "$*" == "plugin list --json" ]] || return 9; : > "$folder/desktop-child"; if [[ $LC_ALL != C.UTF-8 ]]; then printf "Qt non-UTF-8 locale warning\\n"; fi; printf \'[{"id":"kdk.omavless","enabled":true}]\'; }';
userTest('plugin inventory uses child-only UTF-8 locale',{},localeList+'\n[[ $(plugin_state) == enabled && $LC_ALL == C && -f "$folder/desktop-child" ]]',true);
userTest('warning-prefixed registry still fails strict JSON',{},'omarchy() { printf \'warning\\n[{"id":"kdk.omavless","enabled":true}]\'; }; plugin_state',false);
userTest('failed registry query is not success despite JSON',{},'omarchy() { printf \'[{"id":"kdk.omavless","enabled":true}]\'; return 7; }; plugin_state',false);
userTest('ordinary widget removal gets UTF-8 and leaves parent C',{},[
 'export LC_ALL=C; mkdir -p "$HOME/.config/omarchy/plugins/kdk.omavless"',
 'omarchy() { [[ $LC_ALL == C.UTF-8 && "$*" == "plugin remove kdk.omavless" ]] || return 9; rmdir -- "$HOME/.config/omarchy/plugins/kdk.omavless"; }',
 'remove_widget; [[ $LC_ALL == C && ! -e "$HOME/.config/omarchy/plugins/kdk.omavless" ]]'
].join('\n'),true);
userTest('cancelled UTF-8 widget removal retains checkout',{},[
 'mkdir -p "$HOME/.config/omarchy/plugins/kdk.omavless"',
 'omarchy() { [[ $LC_ALL == C.UTF-8 ]] || return 9; return 7; }','remove_widget'
].join('\n'),false,dir=>assert(fs.existsSync(dir+'/home/user/.config/omarchy/plugins/kdk.omavless')));

// Execute the unchanged production destructor's fixed OS wrapper separately.
// Only its binary path is redirected into this private fixture; no installed
// native command, shell IPC, notification or QML process is started. Coupled
// with the final-phase tests below this proves why package-before-widget order
// skips the observer, without pretending that enabled-QML VM unload was tested.
const qml=fs.readFileSync(path.join(__dirname,'../plugin/Service.qml'),'utf8');
const guardMatch=qml.match(/execDetached\(\["\/bin\/sh", "-c", ("if \[ -x \/usr\/bin\/omavless[^\n]+?")\]\)/);
assert(guardMatch,'fixed native removal wrapper');
const nativeGuard=JSON.parse(guardMatch[1]);
assert.equal(nativeGuard,'if [ -x /usr/bin/omavless ]; then exec /usr/bin/omavless plugin watch-removal; fi',
 'refuse an unexpected shell program before executing the fixture');
assert.equal(nativeGuard.split('/usr/bin/omavless').length,3);
for(const [state,expected] of [['absent',0],['nonexecutable',0],['executable',7]]){
 const dir=fixture();
 try{
  const binary=path.join(dir,'fixed-native');
  if(state!=='absent')fs.writeFileSync(binary,'#!/bin/sh\nprintf "OBSERVER:%s\\n" "$*"\nexit 7\n',{mode:state==='executable'?0o700:0o600});
  const quoted="'"+binary.replaceAll("'","'\\''")+"'";
  const r=spawnSync('/bin/sh',['-c',nativeGuard.replaceAll('/usr/bin/omavless',quoted)],{encoding:'utf8',timeout:2000});
  assert(!r.error,state+': '+r.error);assert.equal(r.status,expected,state+': '+r.stderr);
  assert.equal(r.stdout,state==='executable'?'OBSERVER:plugin watch-removal\n':'');count++;
 }finally{fs.rmSync(dir,{recursive:true})}
}

// Exercise the actual final user-side phase, including its actual root body
// construction, with an inert root dispatcher. The existing ROOT_REMOVE tests
// independently execute those guarded package/network leaves. This is order
// and interruption evidence, not permission to invoke the real root worker.
const completionStart=source.indexOf('\nsave_progress removing\n');
const completionEnd=source.indexOf('\ncheck_private_targets\n[[ -d "$setup_lock"',completionStart);
assert(completionStart>0&&completionEnd>completionStart);
const completion=source.slice(completionStart+1,completionEnd)
 .replaceAll('/usr/bin/omavless','"$folder/app-file"');
const completionFixture=[
 'read_progress; save_progress prepared; take_setup_lock',
 'mkdir -p "$HOME/.config/omarchy/plugins/kdk.omavless"',
 'printf app > "$folder/app-file"; printf settings > "$HOME/.config/omavless-settings-witness"',
 'run_root() { printf "ROOT\\n" >> "$folder/order"; [[ ! -e "$folder/fail-package" ]] || return 7; [[ -e "$folder/keep-binary" ]] || rm -f -- "$folder/app-file"; printf "PACKAGES_ABSENT\\n" >> "$folder/order"; }',
 'omarchy() { [[ $LC_ALL == C.UTF-8 && "$*" == "plugin remove kdk.omavless" && ! -e "$folder/app-file" && ! -L "$folder/app-file" ]] || return 9; printf "WIDGET\\n" >> "$folder/order"; [[ ! -e "$folder/fail-widget" ]] || return 7; rmdir -- "$HOME/.config/omarchy/plugins/kdk.omavless"; }'
].join('\n');
const completionInspect=(dir)=>{
 const base=dir+'/home/user';
 assert(fs.existsSync(base+'/.cache/omavless-maintenance/uninstall/progress.json'));
 assert.equal(JSON.parse(fs.readFileSync(base+'/.cache/omavless-maintenance/uninstall/progress.json')).phase,'removing');
 assert(fs.existsSync(dir+'/run/user/'+uid+'/omavless-first-run.lock'));
 assert.equal(fs.readFileSync(base+'/.config/omavless-settings-witness','utf8'),'settings');
};
userTest('actual completion removes packages before widget unload',{},completionFixture+'\n'+completion,true,(dir)=>{
 completionInspect(dir);
 assert.equal(fs.readFileSync(dir+'/home/user/.cache/omavless-maintenance/uninstall/order','utf8'),'ROOT\nPACKAGES_ABSENT\nWIDGET\n');
 assert(!fs.existsSync(dir+'/home/user/.config/omarchy/plugins/kdk.omavless'));
});
userTest('package failure never unloads widget or deletes data',{},completionFixture+'\ntouch "$folder/fail-package"\n'+completion,false,(dir)=>{
 completionInspect(dir);
 assert(fs.existsSync(dir+'/home/user/.config/omarchy/plugins/kdk.omavless'));
 assert.equal(fs.readFileSync(dir+'/home/user/.cache/omavless-maintenance/uninstall/order','utf8'),'ROOT\n');
});
userTest('remaining unregistered binary blocks widget unload',{},completionFixture+'\ntouch "$folder/keep-binary"\n'+completion,false,(dir)=>{
 completionInspect(dir);
 assert(fs.existsSync(dir+'/home/user/.config/omarchy/plugins/kdk.omavless'));
 assert(!fs.readFileSync(dir+'/home/user/.cache/omavless-maintenance/uninstall/order','utf8').includes('WIDGET'));
});
userTest('widget cancellation after packages preserves continuation and data',{},completionFixture+'\ntouch "$folder/fail-widget"\n'+completion,false,(dir)=>{
 completionInspect(dir);
 assert(fs.existsSync(dir+'/home/user/.config/omarchy/plugins/kdk.omavless'));
 assert(!fs.existsSync(dir+'/home/user/.cache/omavless-maintenance/uninstall/app-file'));
});
userTest('same actual completion resumes after widget cancellation',{},completionFixture+'\n'+[
 'touch "$folder/fail-widget"',
 'if ( '+completion+'\n); then exit 9; fi',
 '[[ -e "$record" && -d "$setup_lock" && ! -e "$folder/app-file" ]]',
 'unlink "$folder/fail-widget"; read_progress',completion
].join('\n'),true,(dir)=>{
 completionInspect(dir);
 assert(!fs.existsSync(dir+'/home/user/.config/omarchy/plugins/kdk.omavless'));
 assert.equal(fs.readFileSync(dir+'/home/user/.cache/omavless-maintenance/uninstall/order','utf8'),'ROOT\nPACKAGES_ABSENT\nWIDGET\nROOT\nPACKAGES_ABSENT\nWIDGET\n');
});
for(const [label,setup] of [
 ['redirected widget','rmdir "$HOME/.config/omarchy/plugins/kdk.omavless"; ln -s "$folder" "$HOME/.config/omarchy/plugins/kdk.omavless"'],
 ['dirty Git directory','mkdir "$HOME/.config/omarchy/plugins/kdk.omavless/.git"; git() { printf modified; }'],
 ['dirty Git worktree','printf worktree > "$HOME/.config/omarchy/plugins/kdk.omavless/.git"; git() { printf modified; }'],
 ['failed Git status','mkdir "$HOME/.config/omarchy/plugins/kdk.omavless/.git"; git() { return 7; }']
])userTest(label+' refuses before package effects',{},completionFixture+'\n'+setup+'\n'+completion,false,(dir)=>{
 completionInspect(dir);
 assert(!fs.existsSync(dir+'/home/user/.cache/omavless-maintenance/uninstall/order'));
 assert(fs.existsSync(dir+'/home/user/.cache/omavless-maintenance/uninstall/app-file'));
});
for(const status of [0,7])userTest('native Quit UTF-8 inherited by its desktop child, original exit'+status,{},[
 'export LC_ALL=C; guided=false',
 'function /usr/bin/omavless { [[ $LC_ALL == C.UTF-8 ]] || return 9; bash -c \'[[ $LC_ALL == C.UTF-8 ]]\' || return 9; printf \'{"synthetic":true}\'; return '+status+'; }',
 'original=0; native_shutdown plugin watch-removal || original=$?; [[ $original == '+status+' && $LC_ALL == C ]]'
].join('\n'),true);
// Execute the guided production branch with a real inert PTY too: changing only
// the synchronous path would leave ordinary interactive removal broken.
for(const status of [0,7]){
 const dir=fixture();
 try{
  const script=dir+'/guided-utf8.sh';
  fs.writeFileSync(script,userScript(dir,[
   'export LC_ALL=C; guided=true; spinner="|/-"; stage_directory="$folder"',
   'function /usr/bin/omavless { [[ $LC_ALL == C.UTF-8 ]] || return 9; bash -c \'[[ $LC_ALL == C.UTF-8 ]]\' || return 9; sleep 0.2; printf \'{"synthetic":true}\'; return '+status+'; }',
   'original=0; native_shutdown plugin watch-removal || original=$?; [[ $original == '+status+' && $LC_ALL == C ]]'
  ].join('\n')),{mode:0o600});
  const r=spawnSync('script',['-q','-e','-c','bash --noprofile --norc '+script,'/dev/null'],{encoding:'utf8',timeout:4000});
  assert(!r.error,String(r.error));assert.equal(r.status,0,r.stdout+r.stderr);
  assert(r.stdout.includes('Closing OmaVLESS...')&&r.stdout.includes('{"synthetic":true}'));
  assert.deepEqual(fs.readdirSync(dir+'/home/user/.cache/omavless-maintenance/uninstall'),[]);count++;
 }finally{fs.rmSync(dir,{recursive:true})}
}

// Fixed root checks are path-rewritten into a private fixture. Actual root
// ownership metadata is supplied by inert stat projection; no privilege used.
function rootScript(dir,body,{attempt=nextToken,phase='removing',originalBoot=boot,prologue=''}={}){
 const rewrite=s=>s.replace(/(?<![A-Za-z0-9_/.-])\/(usr|etc|run|proc|sys|var)(?=\/|\b)/g,(_,c)=>dir+'/'+c);
 const checks=rewrite(section('ROOT_CHECKS')),custody=rewrite(section('ROOT_CUSTODY'));
 const prefix=[
 'set -euo pipefail','umask 077','SUDO_UID='+uid,'removal_attempt='+attempt,'removal_phase='+phase,'removal_boot='+originalBoot,'invocation_boot='+newer,
 checks,
 'timeout() { shift; "$@"; }',
 'systemctl() { printf FORBIDDEN_SYSTEMCTL >&2; return 99; }; pacman() { printf FORBIDDEN_PACMAN >&2; return 99; }; ip() { printf FORBIDDEN_IP >&2; return 99; }; resolvectl() { printf FORBIDDEN_RESOLVER >&2; return 99; }',
 'getent() { printf "user:x:'+uid+':'+uid+':User:'+dir+'/home/user:/bin/bash\\n"; }',
 'stat() { local last="${@: -1}"; if [[ "$last" == '+JSON.stringify(dir+'/run/omavless-uninstall.lock')+' && "$2" == "%u:%a:%h" ]]; then printf "0:600:1\\n"; elif [[ "$last" == '+JSON.stringify(dir)+'/proc/*/fd/* ]]; then last=${last#'+JSON.stringify(dir)+'}; /usr/bin/stat "$1" "$2" "$last"; else /usr/bin/stat "$@"; fi; }',
 rewrite(prologue),custody
 ];
 return prefix.join('\n')+'\n'+rewrite(body)+'\n';
}
function rootFixture(){const dir=fixture();writeRecord(dir,record({attempt:nextToken,phase:'removing'}));fs.writeFileSync(dir+'/proc/sys/kernel/random/boot_id',newer);return dir}
function rootTest(label,changes,body,success,inspect=()=>{}){const dir=rootFixture();try{
 if(changes.record)writeRecord(dir,changes.record);
 if(changes.before)changes.before(dir);
 const r=spawnSync('bash',['--noprofile','--norc'],{input:rootScript(dir,body,changes),encoding:'utf8',timeout:4000});
 assert(!r.error,label+': '+r.error);assert.equal(r.status===0,success,label+': '+r.stdout+r.stderr);inspect(dir,r);count++;
}finally{fs.rmSync(dir,{recursive:true})}}
// Exercise the actual fixed DNS file admission with selected delivery hashes,
// not a mock of check_dns_programs or merely the public predecessor catalogue.
const guardSha=createHash('sha256').update(fs.readFileSync(path.join(__dirname,'dns_broker_host/package/package-guard'))).digest('hex');
const brokerUnit=fs.readFileSync(path.join(__dirname,'dns_broker_host/omavless-dns-broker.service'),'utf8')
 .replace('ExecStart=/usr/lib/omavless/omavless-dns-broker --serve','ExecStart=/usr/lib/omavless-dns/omavless-dns-broker --serve')
 .replace('# REVIEW-ONLY opt-in candidate. Not installed or enabled by normal packages.\n','# Installation alone does not enroll, enable, or start this service.\n');
const unitSha=createHash('sha256').update(brokerUnit).digest('hex');
function dnsAdmission(label,brokerSha,expected,options={}){
 rootTest(label,{before:dir=>{
  for(const name of ['/usr/lib/omavless-dns/package-guard','/usr/lib/omavless-dns/omavless-dns-broker','/usr/lib/systemd/system/omavless-dns-broker.service']){
   const file=dir+name;fs.mkdirSync(path.dirname(file),{recursive:true});fs.writeFileSync(file,'inert; never execute');
  }
 }},[
  'pacman() { [[ "$*" == "-Q omavless-dns" ]] || return 99; printf "%s\\n" "omavless-dns '+(options.version||'0.9.8-1')+'"; }',
  'stat() { case "$2" in "%u %a") printf "0 755\\n";; "%u:%a:%h") if [[ "${@: -1}" == */omavless-dns-broker.service ]]; then printf "'+(options.unsafeUnit?'0:666:1':'0:644:1')+'\\n"; else printf "0:755:1\\n"; fi;; *) printf FORBIDDEN_STAT >&2; return 99;; esac; }',
  'sha256sum() { case "$1" in */package-guard) printf "%s  %s\\n" "'+(options.badGuard?'0'.repeat(64):guardSha)+'" "$1";; */omavless-dns-broker) printf "%s  %s\\n" "'+brokerSha+'" "$1";; */omavless-dns-broker.service) printf "%s  %s\\n" "'+(options.badUnit?'0'.repeat(64):unitSha)+'" "$1";; *) printf FORBIDDEN_HASH >&2; return 99;; esac; }',
  'check_dns_programs; printf DNS_ADMITTED'
 ].join('\n'),expected,(_,r)=>assert.equal(r.stdout.includes('DNS_ADMITTED'),expected));
}
for(const [arch,identity] of Object.entries(selected.identities)){
 dnsAdmission('selected DNS '+arch,identity.brokerSha,true,{version:selectedVersion});
 const changed=identity.brokerSha.slice(0,-1)+(identity.brokerSha.endsWith('0')?'1':'0');
 assert(!published.pairs.some(p=>p.brokerSha===changed));
 dnsAdmission('modified selected DNS '+arch,changed,false,{version:selectedVersion});
 for(const options of [{badGuard:true},{badUnit:true},{unsafeUnit:true}])
  dnsAdmission('selected DNS '+arch+' refuses '+Object.keys(options)[0],identity.brokerSha,false,{version:selectedVersion,...options});
 if(selectedVersion==='0.9.9-1'){
  dnsAdmission('099 DNS under public label '+arch,identity.brokerSha,false);
  dnsAdmission('099 DNS under unknown label '+arch,identity.brokerSha,false,{version:'0.9.10-1'});
  for(const predecessor of published.pairs)
   dnsAdmission('public DNS under099 label '+arch+' '+predecessor.sourceCommit,predecessor.brokerSha,false,{version:selectedVersion});
 }
}
for(const record of published.pairs)
 dnsAdmission('public DNS '+record.sourceCommit+' '+record.architecture,record.brokerSha,true);
rootTest('current root epoch admitted',{},'printf ADMITTED',true);
rootTest('old queued authorization token is refused',{attempt:token},'printf FORBIDDEN',false);
rootTest('wrong root phase is refused',{phase:'prepared'},'printf FORBIDDEN',false);
rootTest('wrong originating boot is refused',{originalBoot:newer},'printf FORBIDDEN',false);
rootTest('missing completion record revokes late authorization',{before:dir=>fs.unlinkSync(dir+'/home/user/.cache/omavless-maintenance/uninstall/progress.json')},'printf FORBIDDEN',false);
rootTest('pending old sudo is not inferred absent from free lock',{before:dir=>{fs.mkdirSync(dir+'/proc/123');fs.writeFileSync(dir+'/proc/123/cmdline','sudo\0bash\0-c\0# OmaVLESS removal attempt '+token+'\n');}},'printf FORBIDDEN',false);
const procFixture=dir=>{fs.mkdirSync(dir+'/proc/123');fs.writeFileSync(dir+'/proc/123/cmdline','ordinary\0command\0')};
const churnHead=body=>'head() { if [[ "${@: -1}" == */proc/123/cmdline ]]; then '+body+'; else /usr/bin/head "$@"; fi; }';
rootTest('vanished process after readability check is normal churn',{before:procFixture,prologue:churnHead('unlink "${@: -1}"; rmdir "${file%/cmdline}"; return 1')},'printf ADMITTED',true,(_,r)=>assert(r.stdout.includes('ADMITTED')));
rootTest('successful capture is inspected even after process exit',{before:procFixture,prologue:churnHead('printf "# OmaVLESS removal attempt '+token+'"; unlink "${@: -1}"; rmdir "${file%/cmdline}"')},'printf FORBIDDEN',false,(_,r)=>assert(r.stderr.includes('earlier removal authorization')));
rootTest('partial old authorization capture still blocks after read failure',{before:procFixture,prologue:churnHead('printf "# OmaVLESS removal attempt '+token+'"; unlink "${@: -1}"; rmdir "${file%/cmdline}"; return 1')},'printf FORBIDDEN',false,(_,r)=>assert(r.stderr.includes('earlier removal authorization')));
rootTest('live process read failure is not absence',{before:procFixture,prologue:churnHead('return 1')},'printf FORBIDDEN',false,(_,r)=>assert(r.stderr.includes('process list could not be checked')&&!r.stdout.includes('FORBIDDEN')));
rootTest('live process unreadable cmdline is not skipped',{before:dir=>{procFixture(dir);fs.chmodSync(dir+'/proc/123/cmdline',0o000)}},'printf FORBIDDEN',false,(_,r)=>assert(r.stderr.includes('process list could not be checked')));
rootTest('transformer failure is not forgiven after process exit',{before:procFixture,prologue:churnHead('unlink "${@: -1}"; rmdir "${file%/cmdline}"; return 1')+'\ntr() { /usr/bin/tr "$@"; return 7; }'},'printf FORBIDDEN',false,(_,r)=>assert(r.stderr.includes('process list could not be checked')));
rootTest('current attempt marker remains admitted',{before:dir=>{procFixture(dir);fs.writeFileSync(dir+'/proc/123/cmdline','# OmaVLESS removal attempt '+nextToken+'\0')}},'printf ADMITTED',true);
rootTest('exact cmdline limit preserves trailing NUL and newline',{before:dir=>{procFixture(dir);fs.writeFileSync(dir+'/proc/123/cmdline','x'.repeat(65534)+'\0\n')}},'printf ADMITTED',true);
rootTest('oversized cmdline is not a trusted truncated inventory',{before:dir=>{procFixture(dir);fs.writeFileSync(dir+'/proc/123/cmdline','x'.repeat(65537)+'# OmaVLESS removal attempt '+token)}},'printf FORBIDDEN',false,(_,r)=>assert(r.stderr.includes('process list could not be checked')));
for(const bad of ['[]','{}','[{"servers":[]}]','[{"resolvConfMode":false,"fallbackServers":[],"scopes":[]}]','[{"ifname":"Meta","ifindex":7}]','[{"ifname":"omavless0","ifindex":8}]']){
 // Missing link/global schema and named old links are distinct from a healthy view.
 rootTest('resolver schema/link admission '+bad,{},'ip() { printf \'[]\'; }; resolvectl() { printf \'%s\' '+JSON.stringify(bad)+'; }; network_empty',false);
}
for(const family of ['-4','-6'])rootTest('residual '+family+' rule is not deleted',{},[
 'ip() { case "$*" in *"'+family+' rule"*) printf \'[{"priority":9000,"table":2022}]\';; *) printf \'[]\';; esac; }',
 'resolvectl() { printf \'[{"resolvConfMode":"stub","fallbackServers":[],"scopes":[]},{"ifname":"eth0","ifindex":2}]\'; }','network_empty'
].join('\n'),false);

// Execute the real preparation/removal branch source with inert leaf effects.
// The independently reviewed leaf metadata/network checks are tested above.
const prepareMocks=[
 'pacman() { [[ "$*" == "-Q omavless-dns" ]]; }',
 'check_dns_programs() { printf CHECK_PROGRAMS; }',
 'check_packaged_file() { printf CHECK_CORE; }',
 'check_enrollment() { enrolled=true; }',
 'systemctl() { case "$*" in *"--property=FragmentPath"*) printf /usr/lib/systemd/system/omavless-dns-broker.service;; *"--property=NeedDaemonReload"*) printf no;; *"--property=UnitFileState"*) printf disabled;; "disable omavless-dns-broker.service") printf DISABLED;; *"--property=DropInPaths"*|*"--property=Job"*) :;; *) return 99;; esac; }'
].join('\n');
rootTest('configured preparation retains owner instead of killing session',{},prepareMocks+'\n'+section('ROOT_PREPARE'),true,(_,r)=>assert(r.stdout.includes('CHECK_CORE')&&r.stdout.includes('DISABLED')));
rootTest('foreign enrollment blocks before disabling anything',{},prepareMocks+'\ncheck_enrollment() { refuse; }\n'+section('ROOT_PREPARE'),false,(_,r)=>assert(!r.stdout.includes('DISABLED')));
rootTest('absent enrollment with retained descriptor refuses before disabling',{},prepareMocks+'\ncheck_enrollment() { enrolled=false; }\nsystemctl() { case "$*" in *"--property=MainPID"*) printf 0;; *"--property=NFileDescriptorStore"*) printf 1;; *) printf FORBIDDEN; return 99;; esac; }\n'+section('ROOT_PREPARE'),false,(_,r)=>assert(!r.stdout.includes('DISABLED')));
rootTest('modified bundled core blocks before enrollment or disable',{},prepareMocks+'\ncheck_packaged_file() { refuse; }\ncheck_enrollment() { printf FORBIDDEN; }\n'+section('ROOT_PREPARE'),false,(_,r)=>assert(!r.stdout.includes('FORBIDDEN')&&!r.stdout.includes('DISABLED')));
const removeMocks=[
 'empty_host() { printf ABSENT_HOST; }; network_empty() { printf ABSENT_NETWORK; }',
 'dns_absent() { return 1; }; check_dns_programs() { printf CHECK_PROGRAMS; }',
 'check_enrollment() { enrolled=true; }; remove_empty_enrollment_directory() { printf EMPTY_ENROLLMENT; }',
 'removed=false; pacman() { if [[ "$*" == "-R -- omavless omavless-dns" ]]; then printf REMOVED; removed=true; else [[ "$removed" != true ]]; fi; }'
].join('\n');
function removalLeafFixture(dir){
 const p=dir+'/usr/lib/omavless-dns';fs.mkdirSync(p,{recursive:true});
 fs.writeFileSync(p+'/package-guard','#!/bin/bash\nprintf CHECK_GUARD',{mode:0o755});
 fs.writeFileSync(p+'/omavless-dns-broker','#!/bin/bash\n[[ $1 == --revoke ]] || exit 8\nprintf REVOKED',{mode:0o755});
}
rootTest('guard precedes revoke then one ordinary paired package removal',{before:removalLeafFixture},removeMocks+'\n'+section('ROOT_REMOVE'),true,(_,r)=>{
 assert(r.stdout.indexOf('CHECK_GUARD')<r.stdout.indexOf('REVOKED'));
 assert(r.stdout.indexOf('REVOKED')<r.stdout.indexOf('REMOVED'));
 assert(r.stdout.endsWith('ABSENT_HOSTABSENT_NETWORK'));
});
rootTest('cancel package removal retains resumable boundary',{before:removalLeafFixture},removeMocks+'\npacman() { [[ "$1" != -R ]]; }\n'+section('ROOT_REMOVE'),false,(_,r)=>assert(r.stdout.includes('REVOKED')&&!r.stdout.includes('REMOVED')));
rootTest('already revoked enrollment is not revoked again',{before:removalLeafFixture},removeMocks+'\ncheck_enrollment() { enrolled=false; }\n'+section('ROOT_REMOVE'),true,(_,r)=>assert(!r.stdout.includes('REVOKED')&&r.stdout.includes('REMOVED')));
rootTest('failed original guard blocks both revocation and package removal',{before:dir=>{removalLeafFixture(dir);fs.writeFileSync(dir+'/usr/lib/omavless-dns/package-guard','#!/bin/bash\nexit 8',{mode:0o755})}},removeMocks+'\n'+section('ROOT_REMOVE'),false,(_,r)=>assert(!r.stdout.includes('REVOKED')&&!r.stdout.includes('REMOVED')));
for(const kind of ['stock','changed-core','missing-mtree','redirected-mtree','writable-mtree','foreign-owner'])rootTest('packaged core qualification '+kind,{before:dir=>{
 const core=dir+'/usr/lib/omavless-dns/mihomo',db=dir+'/var/lib/pacman/local/omavless-dns-0.9.8-1';
 fs.mkdirSync(path.dirname(core),{recursive:true});fs.mkdirSync(db,{recursive:true});
 const bytes=Buffer.from('stock fixture core');fs.writeFileSync(core,kind==='changed-core'?'changed':bytes,{mode:0o755});
 if(kind!=='missing-mtree')fs.writeFileSync(db+'/mtree',gzipSync('.'+core+' sha256digest='+createHash('sha256').update(bytes).digest('hex')+'\n'),{mode:0o644});
 if(kind==='redirected-mtree'){fs.renameSync(db+'/mtree',db+'/original');fs.symlinkSync('original',db+'/mtree')}
}},[
 'pacman() { if [[ "$1" == -Qqo ]]; then printf omavless-dns; else printf "omavless-dns 0.9.8-1"; fi; }',
 'stat() { case "$2" in "%u:%a:%h") printf "'+(kind==='foreign-owner'?'999':'0')+':755:1\\n";; "%u:%h") printf "0:1\\n";; "%u %a") if [[ "${@: -1}" == */mtree ]]; then printf "0 '+(kind==='writable-mtree'?'666':'644')+'\\n"; else printf "0 755\\n"; fi;; *) /usr/bin/stat "$@";; esac; }',
 'check_packaged_file omavless-dns /usr/lib/omavless-dns/mihomo'
].join('\n'),kind==='stock');
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
assert(source.indexOf('mask_cleanup_admitted=true')>source.indexOf('run_root "$root_remove"'));

async function rootMutex(){
 const dir=rootFixture();let owner;
 try{
  // Dedicated original process with a real FIFO, not sleep-as-effect evidence.
  const fifo=dir+'/continue';assert.equal(spawnSync('mkfifo',[fifo]).status,0);
  const filename=dir+'/root-worker.sh';fs.writeFileSync(filename,rootScript(dir,'printf "ADMITTED\\n"; read -r answer < '+JSON.stringify(fifo)+'; printf "ORIGINAL_COMPLETED\\n"'));
  owner=spawn('bash',[filename],{stdio:['ignore','pipe','pipe']});let live='';owner.stdout.on('data',c=>live+=c);let error='';owner.stderr.on('data',c=>error+=c);
  const deadline=Date.now()+15000;while(!live.includes('ADMITTED')){assert(owner.exitCode===null&&Date.now()<deadline,'original root fixture did not admit: '+error);await new Promise(r=>setTimeout(r,10));}
  writeRecord(dir,record({attempt:token,phase:'removing'}));
  const concurrent=spawnSync('bash',['--noprofile','--norc'],{input:rootScript(dir,'printf FORBIDDEN',{attempt:token}),encoding:'utf8',timeout:2000});
  assert.notEqual(concurrent.status,0);assert(!concurrent.stdout.includes('FORBIDDEN'));assert(concurrent.stderr.includes('still running'));
  const complete=new Promise(resolve=>owner.once('close',resolve));
  const writer=spawnSync('bash',['-c','printf "go\\n" > "$1"','fixture',fifo],{timeout:4000});assert.equal(writer.status,0);
  assert.equal(await complete,0,error);assert(live.includes('ORIGINAL_COMPLETED'));
  const fresh=spawnSync('bash',['--noprofile','--norc'],{input:rootScript(dir,'printf RECONCILED',{attempt:token}),encoding:'utf8',timeout:2000});
  assert.equal(fresh.status,0,fresh.stderr);assert.equal(fresh.stdout,'RECONCILED');count+=2;
 }finally{
  // Only this test's exact spawned inert worker can be terminated on failure.
  // Drain its pipes before removing its private FIFO/fixture directory.
  if(owner&&owner.exitCode===null){const closed=new Promise(resolve=>owner.once('close',resolve));owner.kill('SIGTERM');await closed;}
  fs.rmSync(dir,{recursive:true});
 }
}
rootMutex().then(()=>console.log('Uninstaller successor: '+count+' inert progress/mask/custody/network cases PASS; no host effects.')).catch(e=>{console.error(e);process.exitCode=1});

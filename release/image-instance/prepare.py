#!/usr/bin/env python3
"""Materialize an image-only staged release and call the existing Manafield CLI."""
import argparse, hashlib, json, os, pathlib, re, secrets, shutil, subprocess as sp, time
PACKAGE=pathlib.Path(__file__).resolve().parent
p=argparse.ArgumentParser()
p.add_argument('--instance-id',required=True)
p.add_argument('--instance-root',required=True,type=pathlib.Path)
p.add_argument('--reference-port',type=int,default=8080)
p.add_argument('--deploy',action='store_true')
a=p.parse_args(); os.umask(0o077)
assert re.fullmatch(r'[a-z0-9][a-z0-9_-]{0,39}',a.instance_id), 'use a lowercase Docker-safe Instance ID'
assert 1024 <= a.reference_port <= 65535, 'invalid Reference loopback port'
ROOT=a.instance_root.expanduser().resolve(); STAGE=ROOT/'staged'
lock=json.loads((PACKAGE/'images.lock.json').read_text())
assert lock['platform']=='linux/amd64'
for x in lock['images'].values():assert re.fullmatch(r'[^\s]+@sha256:[a-f0-9]{64}',x['image'])
env={k:v for k,v in os.environ.items() if not k.startswith('MANAFIELD_') and k not in ['COMPOSE_PROJECT_NAME','COMPOSE_FILE','COMPOSE_PROFILES','COMPOSE_ENV_FILES']}
def run(args,capture=False):
 return sp.check_output([str(x) for x in args],env=env) if capture else sp.run([str(x) for x in args],env=env,check=True)
def write(path,data,mode=0o600):
 with path.open('x') as f:f.write(data)
 path.chmod(mode)
def dump(path,data):write(path,json.dumps(data,indent=2)+'\n')
cli=PACKAGE/'manafield-linux-amd64'
if not cli.exists():
 run(['docker','pull','--platform','linux/amd64',lock['images']['cli']['image']])
 c=run(['docker','create','--platform','linux/amd64',lock['images']['cli']['image'],'--version'],True).decode().strip()
 try:run(['docker','cp',c+':/usr/local/bin/manafield',cli])
 finally:run(['docker','rm',c])
 cli.chmod(0o755)
assert hashlib.sha256(cli.read_bytes()).hexdigest()==lock['cli_binary_sha256'], 'CLI binary checksum mismatch'
project='manafield-'+a.instance_id
identity={'instance_id':a.instance_id,'release_version':lock['release_version'],'images':lock['images'],'reference_port':a.reference_port}
if STAGE.exists():
 assert json.loads((STAGE/'release-manifest.json').read_text())==identity, 'existing staged release differs; do not overwrite'
 for name in ['release.env','compose.yml','modules.compose.yml','compose-profiles.txt','postgresql-allocations.tsv','secrets/postgres-admin','modules/manafield-reference/manafield.module.json','build-plan.json']:
  assert (STAGE/name).is_file(), 'incomplete staged release: '+name
else:
 if ROOT.exists():assert not any(ROOT.iterdir()), 'Instance root must be empty on first preparation'
 # Never adopt an existing Compose project with a different/unknown Instance root.
 for args in [['ps','-aq'],['network','ls','-q'],['volume','ls','-q']]:
  assert not run(['docker',*args,'--filter','label=com.docker.compose.project='+project],True).strip(), 'Compose project already exists; select its original Instance root or another Instance ID'
 ROOT.mkdir(parents=True,exist_ok=True,mode=0o700);STAGE.mkdir(mode=0o700)
 for name in ['compose.yml','modules.compose.yml','images.lock.json']:shutil.copy2(PACKAGE/name,STAGE/name)
 write(STAGE/'compose-profiles.txt','')
 write(STAGE/'postgresql-allocations.tsv','')
 (STAGE/'secrets').mkdir(mode=0o700)
 write(STAGE/'secrets/postgres-admin',secrets.token_hex(24)+'\n')
 modules=STAGE/'modules';modules.mkdir(mode=0o755);modules.chmod(0o755)
 module=modules/'manafield-reference';module.mkdir(mode=0o755);module.chmod(0o755)
 shutil.copy2(PACKAGE/'reference.module.json',module/'manafield.module.json');(module/'manafield.module.json').chmod(0o644)
 definition=json.loads((PACKAGE/'instance.example.yaml').read_text());definition['instance']['id']=a.instance_id
 dump(ROOT/'instance.yaml',definition);shutil.copy2(ROOT/'instance.yaml',STAGE/'instance.release.yaml')
 owner=(STAGE/'postgresql-allocations.tsv').stat()
 values={'MANAFIELD_COMPOSE_PROJECT':project,'MANAFIELD_MODULES_NETWORK':'manafield-modules-'+a.instance_id,'MANAFIELD_REFERENCE_PORT':str(a.reference_port),'MANAFIELD_POSTGRES_PROVIDER_UID':str(owner.st_uid),'MANAFIELD_POSTGRES_PROVIDER_GID':str(owner.st_gid)}
 for key,name in [('core','MANAFIELD_CORE_IMAGE'),('reference','MANAFIELD_REFERENCE_IMAGE'),('postgresql','MANAFIELD_POSTGRES_IMAGE'),('provider','MANAFIELD_POSTGRES_PROVIDER_IMAGE')]:values[name]=lock['images'][key]['image']
 write(STAGE/'release.env',''.join(k+'='+v+'\n' for k,v in values.items()))
 run([cli,'plan',ROOT/'instance.yaml','--output',STAGE/'build-plan.json'])
 dump(STAGE/'release-manifest.json',identity)
compose=['docker','compose','--env-file',STAGE/'release.env','-f',STAGE/'compose.yml','-f',STAGE/'modules.compose.yml']
config=json.loads(run([*compose,'config','--format','json'],True))
expected={'core':'core','module-manafield-reference':'reference','manafield-postgres':'postgresql','manafield-postgres-provider':'provider'}
assert config['name']==project and set(config['services'])==set(expected)
for service,key in expected.items():
 assert config['services'][service]['image']==lock['images'][key]['image'] and 'build' not in config['services'][service], 'mutable image or build configuration rejected'
assert config['networks']['modules']['name']=='manafield-modules-'+a.instance_id
print('Prepared image-only staged release: '+str(STAGE),flush=True)
if a.deploy:
 run([*compose,'pull'])
 # Existing directory-deploy mode: up --no-build + ps, no source/Git/build steps.
 run([cli,'deploy',STAGE])
 deadline=time.monotonic()+150
 while True:
  ids=run([*compose,'ps','-aq'],True).decode().split()
  states=json.loads(run(['docker','inspect',*ids],True)) if ids else []
  if len(states)==4 and all(x['State']['Status']=='running' and x['State'].get('Health',{}).get('Status')=='healthy' for x in states):break
  if time.monotonic()>=deadline:raise RuntimeError('Instance readiness timeout; containers/data retained for operator inspection')
  time.sleep(2)
 state={'instance_id':a.instance_id,'status':'healthy','build_performed':False,'release_version':lock['release_version'],'images':lock['images']}
 (ROOT/'deployment-state.json').write_text(json.dumps(state,indent=2)+'\n')
 print('Instance healthy; only pinned image Pull and no-build deployment performed.',flush=True)

#!/usr/bin/env python3
"""Development-side artifact publisher; never connects to deployment hosts."""
import argparse, datetime, hashlib, json, pathlib, shutil, subprocess as sp, tarfile
ROOT=pathlib.Path(__file__).resolve().parents[2]; SOURCE=pathlib.Path(__file__).resolve().parent
def require(condition, message):
 if not condition: raise SystemExit(message)

p=argparse.ArgumentParser();p.add_argument('--output',required=True,type=pathlib.Path);p.add_argument('--validation',required=True,type=pathlib.Path);p.add_argument('--publish',action='store_true');a=p.parse_args()
OUT=a.output.resolve();OUT.mkdir(parents=True,exist_ok=True);PAYLOAD=OUT/'package';PAYLOAD.mkdir(exist_ok=True)
def run(args):return sp.check_output([str(x) for x in args],stderr=sp.DEVNULL)
def write(path,data):path.write_text(json.dumps(data,indent=2)+'\n')
require(not run(['git','-C',ROOT,'status','--porcelain']), 'commit source changes first')
sha=run(['git','-C',ROOT,'rev-parse','HEAD']).decode().strip();branch=run(['git','-C',ROOT,'branch','--show-current']).decode().strip()
remote=run(['git','-C',ROOT,'ls-remote','origin','refs/heads/'+branch]).decode().split();require(remote and remote[0]==sha, 'push the reviewed feature branch first')
original='60214d2c15061855ab4c9dac993642f2db8244f1'
require(run(['gh','api','repos/eventide-manafield/manafield/commits/'+original,'--jq','.sha']).decode().strip()==original, 'release validation failed')
lock=json.loads((SOURCE/'images.lock.json').read_text());validation=json.loads(a.validation.read_text());require(validation['passed'], 'release validation failed')
for row in lock['images'].values():
 actual=json.loads(run(['docker','buildx','imagetools','inspect',row['image'],'--format','{{json .Manifest.Digest}}']))
 require(row['image'].endswith('@'+actual), 'release validation failed')
for name in ['README.ko.md','images.lock.json','reference.module.json','instance.example.yaml','compose.yml','modules.compose.yml','compose-profiles.txt','release.env.example','prepare.py','deploy.sh']:
 shutil.copy2(SOURCE/name,PAYLOAD/name)
# Copy only the CLI executable from the immutable published image, no source build.
run(['docker','pull','--platform','linux/amd64',lock['images']['cli']['image']])
c=run(['docker','create','--platform','linux/amd64',lock['images']['cli']['image'],'--version']).decode().strip()
try:run(['docker','cp',c+':/usr/local/bin/manafield',PAYLOAD/'manafield-linux-amd64'])
finally:run(['docker','rm',c])
require(hashlib.sha256((PAYLOAD/'manafield-linux-amd64').read_bytes()).hexdigest()==lock['cli_binary_sha256'], 'release validation failed');(PAYLOAD/'manafield-linux-amd64').chmod(0o755)
write(PAYLOAD/'git-remote-state.json',{'repository':'https://github.com/eventide-manafield/manafield','branch':branch,'head_sha':sha,'remote_head_sha':remote[0],'original_cli_release_sha':original,'original_commit_available_on_github':True,'original_commit_on_main_claimed':False,'checked_at_utc':datetime.datetime.now(datetime.timezone.utc).isoformat()})
write(PAYLOAD/'compatibility.json',validation)
# Template archive never contains Instance roots, admin passwords or runtime state.
require(not any(x.name in ['secrets','release.env','postgres-admin','deployment-state.json'] for x in PAYLOAD.rglob('*')), 'release validation failed')
(PAYLOAD/'SHA256SUMS').write_text(''.join(hashlib.sha256(x.read_bytes()).hexdigest()+'  '+x.name+'\n' for x in sorted(PAYLOAD.iterdir()) if x.is_file() and x.name!='SHA256SUMS'))
archive=OUT/('manafield-'+lock['release_version']+'-linux-amd64.tar.gz')
with tarfile.open(archive,'w:gz') as t:
 for f in sorted(PAYLOAD.iterdir()):t.add(f,arcname=f.name)
image='ghcr.io/eventide-manafield/manafield-instance-release:'+lock['release_version']
delivery={'release_version':lock['release_version'],'source_sha':sha,'image':image,'registry_digest':None,'archive':archive.name,'archive_sha256':hashlib.sha256(archive.read_bytes()).hexdigest(),'runtime_images':lock['images']}
if a.publish:
 exists=sp.run(['docker','buildx','imagetools','inspect',image],stdout=sp.DEVNULL,stderr=sp.DEVNULL)
 require(exists.returncode!=0, 'release artifact tag exists; refuse overwrite')
 with (OUT/'package-build.log').open('wb') as f:sp.run(['docker','buildx','build','--load','--provenance=false','--platform','linux/amd64','-f',str(SOURCE/'package.Dockerfile'),'--label','org.opencontainers.image.source=https://github.com/eventide-manafield/manafield','--label','org.opencontainers.image.revision='+sha,'--label','org.opencontainers.image.version='+lock['release_version'],'-t',image,str(PAYLOAD)],stdout=f,stderr=sp.STDOUT,check=True)
 with (OUT/'package-push.log').open('wb') as f:sp.run(['docker','push','--platform','linux/amd64',image],stdout=f,stderr=sp.STDOUT,check=True)
 delivery['registry_digest']=json.loads(run(['docker','buildx','imagetools','inspect',image,'--format','{{json .Manifest.Digest}}']))
 delivery['immutable_image']=image.split(':')[0]+'@'+delivery['registry_digest']
write(OUT/'delivery.lock.json',delivery)
(OUT/'SHA256SUMS').write_text(delivery['archive_sha256']+'  '+archive.name+'\n')
print(json.dumps(delivery,indent=2))

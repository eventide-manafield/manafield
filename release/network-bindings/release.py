#!/usr/bin/env python3
"""Shell/Jenkins release packaging; never deploys services or changes visibility."""
import argparse, collections, hashlib, io, json, os, pathlib, re, shutil, subprocess as sp, tarfile
ROOT = pathlib.Path(__file__).resolve().parents[2]
RECIPE = ROOT / 'release/network-bindings'
VERSION = '2026.10.22-network-bindings.1'
SOURCE = 'https://github.com/eventide-manafield/manafield'
TESTED = '66257cf37b8dd250f275d296d73a0a7fb1b1055e'
os.umask(0o077)
p = argparse.ArgumentParser()
p.add_argument('action', choices=['build', 'audit', 'publish', 'export'])
p.add_argument('--output', required=True, type=pathlib.Path)
p.add_argument('--evidence', type=pathlib.Path)
p.add_argument('--trivy', default='trivy')
p.add_argument('--cache', type=pathlib.Path)
a = p.parse_args(); OUT = a.output.resolve(); OUT.mkdir(parents=True, exist_ok=True)
def run(args): return sp.check_output([str(x) for x in args], stderr=sp.DEVNULL)
def write(name, data): (OUT/name).write_text(json.dumps(data, indent=2, ensure_ascii=False)+'\n')
def read(name): return json.loads((OUT/name).read_text())
def digest(data): return 'sha256:'+hashlib.sha256(data).hexdigest()
sha = run(['git', '-C', ROOT, 'rev-parse', 'HEAD']).decode().strip()
images = {'cli': 'ghcr.io/eventide-manafield/manafield-cli:'+VERSION,
          'postgresql': 'ghcr.io/eventide-manafield/manafield-resource-postgresql:'+VERSION}
def check_identity(lock):
 assert lock['source_sha'] == sha and lock['release_version'] == VERSION
 for key, tag in images.items():
  x = json.loads(run(['docker','image','inspect',tag]))[0]
  assert x['Id'] == lock['images'][key]['local_id']
 return lock
if a.action == 'build':
 assert not run(['git','-C',ROOT,'status','--porcelain']), 'commit source changes before building'
 assert not run(['git','-C',ROOT,'diff',TESTED,sha,'--','src','Cargo.toml','Cargo.lock','rust-toolchain.toml','providers/postgresql/register.sh','deploy/compose.yml','Jenkinsfile']), 'reused integration evidence does not cover changed runtime/deployment source'
 assert a.evidence, '--evidence is required'
 evidence = json.loads(a.evidence.read_text()); assert evidence['passed'] and evidence['source_commit'] == TESTED
 write('integration-evidence.json', evidence)
 contexts = OUT/'contexts'; contexts.mkdir(exist_ok=True)
 core = contexts/'cli'; core.mkdir(exist_ok=True)
 for name in ['Cargo.toml','Cargo.lock','rust-toolchain.toml']: shutil.copy2(ROOT/name,core/name)
 shutil.copytree(ROOT/'src',core/'src',dirs_exist_ok=True)
 (core/'deploy').mkdir(exist_ok=True); shutil.copy2(ROOT/'deploy/instance.bootstrap.yaml',core/'deploy/instance.bootstrap.yaml')
 provider = contexts/'postgresql'; provider.mkdir(exist_ok=True)
 shutil.copy2(ROOT/'providers/postgresql/register.sh', provider/'register.sh')
 lock = {'release_version':VERSION,'crate_version':'0.0.1','source_repository':SOURCE,'source_sha':sha,'platform':'linux/amd64','integration_tested_sha':TESTED,'integration_reused':True,'images':{}}
 for key,tag in images.items():
  recipe = RECIPE/('cli.Dockerfile' if key=='cli' else 'provider.Dockerfile')
  args = ['docker','buildx','build','--load','--provenance=false','--platform','linux/amd64','-f',recipe,'-t',tag]
  for k,v in [('source',SOURCE),('revision',sha),('version',VERSION)]: args += ['--label','org.opencontainers.image.'+k+'='+v]
  with (OUT/(key+'-build.log')).open('wb') as f:
   sp.run([str(x) for x in args+[core if key=='cli' else provider]],stdout=f,stderr=sp.STDOUT,check=True)
  x = json.loads(run(['docker','image','inspect',tag]))[0]; assert x['Os']=='linux' and x['Architecture']=='amd64'
  lock['images'][key]={'image':tag,'local_id':x['Id'],'registry_digest':None,'size_bytes':x['Size'],'published':False}
 container = run(['docker','create',images['cli'],'--version']).decode().strip()
 try: run(['docker','cp',container+':/usr/local/bin/manafield',OUT/'manafield-linux-amd64'])
 finally: run(['docker','rm',container])
 (OUT/'manafield-linux-amd64').chmod(0o755)
 version = run(['docker','run','--rm','--network','none',images['cli'],'--version']).decode().strip(); assert '0.0.1' in version
 write('compatibility.json',{'cli_version_output':version,'runtime_and_deployment_sources_unchanged_from_integration':True,'provider_script_sha256':digest((provider/'register.sh').read_bytes()),'cli_binary_sha256':digest((OUT/'manafield-linux-amd64').read_bytes()),'cli_container_scope':'CLI commands; Docker/Git executables are host prerequisites for deployment/build; no Docker socket embedded'})
 write('images.lock.json',lock)
 print('Built CLI and PostgreSQL Provider; existing integration evidence reused.')
elif a.action == 'audit':
 lock = check_identity(read('images.lock.json')); summary={}
 for key,tag in images.items():
  archive=OUT/(key+'-image.tar'); run(['docker','image','save','-o',archive,tag])
  args=[a.trivy,'image','--input',archive,'--scanners','vuln,secret','--image-config-scanners','secret','--format','json','--quiet']
  if a.cache:args+=['--cache-dir',a.cache,'--skip-db-update']
  v=json.loads(run(args)); clean=[]; counts=collections.Counter(); secrets=0
  for row in v.get('Results',[]):
   vulns=[{k:x.get(k) for k in ['VulnerabilityID','PkgName','InstalledVersion','FixedVersion','Severity']} for x in row.get('Vulnerabilities',[])]
   counts.update(x['Severity'] for x in vulns)
   found=[{'rule':x.get('RuleID'),'path':row.get('Target'),'value':'[REDACTED]'} for x in row.get('Secrets',[])];secrets+=len(found)
   clean.append({'target':row.get('Target'),'vulnerabilities':vulns,'secrets':found})
  write(key+'-trivy.json',clean)
  findings=[]; files=0
  def scan(data,path):
   for kind,pattern in [('private-key',rb'-----BEGIN (?:[A-Z0-9]+ )*PRIVATE KEY-----'),('development-address',rb'manafield\.studio|/home/nayuz|/opt/manafield/ai-workspace'),('credential-url',rb'(?i)(?:postgres(?:ql)?|https?)://[^\s/:]+:[^\s/@]+@')]:
    if re.search(pattern,data):findings.append({'path':path,'type':kind,'value':'[REDACTED]'})
  with tarfile.open(archive) as t:
   manifest=json.load(t.extractfile('manifest.json'))[0];scan(t.extractfile(manifest['Config']).read(),'config-env-labels-history')
   for n,lp in enumerate(manifest['Layers']):
    with tarfile.open(fileobj=t.extractfile(lp)) as layer:
     for f in layer:
      if not f.isfile():continue
      files+=1; scan(layer.extractfile(f).read(),f'layer-{n}/{f.name}')
      if pathlib.PurePosixPath(f.name).name in ['.env','.pgpass','credentials','id_rsa','id_ed25519']:findings.append({'path':f'layer-{n}/{f.name}','type':'sensitive-filename'})
  write(key+'-all-layers.json',{'files_scanned':files,'findings':findings})
  summary[key]={'vulnerabilities':dict(counts),'secrets':secrets,'all_layer_findings':len(findings)}
 write('security.json',summary)
 assert all(not x['vulnerabilities'].get('CRITICAL') and not x['vulnerabilities'].get('HIGH') and not x['secrets'] and not x['all_layer_findings'] for x in summary.values()), 'security gate failed; inspect redacted reports'
 write('audit-pass.json',{'source_sha':sha,'local_ids':{k:v['local_id'] for k,v in lock['images'].items()}})
 print(json.dumps(summary))
elif a.action == 'publish':
 lock=check_identity(read('images.lock.json')); audit=read('audit-pass.json');assert audit['source_sha']==sha
 for key,tag in images.items():
  assert audit['local_ids'][key]==lock['images'][key]['local_id']
  # Refuse an existing tag unless it already identifies this exact local manifest.
  pre=sp.run(['docker','buildx','imagetools','inspect',tag,'--raw'],capture_output=True)
  if pre.returncode==0:
   assert digest(pre.stdout.rstrip(b'\n'))==lock['images'][key]['local_id'], 'release tag already exists; refusing overwrite'
  with (OUT/(key+'-push.log')).open('wb') as f:sp.run(['docker','push','--platform','linux/amd64',tag],stdout=f,stderr=sp.STDOUT,check=True)
  raw=run(['docker','buildx','imagetools','inspect',tag,'--raw']); manifest=json.loads(raw)
  # The single-platform manifest is compared to the saved content, not newline formatting.
  with tarfile.open(OUT/(key+'-image.tar')) as t:
   matches=[]
   for entry in t.getmembers():
    if entry.isfile() and entry.name.startswith('blobs/sha256/'):
     b=t.extractfile(entry).read()
     if b.lstrip().startswith(b'{'):
      try:
       if json.loads(b)==manifest:matches.append(digest(b))
      except ValueError:pass
   assert len(matches)==1,'registry/local manifest mismatch'
  lock['images'][key].update(registry_digest=matches[0],immutable_image=tag.split(':')[0]+'@'+matches[0],published=True)
  write('images.lock.json',lock)
 print('Published both release images and verified registry digests; visibility unchanged.')
else:
 lock=check_identity(read('images.lock.json')); assert all(x['published'] for x in lock['images'].values())
 shutil.copy2(RECIPE/'README.ko.md',OUT/'README.ko.md')
 run(['git','-C',ROOT,'archive','--format=tar.gz','-o',OUT/'source.tar.gz',sha])
 with (OUT/'changes.patch').open('wb') as f:sp.run(['git','-C',ROOT,'format-patch','--stdout','34672cc870482d917ffe6444b6d06b645c14a422..'+sha],stdout=f,check=True)
 package=OUT/('manafield-'+VERSION+'-linux-amd64.tar.gz')
 with tarfile.open(package,'w:gz') as t:
  for name in ['manafield-linux-amd64','images.lock.json','compatibility.json','integration-evidence.json','security.json','cli-trivy.json','postgresql-trivy.json','cli-all-layers.json','postgresql-all-layers.json','README.ko.md','source.tar.gz','changes.patch']:
   t.add(OUT/name,arcname=name)
 (OUT/'SHA256SUMS').write_text(''.join(hashlib.sha256((OUT/name).read_bytes()).hexdigest()+'  '+name+'\n' for name in ['manafield-linux-amd64','source.tar.gz',package.name]))
 print(package)

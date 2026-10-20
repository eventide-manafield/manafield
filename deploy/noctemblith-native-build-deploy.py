#!/usr/bin/env python3
"""Build the published UI revision, then replace only the native app image."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import time
import uuid

sys.dont_write_bytecode = True
REPO = Path('/opt/manafield/ai-workspace/noctemblith-native-redis-20261016')
REVISION = 'd586a1d8ce26b31b0f97227f3f365008ef510510'
TREE = 'efccc9e4ce16f5382cd287199f03ef4db1dd0980'
TAG = 'manafield-module-noctemblith:' + REVISION[:12]
UPDATER = Path(__file__).with_name('noctemblith-native-update.py')
UPDATER_SHA256 = '83a288d5e5b6e405b0be3022a74b129c9525929a77e1895b8f3d2729bd76b3f7'
RECORDS = Path('/opt/manafield/ai-workspace/instance/builds')
ENV = {'PATH': '/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin', 'LANG': 'C.UTF-8'}


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def capture(args):
    return subprocess.check_output([str(x) for x in args], env=ENV, text=True,
                                   stderr=subprocess.PIPE, timeout=180).strip()


def load_updater():
    require(hashlib.sha256(UPDATER.read_bytes()).hexdigest() == UPDATER_SHA256,
            'The reviewed update helper changed; inspect before deploying')
    spec = importlib.util.spec_from_file_location('native_image_update', UPDATER)
    updater = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(updater)
    return updater


def verified_image_id(iid, metadata, target):
    require(re.fullmatch(r'sha256:[0-9a-f]{64}', iid) is not None,
            'Build did not write a valid image ID')
    digests = {iid}
    for key in ('containerimage.digest', 'containerimage.config.digest'):
        value = metadata.get(key)
        if isinstance(value, str) and re.fullmatch(r'sha256:[0-9a-f]{64}', value):
            digests.add(value)
    image_id = target.get('Id')
    require(image_id in digests, 'Image tag no longer matches this build result')
    return image_id


def main():
    require(len(sys.argv) == 1, 'Run without arguments; this helper builds and deploys the pinned UI commit')
    require(os.geteuid() == 0, 'Run with sudo: Docker and release.env access are required')
    os.umask(0o077)
    updater = load_updater()
    git = ['git', '-c', 'safe.directory=' + str(REPO), '-C', REPO]
    require(capture(git + ['rev-parse', REVISION + '^{commit}']) == REVISION,
            'The published commit is unavailable')
    require(capture(git + ['rev-parse', REVISION + '^{tree}']) == TREE,
            'The source tree differs from the published UI changes')
    capture(['docker', 'info', '--format', '{{.ServerVersion}}'])
    RECORDS.mkdir(parents=True, exist_ok=True)
    record = RECORDS / ('noctemblith-' + REVISION[:12] + '-' +
                        time.strftime('%Y%m%dT%H%M%SZ', time.gmtime()) + '-' + uuid.uuid4().hex[:6])
    record.mkdir(mode=0o700)
    print('PASS: exact published commit and source tree', flush=True)
    print('Build records:', record, flush=True)
    with tempfile.TemporaryDirectory(prefix='noctemblith-build-') as temporary:
        archive = Path(temporary) / 'source.tar'
        capture(git + ['archive', '--format=tar', '--output=' + str(archive), REVISION])
        print('Building ' + TAG + ' ...', flush=True)
        with archive.open('rb') as source:
            subprocess.run([
                'docker', 'buildx', 'build', '--load', '--progress=plain',
                '--build-arg', 'NOCTEMBLITH_BUILD_SHA=' + REVISION,
                '--build-arg', 'NOCTEMBLITH_UPDATE_CHANNEL=noctemblith/dev',
                '--metadata-file', str(record / 'build-metadata.json'),
                '--iidfile', str(record / 'image-id'), '-t', TAG, '-',
            ], stdin=source, env=ENV, check=True)
    iid = (record / 'image-id').read_text().strip()
    metadata = json.loads((record / 'build-metadata.json').read_text())
    target = json.loads(capture(['docker', 'image', 'inspect', TAG]))[0]
    image_id = verified_image_id(iid, metadata, target)
    result = {'revision': REVISION, 'tree': TREE, 'tag': TAG, 'image_id': image_id,
              'status': 'built'}
    result_file = record / 'result.json'
    result_file.write_text(json.dumps(result, indent=2) + '\n')
    print('PASS: image built and pinned: ' + image_id, flush=True)
    # Bind the proven updater to the immutable image from this build, never a mutable tag.
    updater.REVISION = REVISION
    updater.IMAGE = image_id
    updater.EXPECTED_IMAGE_DIGESTS = {image_id: 'fresh build'}
    try:
        status = updater.main()
        require(status == 0, 'The image updater returned a failure status')
    except BaseException:
        result['status'] = 'deployment_failed'
        result_file.write_text(json.dumps(result, indent=2) + '\n')
        raise
    result['status'] = 'deployed'
    result_file.write_text(json.dumps(result, indent=2) + '\n')
    print('PASS: published UI commit built and deployed', flush=True)
    print('Build/result:', result_file, flush=True)
    return 0


if __name__ == '__main__':
    try:
        sys.exit(main())
    except RuntimeError as error:
        print('Stopped:', str(error), file=sys.stderr)
        sys.exit(1)
    except (OSError, ValueError, KeyError, subprocess.SubprocessError):
        print('Stopped: build or deployment failed; inspect the build output and recovery/result.', file=sys.stderr)
        sys.exit(1)

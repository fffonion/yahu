#!/usr/bin/env python3
"""Build the bundled-GeckoView Android app entirely inside Docker."""
import argparse
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


def run(*args):
    print('+', ' '.join(str(arg) for arg in args), flush=True)
    subprocess.run([str(arg) for arg in args], check=True)


def main():
    if not Path('/.dockerenv').exists():
        raise SystemExit('Run this build inside android/Dockerfile, not on the host.')
    parser = argparse.ArgumentParser()
    parser.add_argument('--test-only', action='store_true')
    parser.add_argument('--output', default='/out')
    parser.add_argument('--signing', default='/signing')
    args = parser.parse_args()
    root = Path(__file__).resolve().parent
    out = Path(args.output).resolve()
    out.mkdir(parents=True, exist_ok=True)
    tools = Path(os.environ['ANDROID_BUILD_TOOLS'])
    run('python3', '-m', 'unittest', 'discover', '-s', root / 'tests', '-p', 'test_*.py')
    package = Path('io/github/fffonion/yahu')
    test_classes = out / 'test-classes'
    test_classes.mkdir(exist_ok=True)
    run('javac', '-source', '8', '-target', '8', '-Xlint:-options', '-d', test_classes,
        root / 'app/src' / package / 'NavigationPolicy.java', root / 'tests' / package / 'NavigationPolicyTest.java')
    run('java', '-cp', test_classes, 'io.github.fffonion.yahu.NavigationPolicyTest')
    if args.test_only:
        return

    checkout = out / 'checkout'
    staged = checkout / 'android'
    if staged.exists():
        shutil.rmtree(staged)
    shutil.copytree(root, staged, ignore=shutil.ignore_patterns('.gradle', 'build', '__pycache__'))
    icon = checkout / 'frontend/public/icon-192.png'
    icon.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(root.parent / 'frontend/public/icon-192.png', icon)
    run('gradle', '--no-daemon', '--console=plain', '--project-cache-dir', out / 'gradle-project-cache',
        '-p', staged, '-PyahuBuildDir=' + str(out / 'gradle-build'), ':app:assembleRelease')
    unsigned = out / 'gradle-build/app/outputs/apk/release/app-release-unsigned.apk'
    if not unsigned.is_file():
        raise SystemExit('Gradle did not produce the expected APK.')
    run(tools / 'zipalign', '-P', '16', '-f', '4', unsigned, out / 'aligned.apk')
    signing = Path(args.signing).resolve()
    signing.mkdir(mode=0o700, parents=True, exist_ok=True)
    key, cert = signing / 'yahu-key.pk8', signing / 'yahu-cert.pem'
    if key.exists() != cert.exists():
        raise SystemExit('Incomplete signing identity; refusing to replace existing signing material.')
    if not key.exists():
        with tempfile.TemporaryDirectory(dir=signing) as temp:
            temp = Path(temp)
            result = subprocess.run(['openssl', 'req', '-x509', '-newkey', 'rsa:2048', '-nodes',
                '-keyout', str(temp / 'key.pem'), '-out', str(temp / 'cert.pem'),
                '-days', '3650', '-subj', '/CN=Yahu Android/'], stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
            if result.returncode:
                raise SystemExit('Signing identity generation failed.')
            run('openssl', 'pkcs8', '-topk8', '-inform', 'PEM', '-outform', 'DER',
                '-in', temp / 'key.pem', '-out', temp / 'key.pk8', '-nocrypt')
            os.chmod(temp / 'key.pk8', 0o600)
            os.replace(temp / 'key.pk8', key)
            os.replace(temp / 'cert.pem', cert)
    run(tools / 'apksigner', 'sign', '--key', key, '--cert', cert,
        '--out', out / 'yahu.apk', out / 'aligned.apk')
    run(tools / 'apksigner', 'verify', '--verbose', out / 'yahu.apk')
    run(tools / 'zipalign', '-P', '16', '-c', '4', out / 'yahu.apk')
    badging = subprocess.check_output([str(tools / 'aapt'), 'dump', 'badging', str(out / 'yahu.apk')], text=True)
    print('\n'.join(line for line in badging.splitlines() if line.startswith(('package:', 'sdkVersion:', 'targetSdkVersion:', 'application-label:', 'launchable-activity:', 'native-code:'))))
    print('APK:', out / 'yahu.apk')


if __name__ == '__main__':
    main()

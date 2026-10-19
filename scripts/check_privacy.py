#!/usr/bin/env python3
"""Check tracked public text; diagnostics never include matched private values."""
import ipaddress
from pathlib import Path
import re
import subprocess
import sys

IPV4 = re.compile(r'(?<![\w.])(?:\d{1,3}\.){3}\d{1,3}(?![\d.])')
HOST_PATH = re.compile(r'/(?:home|Users|mnt)/[A-Za-z0-9_.-]+|\b[A-Za-z]:\\{1,4}Users\\{1,4}[A-Za-z0-9_.-]+')
SENDER_ID = re.compile(r'\[[^\]\n|]+\|-?\d{8,}\]')
PLATFORM_ID = re.compile(r'''["']?(?:chat_id|source_chat_id|user_id|platformSenderId)["']?\s*:\s*["']?-?\d{8,}''')


def privacy_issues(text):
    issues = []
    for number, line in enumerate(text.splitlines(), 1):
        kinds = set()
        if HOST_PATH.search(line):
            kinds.add('host-specific-path')
        for match in IPV4.finditer(line):
            try:
                address = ipaddress.IPv4Address(match.group())
            except ipaddress.AddressValueError:
                continue
            first, second, *_ = address.packed
            if (first == 10 or (first == 172 and 16 <= second <= 31)
                    or (first, second) == (192, 168) or address.is_link_local):
                kinds.add('private-network-ip')
        unescaped = line.replace('\\"', '"').replace("\\'", "'")
        if SENDER_ID.search(unescaped) or PLATFORM_ID.search(unescaped):
            kinds.add('platform-account-id')
        issues.extend((number, kind) for kind in sorted(kinds))
    return issues


def scan_repository(root):
    root = Path(root)
    names = subprocess.check_output(['git', 'ls-files', '-z'], cwd=root).decode().split('\0')
    findings = []
    for name in filter(None, names):
        path = root / name
        if path.is_symlink():
            import os
            text = os.readlink(path)
        else:
            raw = path.read_bytes()
            if b'\0' in raw:
                continue
            try:
                text = raw.decode('utf-8')
            except UnicodeDecodeError:
                continue
        findings.extend((name, line, kind) for line, kind in privacy_issues(text))
    return findings


def main():
    findings = scan_repository(Path(__file__).resolve().parents[1])
    for name, line, kind in findings:
        print(f'{name}:{line}: {kind}')
    if findings:
        print(f'Privacy check failed: {len(findings)} findings; values omitted.')
        return 1
    print('Privacy check passed: no host-specific paths, private IPs or platform account IDs in tracked text.')
    return 0


if __name__ == '__main__':
    sys.exit(main())

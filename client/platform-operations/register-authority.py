#!/usr/bin/env python3
"""Register one root-governed module platform operation in Boss AuthoritySupply.

This deployment utility does not restart Boss or invoke the privileged provider.
Production policy: never replace existing authorities or broaden existing grants.
"""
from __future__ import annotations
import argparse
import datetime
import json
import os
import pathlib
import pwd
import socket
import stat
import subprocess
import sys
import tempfile
import zipfile

SUPPLY = pathlib.Path('/usr/lib/neebles/platform/authority/authority-supply.json')
DESCRIPTOR = pathlib.Path('/usr/lib/neebles/platform/authority/neebles-proxy-client-platform.json')
POLICY = pathlib.Path('/etc/neebles/platform-operations/neebles-proxy-client.json')
MODULE = 'neebles-proxy-client'
AUTH = 'neebles.platform.operation'
OPS = ('activate', 'deactivate')
VIVO_HOST = 'minithomyorke-vivobookasuslaptopx580gdn580gd'

def trusted_root_path(p: pathlib.Path, *, file=True) -> os.stat_result:
    if not p.is_absolute() or '..' in p.parts:
        raise ValueError('path must be absolute and normalized')
    current=pathlib.Path('/')
    for i,part in enumerate(p.parts[1:]):
        current=current/part
        st=os.lstat(current)
        islast=i==len(p.parts)-2
        if st.st_uid!=0 or st.st_mode & 0o022 or stat.S_ISLNK(st.st_mode):
            raise ValueError(f'root path not trusted: {current} uid={st.st_uid} mode={oct(stat.S_IMODE(st.st_mode))}')
        if not islast and not stat.S_ISDIR(st.st_mode):
            raise ValueError(f'not a directory: {current}')
    st=os.lstat(p)
    if file and not stat.S_ISREG(st.st_mode):raise ValueError('root path not a regular file')
    if not file and not stat.S_ISDIR(st.st_mode):raise ValueError('root path not a directory')
    return st

def json_without_duplicates(path: pathlib.Path):
    trusted_root_path(path)
    def unique(pairs):
        result={}
        for k,v in pairs:
            if k in result:raise ValueError(f'duplicate JSON field: {k}')
            result[k]=v
        return result
    raw=path.read_bytes()
    if len(raw)>128*1024:raise ValueError(f'oversized policy/authority: {path}')
    return json.loads(raw,object_pairs_hook=unique)

def validate_policy(policy):
    if not isinstance(policy,dict) or policy.get('schema')!='1' or policy.get('module')!=MODULE:
        raise ValueError('unexpected installed root module policy')
    operations=policy.get('operations')
    if not isinstance(operations,dict) or set(operations)!=set(OPS):
        raise ValueError('root policy must declare exactly activate/deactivate')
    for op in OPS:
        spec=operations[op]
        if not isinstance(spec,dict) or set(spec)!= {'executable','argv'}:
            raise ValueError('malformed privileged OS policy')
        if spec['executable']!='/usr/lib/neebles/module-operations/neebles-proxy-client-platform':
            raise ValueError('unexpected helper path for policy')
    return True

def descriptor_payload():
    return {'schema':'1','authority':AUTH,'module':MODULE,'operations':list(OPS)}

def merge_supply(original):
    if not isinstance(original,dict) or set(original)!={'schema','name','entries'}:
        raise ValueError('supply unknown schema fields')
    if original['schema']!='1' or original['name']!='neebles-authority-supply':
        raise ValueError('supply identity/schema mismatch')
    entries=original['entries']
    if not isinstance(entries,list) or not entries or len(entries)>128:
        raise ValueError('invalid authority supply entries')
    existing=[]
    for item in entries:
        if not isinstance(item,dict) or set(item)!={'authority','location'}:
            raise ValueError('unexpected authority entry shape')
        if not isinstance(item['authority'],str) or not isinstance(item['location'],str):
            raise ValueError('invalid authority reference types')
        existing.append(item['authority'])
    if len(existing)!=len(set(existing)):
        raise ValueError('duplicate original authority identities')
    if AUTH in existing:
        for entry in entries:
            if entry['authority']==AUTH and entry['location']==str(DESCRIPTOR):
                return original,False
        raise ValueError('another platform operation authority is already supplied; no overwrite')
    new={'schema':original['schema'],'name':original['name'],'entries':[
        *entries,{'authority':AUTH,'location':str(DESCRIPTOR)}]}
    assert new['entries'][:-1] == entries
    return new,True

def atomic_root_json(path: pathlib.Path, payload: dict):
    trusted_root_path(path.parent,file=False)
    if path.exists() or path.is_symlink():
        raise ValueError(f'file exists; do not overwrite: {path}')
    content=(json.dumps(payload,indent=2,ensure_ascii=False)+'\n').encode()
    fd,tmp=tempfile.mkstemp(prefix='.neebles-authority-',dir=path.parent)
    try:
        os.fchmod(fd,0o644)
        os.fchown(fd,0,0)
        with os.fdopen(fd,'wb',closefd=True) as out:
            out.write(content);out.flush();os.fsync(out.fileno())
        os.replace(tmp,path)
        dirfd=os.open(str(path.parent),os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW)
        try:os.fsync(dirfd)
        finally:os.close(dirfd)
        trusted_root_path(path)
    finally:
        if os.path.exists(tmp):os.unlink(tmp)

def replace_root_supply(path: pathlib.Path, data: dict, original_stat: os.stat_result):
    # Disallow concurrent/foreign replacements before the atomic commit.
    current=trusted_root_path(path)
    if (current.st_dev,current.st_ino,current.st_size,current.st_mtime_ns) != (
        original_stat.st_dev,original_stat.st_ino,original_stat.st_size,original_stat.st_mtime_ns):
        raise ValueError('authority supply changed during registration')
    content=(json.dumps(data,indent=2,ensure_ascii=False)+'\n').encode()
    fd,tmp=tempfile.mkstemp(prefix='.neebles-authority-',dir=path.parent)
    try:
        os.fchmod(fd,0o644);os.fchown(fd,0,0)
        with os.fdopen(fd,'wb',closefd=True) as out:
            out.write(content);out.flush();os.fsync(out.fileno())
        os.replace(tmp,path)
        dirfd=os.open(str(path.parent),os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW)
        try:os.fsync(dirfd)
        finally:os.close(dirfd)
    finally:
        if os.path.exists(tmp):os.unlink(tmp)

def main():
    ap=argparse.ArgumentParser(description=__doc__)
    g=ap.add_mutually_exclusive_group(required=True)
    g.add_argument('--audit',action='store_true')
    g.add_argument('--install',action='store_true')
    args=ap.parse_args()
    user=os.environ.get('SUDO_USER') or pwd.getpwuid(os.getuid()).pw_name
    try:
        target=pwd.getpwnam(user).pw_dir
    except KeyError:
        target=str(pathlib.Path.home())
    downloads=pathlib.Path(target)/'Downloads'
    if not downloads.exists() or not downloads.is_dir():
        downloads=pathlib.Path('/tmp')
    now=datetime.datetime.now().strftime('%Y%m%d_%H%M%S')
    base=downloads/f'NEEBLES_BOSS_F19_AUTHORITY_{now}'
    report=['F19 Boss / OS platform authority',f'HOST={socket.gethostname()}',f'MODE={"INSTALL" if args.install else "AUDIT"}','NO TAILSCALE SET; NO SERVICE RESTART; NO MODULE EXECUTION']
    ok=False
    try:
        if os.geteuid()!=0:raise ValueError('root required')
        if socket.gethostname()!=VIVO_HOST:raise ValueError('VivoProBook hostname required; do not run on Ryzen')
        original_stat=trusted_root_path(SUPPLY)
        current=json_without_duplicates(SUPPLY)
        proposed,modified=merge_supply(current)
        report.append(f'PASS supply existing identities={len(current["entries"])}; preserve all; add={modified}')
        root_policy=json_without_duplicates(POLICY)
        validate_policy(root_policy)
        report.append('PASS installed root policy contains exact two operations and helper path')
        helper=pathlib.Path('/usr/lib/neebles/module-operations/neebles-proxy-client-platform')
        s=trusted_root_path(helper)
        if not s.st_mode & 0o111:raise ValueError('helper not executable')
        report.append('PASS root-controlled helper')
        unit=subprocess.run(['systemctl','is-active','neebles-runtime.service'],capture_output=True,text=True,timeout=8)
        if unit.returncode!=0 or unit.stdout.strip()!='active':raise ValueError('Boss runtime not active')
        svc=subprocess.run(['systemctl','show','neebles-runtime.service','-p','ExecStart','--value'],capture_output=True,text=True,timeout=8)
        if svc.returncode or str(SUPPLY) not in svc.stdout:raise ValueError('Boss runtime not using expected supply')
        report.append('PASS Boss service loads this authority supply')
        if DESCRIPTOR.exists() or DESCRIPTOR.is_symlink():
            if json_without_duplicates(DESCRIPTOR)!=descriptor_payload():
                raise ValueError('existing OS descriptor differs; never overwrite')
            report.append('PASS existing exact platform authority descriptor')
        elif not modified:
            raise ValueError('supply points to a missing descriptor; cannot certify')
        if args.install:
            if not DESCRIPTOR.exists() and not DESCRIPTOR.is_symlink():
                atomic_root_json(DESCRIPTOR,descriptor_payload())
                report.append('INSTALLED root-controlled platform authority descriptor')
            if modified:
                replace_root_supply(SUPPLY,proposed,original_stat)
                report.append('REGISTERED platform authority in Boss supply (other entries preserved)')
            now_current=json_without_duplicates(SUPPLY)
            after,need=merge_supply(now_current)
            if need or after!=now_current or json_without_duplicates(DESCRIPTOR)!=descriptor_payload():
                raise ValueError('postinstall verification mismatch')
            report.append('PASS OS authority persisted; requires explicit Boss runtime restart to take effect')
            report.append('RESULT=F19_AUTHORITY_REGISTERED_RESTART_NOT_PERFORMED')
        else:
            report.append('RESULT=F19_AUDIT_PASS_NO_MUTATION')
        ok=True
    except Exception as e:
        report.append('BLOCK='+str(e))
        report.append('RESULT=BLOCKED; do not trigger Activate/Deactivate')
    finally:
        txt=base.with_suffix('.txt');zp=base.with_suffix('.zip')
        txt.write_text('\n'.join(report)+'\n')
        try:
            uid=pwd.getpwnam(user).pw_uid;gid=pwd.getpwnam(user).pw_gid
            if os.geteuid()==0:os.chown(txt,uid,gid)
        except Exception:pass
        with zipfile.ZipFile(zp,'w',zipfile.ZIP_DEFLATED) as z:z.write(txt,txt.name)
        try:
            if os.geteuid()==0:os.chown(zp,uid,gid)
        except Exception:pass
        print('\n'.join(report));print('REPORT='+str(zp))
    if not ok:sys.exit(2)
if __name__=='__main__':main()

#!/usr/bin/env python3
"""NEEBLES OS platform provider candidate, systemd socket activated.
No privileged mutation is enabled in this candidate.
"""
import json
import os
import socket
import struct
import subprocess
import sys

SOCKET_FD = 3
ALLOWED = {'tailscale.exit_node.status', 'tailscale.exit_node.activate', 'tailscale.exit_node.deactivate'}

def respond(request, peer_uid, authorized_uid, runner=subprocess.run):
    if peer_uid != authorized_uid:
        return {'schema':'1','ok':False,'error':'untrusted caller'}
    if not isinstance(request, dict) or set(request) != {'schema','module','operation'} or request.get('schema') != '1' or request.get('module') != 'neebles-proxy-server' or request.get('operation') not in ALLOWED:
        return {'schema':'1','ok':False,'error':'invalid request'}
    if request['operation'] != 'tailscale.exit_node.status':
        return {'schema':'1','ok':False,'error':'mutating operations disabled until OS policy and Secrets FD are integrated'}
    result = runner(['/usr/bin/tailscale','status','--json'], capture_output=True, text=True, timeout=6, check=False)
    if result.returncode or len(result.stdout)>65536:
        return {'schema':'1','ok':False,'error':'tailscale status unavailable'}
    try:
        obj=json.loads(result.stdout)
        self_data=obj['Self']
        return {'schema':'1','ok':True,'result':{'advertised':self_data.get('ExitNodeOption') is True,'hostname':self_data.get('HostName'),'tailscale_ips':self_data.get('TailscaleIPs',[])}}
    except (KeyError, TypeError, ValueError):
        return {'schema':'1','ok':False,'error':'invalid tailscale status'}

def serve():
    if os.geteuid()!=0: raise RuntimeError('provider must run as root')
    if os.environ.get('LISTEN_PID') != str(os.getpid()) or os.environ.get('LISTEN_FDS') != '1':
        raise RuntimeError('requires exactly one systemd socket')
    uid_text=os.environ.get('NEEBLES_BOSS_UID','')
    if not uid_text.isdecimal(): raise RuntimeError('root-controlled NEEBLES_BOSS_UID required')
    authorized_uid=int(uid_text)
    listener=socket.socket(fileno=SOCKET_FD)
    if listener.family != socket.AF_UNIX or listener.type & socket.SOCK_STREAM != socket.SOCK_STREAM:
        raise RuntimeError('socket must be Unix stream')
    listener.settimeout(30)
    while True:
        try: client,_=listener.accept()
        except socket.timeout: continue
        with client:
            client.settimeout(8)
            rawcred=client.getsockopt(socket.SOL_SOCKET,socket.SO_PEERCRED,12)
            _,peer_uid,_=struct.unpack('3i',rawcred)
            line=bytearray()
            while len(line)<16384:
                b=client.recv(1)
                if not b or b==b'\n':break
                line.extend(b)
            try:
                if len(line)>=16384: raise ValueError('oversized request')
                req=json.loads(line)
                res=respond(req,peer_uid,authorized_uid)
            except (ValueError,UnicodeDecodeError):
                res={'schema':'1','ok':False,'error':'malformed request'}
            client.sendall(json.dumps(res,separators=(',',':')).encode()+b'\n')

if __name__=='__main__':
    if sys.argv[1:]!=['--serve']:
        raise SystemExit('Usage: neebles-platform-provider --serve (systemd socket activation)')
    serve()

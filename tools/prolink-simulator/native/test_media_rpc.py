"""Loopback/host RPC acceptance; synthetic markers only, no USB mutation."""
import hashlib
import json
import os
from pathlib import Path
import select
import socket
import struct
import subprocess
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parents[3]
SERVER = ROOT / 'build/simulator-native/lumi-simulator-rpc'
def words(*values): return struct.pack('!' + 'I' * len(values), *values)
def text(value):
    raw = value.encode('utf-16le')
    return words(len(raw)) + raw + bytes((-len(raw)) % 4)

class MediaRpcTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='lumi-rpc-')
        self.roots = [Path(self.temp.name) / str(i) for i in range(2)]
        self.markers=[]
        for i, root in enumerate(self.roots):
            root.mkdir()
            marker=json.dumps({'schemaVersion':1,'mediaId':f'00000000-0000-4000-8000-00000000000{i}',
                               'sourceId':f'usb-local:test{i}'}).encode()
            (root / '.lumi-media.json').write_bytes(marker)
            self.markers.append(marker)
        self.client=socket.socket(socket.AF_INET,socket.SOCK_DGRAM)
        self.client.settimeout(1)
        # Use a free high port for isolated regression runs; production uses 111.
        with socket.socket(socket.AF_INET,socket.SOCK_DGRAM) as reserve:
            reserve.bind(('127.0.0.1',0)); self.port=reserve.getsockname()[1]
        if os.environ.get('LUMI_RPC_STANDARD_PORT')=='1': self.port=111
        route=subprocess.check_output(['/sbin/route','-n','get','default'],text=True)
        interface=next(line.split(':',1)[1].strip() for line in route.splitlines() if 'interface:' in line)
        address=subprocess.check_output(['/usr/sbin/ipconfig','getifaddr',interface],text=True).strip()
        self.addresses=['127.0.0.1', address]
        self.proc=subprocess.Popen([str(SERVER),str(self.port),self.addresses[0],str(self.roots[0]),
                                    self.addresses[1],str(self.roots[1])],stdin=subprocess.PIPE,stdout=subprocess.PIPE)
        self.assertTrue(select.select([self.proc.stdout],[],[],3)[0])
        self.assertEqual(self.proc.stdout.readline(),b'READY\n')
        self.xid=0
    def tearDown(self):
        self.client.close()
        self.proc.stdin.close()
        self.proc.wait(timeout=3)
        self.proc.stdout.close()
        self.temp.cleanup()
    def rpc(self, index, program, version, procedure, args=b''):
        self.client.close()
        self.client=socket.socket(socket.AF_INET,socket.SOCK_DGRAM)
        self.client.settimeout(1)
        self.xid+=1
        packet=words(self.xid,0,2,program,version,procedure,0,0,0,0)+args
        self.client.sendto(packet,(self.addresses[index],self.port))
        data,peer=self.client.recvfrom(8192)
        self.assertEqual(peer,(self.addresses[index],self.port))
        self.assertEqual(data[:24],words(self.xid,1,0,0,0,0))
        return data[24:]
    def mount(self,i):
        result=self.rpc(i,100005,1,1,text('/C/')); self.assertEqual(result[:4],words(0)); return result[4:]
    def lookup(self,i,root): return self.rpc(i,100003,2,4,root+text('.lumi-media.json'))
    def read(self,i,handle,count): return self.rpc(i,100003,2,6,handle+words(0,count,0))
    def test_two_destinations_exact_markers_and_no_writes(self):
        for i in range(2):
            self.assertEqual(self.rpc(i,100000,2,3,words(100003,2,17,0)),words(self.port))
            found=self.lookup(i,self.mount(i)); self.assertEqual(found[:4],words(0))
            result=self.read(i,found[4:36],len(self.markers[i]))
            self.assertEqual(result[76:76+len(self.markers[i])],self.markers[i])
            self.assertEqual((self.roots[i]/'.lumi-media.json').read_bytes(),self.markers[i])
        # Cross-player handle must not resolve against the other USB.
        self.assertEqual(self.lookup(1,self.mount(0)),words(70))
    def test_eject_reinsert_invalidates_old_handles(self):
        root=self.mount(0); handle=self.lookup(0,root)[4:36]
        self.proc.stdin.write(b'0 0 0\n'); self.proc.stdin.flush(); time.sleep(.1)
        self.assertEqual(self.read(0,handle,1),words(70))
        self.proc.stdin.write(b'0 1 0\n'); self.proc.stdin.flush(); time.sleep(.1)
        self.assertEqual(self.lookup(0,root),words(70))
        self.assertEqual(self.lookup(0,self.mount(0))[:4],words(0))
    def test_missing_symlink_oversize_and_changed_marker(self):
        root=self.mount(0); handle=self.lookup(0,root)[4:36]
        path=self.roots[0]/'.lumi-media.json'
        path.write_bytes(b'changed')
        self.assertEqual(self.read(0,handle,1),words(70))
        path.write_bytes(b'x'*4097)
        self.assertEqual(self.lookup(0,root),words(13))
        path.unlink(); self.assertEqual(self.lookup(0,root),words(2))
        path.symlink_to(self.roots[1]/'.lumi-media.json')
        self.assertEqual(self.lookup(0,root),words(13))
    def test_paths_and_read_bounds(self):
        root=self.mount(0)
        self.assertEqual(self.rpc(0,100003,2,4,root+text('../secret')),words(2))
        handle=self.lookup(0,root)[4:36]
        self.assertEqual(self.read(0,handle,4096),words(13))
        packet=words(99,0,2,100003,2,8,0,0,0,0)
        self.client.sendto(packet,('127.0.0.1',self.port))
        self.assertEqual(self.client.recv(8192),words(99,1,0,0,0,3))

    def test_faults_and_recovery(self):
        root=self.mount(0)
        self.proc.stdin.write(b'0 1 2\n'); self.proc.stdin.flush(); time.sleep(.05)
        self.assertEqual(self.lookup(0,root),words(2))
        self.proc.stdin.write(b'0 1 1\n'); self.proc.stdin.flush(); time.sleep(.05)
        with self.assertRaises(TimeoutError): self.lookup(0,root)
        self.proc.stdin.write(b'0 1 0\n'); self.proc.stdin.flush(); time.sleep(.05)
        self.assertEqual(self.lookup(0,root)[:4],words(0))

    def test_unconfigured_destination_and_malformed_rpc_do_not_escape(self):
        self.client.sendto(words(1,0,2,100003,2,4,0,0xffffffff),('127.0.0.1',self.port))
        with self.assertRaises(TimeoutError): self.client.recv(8192)
        self.assertEqual(self.lookup(0,self.mount(0))[:4],words(0))
        self.client.sendto(words(88,0,2,100000,2,0,0,0,0,0),('127.0.0.2',self.port))
        with self.assertRaises(TimeoutError): self.client.recv(8192)

    def test_parent_pipe_close_releases_port(self):
        self.proc.stdin.close()
        self.assertEqual(self.proc.wait(timeout=3),0)
        with socket.socket(socket.AF_INET,socket.SOCK_DGRAM) as check:
            check.bind(('0.0.0.0',self.port))

    def test_conflicting_service_is_not_replaced(self):
        result=subprocess.run([str(SERVER),str(self.port),self.addresses[0],str(self.roots[0])],
                              stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=3)
        self.assertEqual(result.returncode,3)
        self.assertEqual(self.lookup(0,self.mount(0))[:4],words(0))

    @unittest.skipUnless(os.environ.get('LUMI_RPC_STANDARD_PORT')=='1', 'requires standard port acceptance')
    def test_unchanged_production_reader(self):
        java=ROOT/'build/package-toolchains/temurin-21-macos-aarch64/Contents/Home/bin/java'
        jar=ROOT/'bridges/prolink/target/lumi-prolink-bridge.jar'
        result=subprocess.run([str(java),'-cp',str(jar),
            'co.victorblan.tech.lumi.prolink.MediaIdentityProbeMain','--worker-marker',self.addresses[1]],
            text=True,capture_output=True,timeout=9)
        self.assertEqual(result.returncode,0,result.stdout+result.stderr)
        reply=json.loads(result.stdout)
        self.assertEqual(reply['outcome'],'marker_read')
        self.assertEqual(reply['sha256'],hashlib.sha256(self.markers[1]).hexdigest())

if __name__=='__main__': unittest.main()

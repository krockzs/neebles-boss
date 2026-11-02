import json
import unittest
from unittest.mock import Mock
import provider
class Tests(unittest.TestCase):
    def test_wrong_peer(self):
        self.assertFalse(provider.respond({'schema':'1','module':'neebles-proxy-server','operation':'tailscale.exit_node.status'},100,101)['ok'])
    def test_mutation_denied(self):
        for action in ['activate','deactivate']:
            self.assertFalse(provider.respond({'schema':'1','module':'neebles-proxy-server','operation':f'tailscale.exit_node.{action}'},100,100)['ok'])
    def test_exact_request(self):
        r=provider.respond({'schema':'1','module':'neebles-proxy-server','operation':'tailscale.exit_node.status'},100,100,lambda *a,**kw:Mock(returncode=0,stdout=json.dumps({'Self':{'ExitNodeOption':True,'HostName':'server','TailscaleIPs':['100.71.3.52']}})))
        self.assertTrue(r['ok']);self.assertTrue(r['result']['advertised'])
    def test_extra_fields_denied(self):
        self.assertFalse(provider.respond({'schema':'1','module':'neebles-proxy-server','operation':'tailscale.exit_node.status','shell':'x'},1,1)['ok'])
if __name__=='__main__':unittest.main()

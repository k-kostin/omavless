"""Synthetic only: no candidate/readelf execution, dependency FD or guest."""
import copy
import hashlib
import json
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch
from tests.encoder_unlisted_dependency import probe, validator, supervisor, transport, owned
from tests import test_encoder_boundary_provenance as prior
from tests import test_brotli_encoder_owned as old_owned
from tests import test_live_fd_transport as old_transport
from tests import test_encoder_boundary_events as old_events

ROOT=Path(__file__).parent
MANIFEST=prior.MANIFEST
UNLISTED='/usr/lib/synthetic-unlisted.so.1'


def fixture(names=None):
    sources,base,helper=prior.capture_fixture()
    old=sources.take.side_effect
    def take(path):
        row=old(path);sources.files[path]=row;return row
    sources.take.side_effect=take
    helper.decode_readelf.side_effect=None
    helper.decode_readelf.return_value=dict(needed=names or ['synthetic-unlisted.so.1'],
                                           interpreter=None,declared_search_tokens=[])
    helper.resolve_needed.side_effect=lambda name:('/usr/lib/'+name,'/usr/lib/'+name,[])
    return sources,base,helper


def receipt():
    sources,base,helper=fixture()
    with patch.object(probe,'Sources',return_value=sources):
        return probe.capture(base,helper,helper.owned,MANIFEST)


class UnlistedTests(unittest.TestCase):
    def test_exact_graph(self):
        for name,pin in transport.PINS.items():
            path=ROOT/'encoder_unlisted_dependency'/name
            if name=='containment.py':path=ROOT/'real_resolved_binary/probe.py'
            if name=='helpers.py':path=ROOT/'static_elf_provenance/probe_four_mib.py'
            if name=='copy-manifest.json':path=ROOT/'decoder_copy_admission/copy-manifest.json'
            self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(),pin)
        self.assertEqual(supervisor.PINS['probe.py'],transport.PINS['probe.py'])
        self.assertEqual(len(MANIFEST['source_provenance']),17)
        self.assertNotIn(probe.CANDIDATE,MANIFEST['source_provenance'])

    def test_first_unlisted_is_data_only_exact_four_fds_one_tool(self):
        sources,base,helper=fixture(['libc.so.6','synthetic-unlisted.so.1','never-inspected.so'])
        with patch.object(probe,'Sources',return_value=sources):
            value=probe.capture(base,helper,helper.owned,MANIFEST)
        self.assertEqual(validator.validate(value,MANIFEST),value)
        self.assertEqual(value['outcome'],'OBSERVED_UNLISTED_DEPENDENCY')
        self.assertEqual(value['unlisted_dependency']['resolved_path'],UNLISTED)
        self.assertEqual([e['listed_in_manifest'] for e in value['resolved_prefix']],[True,False])
        self.assertEqual(set(sources.files),{probe.CANDIDATE,probe.READELF,probe.PACKAGE+'desc',probe.PACKAGE+'files'})
        self.assertEqual(helper.resolve_needed.call_count,2)
        base.command.assert_called_once()
        self.assertEqual(sources.recheck.call_count,3)
        for flag in ('dependency_elf_opened','unlisted_object_identity_proven','candidate_elf_executed','allowlist_adoption','loaded_elf_identity_proven','compatibility_acceptance'):
            self.assertIs(value[flag],False)

    def test_actual_sources_reject_all_dependency_fds_before_open(self):
        for path in (UNLISTED,'/usr/lib/libc.so.6'):
            sources=probe.Sources(MANIFEST['source_provenance'])
            with patch.object(probe.os,'open') as opened:
                with self.assertRaises(RuntimeError):sources.take(path)
                opened.assert_not_called()
            self.assertEqual(sources.state,'refused')

    def test_known_only_or_no_needed_cannot_be_promoted(self):
        for needed in ([],['libc.so.6']):
            sources,base,helper=fixture()
            helper.decode_readelf.return_value['needed']=needed
            with patch.object(probe,'Sources',return_value=sources),self.assertRaises(RuntimeError):
                probe.capture(base,helper,helper.owned,MANIFEST)
            base.command.assert_called_once()

    def test_unknown_resolver_or_final_original_recheck_seals_before_output(self):
        for phase in ('resolve','recheck'):
            sources,base,helper=fixture();events=probe.Events()
            if phase=='resolve':helper.resolve_needed.side_effect=OSError('private')
            else:sources.recheck.side_effect=[None,None,OSError('private')]
            with patch.object(probe,'EVENTS',events),patch.object(probe.os,'write',side_effect=lambda _,b:len(b)),patch.object(probe,'Sources',return_value=sources):
                with self.assertRaises(OSError):probe.capture(base,helper,helper.owned,MANIFEST)
                with self.assertRaises(RuntimeError):probe.capture(base,helper,helper.owned,MANIFEST)
            self.assertTrue(events.sealed);base.command.assert_called_once()

    def test_public_edge_rejects_private_invalid_chain_and_boolean_metadata(self):
        link={'path':'/usr/lib/synthetic.so','target':'synthetic-unlisted.so.1',
              'identity':[31,1,23,0,0,0o120777,1,1,1]}
        link['identity'][2]=len(link['target'])
        good=probe.public_edge('synthetic.so',link['path'],UNLISTED,[link],{})
        self.assertFalse(good['listed_in_manifest'])
        for change in ('private','bool','target','extra','chain'):
            row=copy.deepcopy(link);target=UNLISTED
            if change=='private':target='/private/secret'
            elif change=='bool':row['identity'][1]=True
            elif change=='target':row['target']='../private/secret'
            elif change=='extra':row['secret']='x'
            else:row['path']='/usr/lib/wrong.so'
            with patch.object(probe.os,'open') as opened,self.assertRaises(RuntimeError):
                probe.public_edge('synthetic.so',link['path'],target,[row],{})
            opened.assert_not_called()

    def test_strict_receipt_duplicates_types_prefix_and_false_proofs(self):
        good=receipt()
        variants=[]
        for key,value in [('dependency_elf_opened',0),('unlisted_object_identity_proven',0),
                          ('resolved_prefix','x'),('outcome','PASS')]:
            bad=copy.deepcopy(good);bad[key]=value;variants.append(bad)
        bad=copy.deepcopy(good);bad['unlisted_dependency']['resolved_path']='/private';variants.append(bad)
        bad=copy.deepcopy(good);bad['resolved_prefix'][0]['listed_in_manifest']=True;variants.append(bad)
        bad=copy.deepcopy(good);bad['candidate_record']['identity'][1]=False;variants.append(bad)
        bad=copy.deepcopy(good);bad['candidate_record']['dynamic']['needed']=['other.so'];variants.append(bad)
        bad=copy.deepcopy(good)
        bad['unlisted_dependency']=copy.deepcopy(bad['unlisted_dependency'])
        bad['unlisted_dependency']['listed_in_manifest']=0
        variants.append(bad)
        for value in variants:
            with self.assertRaises((ValueError,TypeError)):validator.validate(value,MANIFEST)
        for raw in (b'{"a":1,"a":2}',b'NaN',b'x'*262145):
            with self.assertRaises(ValueError):validator.decode(raw)

    def test_last_unlisted_duplicate_not_inferred_from_later_edge(self):
        value=receipt()
        value['resolved_prefix'].append(copy.deepcopy(value['resolved_prefix'][0]))
        value['candidate_record']['dynamic']['needed'].append('another.so')
        with self.assertRaises(ValueError):validator.validate(value,MANIFEST)

    def test_edge_validator_matches_producer_as_ast(self):
        import ast
        def tree(module):
            node=next(n for n in ast.parse(Path(module.__file__).read_text()).body
                      if isinstance(n,ast.FunctionDef) and n.name=='public_edge')
            for item in ast.walk(node):
                if isinstance(item,ast.Name) and item.id=='require':item.id='need'
            return ast.dump(node,include_attributes=False)
        self.assertEqual(tree(probe),tree(validator))

    def test_noncanonical_candidate_or_tool_unknown_never_resolves_dependency(self):
        for failure in ('candidate','tool'):
            sources,base,helper=fixture()
            if failure=='candidate':helper.canonical_public.side_effect=lambda _:(probe.CANDIDATE,['alias'])
            else:base.command.side_effect=RuntimeError('unknown')
            with patch.object(probe,'Sources',return_value=sources),self.assertRaises(RuntimeError):
                probe.capture(base,helper,helper.owned,MANIFEST)
            helper.resolve_needed.assert_not_called()

    def test_original_fd_mutation_and_permanent_deadline_controls(self):
        with patch.object(prior,'probe',probe):
            old=prior.EncoderTests()
            old.test_actual_original_fd_replacement_and_change_reversion_seal()
            old.test_sources_unknown_and_deadline_are_permanent_no_reads()

    def test_terminal_wrapper_and_supervisor_controls(self):
        # Call original inert cases with only module/folder selection replaced.
        with patch.object(prior,'supervisor',supervisor):
            prior.EncoderTests().test_supervisor_unknown_nonzero_and_boolean_status_never_follow_up()
        import subprocess,tempfile
        source=(ROOT/'encoder_unlisted_dependency/vm-guard.sh').read_text()
        fragment=source.split('task_failed=0\n',1)[1].split('check_category() {',1)[0]
        for failure in ('supervisor','validator'):
            with tempfile.TemporaryDirectory(dir='/var/tmp') as folder:
                stage=Path(folder);(stage/'scratch').mkdir()
                for name in ('supervisor','validator'):
                    (stage/(name+'.py')).write_text('raise SystemExit('+('1' if name==failure else '0')+')\n')
                result=subprocess.run(['/bin/bash','-c',fragment+'printf FORBIDDEN'],env={'PATH':'/usr/bin','task_stage':folder},capture_output=True,timeout=5)
                self.assertNotEqual(result.returncode,0);self.assertNotIn(b'FORBIDDEN',result.stdout)


class OwnedTests(old_owned.OwnedTests):
    def setUp(self):
        selected=patch.object(old_owned,'owned',owned);selected.start();self.addCleanup(selected.stop)


class EventsTests(old_events.EventsTests):
    def setUp(self):
        for name,value in (('probe',probe),('capture_fixture',fixture)):
            selected=patch.object(old_events,name,value);selected.start();self.addCleanup(selected.stop)


FROZEN_PINS=dict(transport.PINS)
class TransportTests(old_transport.TransportTests):
    def setUp(self):
        selected=patch.object(old_transport,'transport',transport);selected.start();self.addCleanup(selected.stop)
        super().setUp()

    def test_all_eight_transport_pins_match_exact_source_generation(self):
        with patch.object(transport,'PINS',FROZEN_PINS):UnlistedTests().test_exact_graph()


if __name__=='__main__':unittest.main()

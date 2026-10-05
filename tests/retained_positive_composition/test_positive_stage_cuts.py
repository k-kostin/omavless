"""Synthetic whole-coordinator cuts; no native, FD, child, thread or socket.

Only Case.run is reached. Helper/stream/owner bodies are inert adapters; their
separate real-source unit controls remain required. This proves coordinator
ordering/no downstream call in this model, not actual resource custody.
"""
import importlib.util
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

spec = importlib.util.spec_from_file_location('case_controls', Path(__file__).with_name('test_positive.py'))
controls = importlib.util.module_from_spec(spec)
spec.loader.exec_module(controls)
p = controls.p

OPERATIONS = (
    'images.verify', 'write.dbus', 'spawn.bus', 'ready.bus', 'mapped.bus',
    'spawn.resolved', 'ready.resolved', 'verify.resolved', 'mapped.resolved',
    'spawn.broker', 'spawn.host', 'helper.construct', 'helper.ready',
    'verify.host', 'native_ready.host', 'mapped.host', 'live.broker',
    'broker.release', 'live.broker', 'broker.stdin.close', 'snapshot.broker_ready',
    'verify.broker', 'native_ready.broker', 'mapped.broker', 'mkdir.core',
    'chown.core', 'write.core', 'spawn.core', 'snapshot.active', 'verify.core',
    'bootstrap.construct', 'bootstrap.receipt', 'controller.construct',
    'controller.ready', 'native_ready.core', 'mapped.core', 'streams.construct',
    'streams.witness', 'helper.snapshot', 'base.active', 'controller.ready',
    'streams.finish', 'final_map.core', 'final_map.broker', 'final_map.host',
    'shutdown.core', 'snapshot.released', 'shutdown.broker', 'snapshot.final',
    'helper.positive_eof', 'host.zero', 'final_map.resolved', 'shutdown.resolved',
    'final_map.bus', 'shutdown.bus', 'owner.complete',
)
PHASES = (
    'before_initial_images_verify', 'after_initial_images_verify',
    'before_bus_spawn', 'after_bus_maps', 'before_resolved_spawn', 'after_resolved_maps',
    'before_broker_spawn', 'before_host_spawn', 'after_host_maps',
    'before_broker_release', 'after_broker_maps', 'before_core_spawn',
    'after_core_active', 'after_core_bootstrap', 'after_core_maps',
    'before_stream_witness', 'after_stream_witness', 'before_stream_finish',
    'after_stream_finish', 'before_core_shutdown', 'after_core_shutdown',
    'after_dns_release', 'before_broker_shutdown', 'after_broker_shutdown',
    'before_host_finish', 'after_host_zero', 'before_daemon_shutdown',
    'after_resolved_zero', 'after_bus_zero', 'before_complete', 'after_complete',
)


class Synthetic:
    def __init__(self, cut=None):
        self.events = []
        self.cut = cut
        self.value, self.clock = controls.Controls().fixture()
        self.value.copies.records = []
        self.owner = self.value.owner
        self.children = {}

    def event(self, kind, name, edge):
        event = (kind, name, edge)
        self.events.append(event)
        if len(self.events) - 1 == self.cut:
            raise RuntimeError('synthetic_first_uncertainty')

    def operation(self, name, result=None, action=None):
        def call(*args, **kwargs):
            self.event('operation', name, 'before')
            value = action(*args, **kwargs) if action is not None else result
            self.event('operation', name, 'after')
            return value
        return call

    def run(self):
        value, owner = self.value, self.owner
        rows = [{'path': path} for path in sorted(p.SIX)]
        value.images.verify = self.operation('images.verify')
        value.write = lambda path, *args: self.operation('write.dbus' if path == '/tmp/dbus.xml' else 'write.core')()
        def spawned(role, *args, **kwargs):
            child = SimpleNamespace(pid=10 + len(self.children))
            if role == 'broker':
                child.stdin = FakeInput()
                child.stdin.close = self.operation('broker.stdin.close')
            self.children[role] = value.children[role] = child
            return child
        value.spawn = lambda role, *args, **kwargs: self.operation('spawn.' + role, action=spawned)(role)
        def mapped(role):
            value.initial[role] = rows
        value.mapped = lambda role: self.operation('mapped.' + role, action=mapped)(role)
        def final_map(role):
            value.final[role] = rows
        value.final_map = lambda role: self.operation('final_map.' + role, action=final_map)(role)
        value.wait_snapshot = lambda helper, kind: self.operation('snapshot.' + kind, controls.frame('active' if kind == 'active' else 'final' if kind == 'released' else 'ready'))()
        value.final_snapshot = self.operation('snapshot.final', controls.frame('final'))
        owner.phase = lambda name, *args: self.event('phase', name, 'emit')
        owner.ready = lambda role: self.operation('ready.' + role)()
        owner.native_ready = lambda role: self.operation('native_ready.' + role)()
        owner.live = lambda child: self.operation('live.broker')()
        owner.perform = lambda fn, *args: fn(*args)
        owner.shutdown = lambda role, *args: self.operation('shutdown.' + role, {'original_zero': True})()
        owner.host_finished_zero = self.operation('host.zero', {'original_zero': True})
        owner.complete = self.operation('owner.complete', {'five_owned_zero': True})
        value.base.bus_config = lambda kind: 'synthetic-public-config'
        value.base.resolved_exec = lambda: ['INERT_RESOLVED']
        value.base.cap_exec = lambda name, uid, args: ['INERT_' + name, *args]
        value.base.RESOLVER_CAPS = value.base.CAP = ()
        value.base.verify_child = lambda child, *args: self.operation('verify.' + next(role for role, original in self.children.items() if original is child))()
        value.base.active = self.operation('base.active')
        helper = SimpleNamespace(ready=self.operation('helper.ready'),
            snapshot=self.operation('helper.snapshot', controls.frame('active')),
            positive_eof=self.operation('helper.positive_eof'))
        bootstrap = SimpleNamespace(receipt=self.operation('bootstrap.receipt', {'synthetic': True}))
        controller = SimpleNamespace(ready=self.operation('controller.ready'))
        streams = SimpleNamespace(witness=self.operation('streams.witness', {'synthetic': True}),
            finish_positive=self.operation('streams.finish', {'synthetic': True}))
        value.helper_module.Helper = self.operation('helper.construct', helper)
        value.bootstrap_module.Bootstrap = self.operation('bootstrap.construct', bootstrap)
        value.controller_module.Controller = self.operation('controller.construct', controller)
        value.stream_module.Streams = self.operation('streams.construct', streams)
        with patch.object(p.io, 'FileIO', FakeInput), \
             patch.object(p.os, 'write', self.operation('broker.release', 1)), \
             patch.object(p.os, 'mkdir', self.operation('mkdir.core')), \
             patch.object(p.os, 'chown', self.operation('chown.core')):
            return value.run()


class FakeInput:
    def fileno(self):
        return 71  # No real descriptor; os.write is replaced before Case.run.


class StageCuts(unittest.TestCase):
    def positive(self):
        synthetic = Synthetic()
        result = synthetic.run()
        self.assertEqual(tuple(name for kind, name, edge in synthetic.events
                               if kind == 'operation' and edge == 'before'), OPERATIONS)
        self.assertEqual(tuple(name for kind, name, edge in synthetic.events
                               if kind == 'phase'), PHASES)
        self.assertEqual(len(synthetic.events), 2 * len(OPERATIONS) + len(PHASES))
        return synthetic, result

    def test_complete_synthetic_order_only_returns_non_authoritative_result(self):
        synthetic, result = self.positive()
        self.assertEqual(result['case'], 'success')
        self.assertFalse(result['parent_whole_known_zero'])
        for key in ('production_effect_authority', 'installed_compatibility', 'normal_owner_adoption'):
            self.assertIs(result[key], False)
        self.assertFalse(synthetic.value.sealed or synthetic.owner.sealed)

    def test_every_before_after_and_phase_cut_is_terminal_with_no_downstream_call(self):
        complete, _ = self.positive()
        for index, event in enumerate(complete.events):
            with self.subTest(index=index, event=event):
                synthetic = Synthetic(index)
                with self.assertRaisesRegex(p.Refused, '^retained_positive_case_refused$'):
                    synthetic.run()
                self.assertEqual(synthetic.events, complete.events[:index + 1])
                self.assertTrue(synthetic.value.sealed and synthetic.owner.sealed)

    def test_refusal_cannot_be_reentered_after_model_clock_recovers(self):
        complete, _ = self.positive()
        for operation in ('streams.witness', 'streams.finish', 'shutdown.core',
                          'snapshot.released', 'snapshot.final', 'host.zero', 'owner.complete'):
            index = complete.events.index(('operation', operation, 'after'))
            synthetic = Synthetic(index)
            with self.assertRaises(p.Refused):
                synthetic.run()
            saved = list(synthetic.events)
            synthetic.clock[0] = 0.0
            with self.assertRaises(p.Refused):
                synthetic.value.run()
            self.assertEqual(synthetic.events, saved)


if __name__ == '__main__':
    unittest.main()

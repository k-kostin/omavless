"""Run the existing actual native O_EXCL controls on the fixed encoder helper."""
from unittest.mock import patch
from tests import test_live_fd_transport as original
from tests import test_brotli_encoder_provenance as encoder_tests
from tests.brotli_encoder_provenance import transport
FROZEN_PINS = dict(transport.PINS)


class EncoderTransportTests(original.TransportTests):
    def setUp(self):
        selected = patch.object(original, 'transport', transport)
        selected.start()
        self.addCleanup(selected.stop)
        super().setUp()

    def test_all_eight_transport_pins_match_exact_source_generation(self):
        # The predecessor's path routing is intentionally not reused.
        with patch.object(transport, 'PINS', FROZEN_PINS):
            encoder_tests.EncoderTests().test_entire_exact_dependency_graph_and_no_old_main()

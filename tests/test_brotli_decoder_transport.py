"""Run the existing actual native O_EXCL controls on the fixed decoder helper."""
from unittest.mock import patch
from tests import test_live_fd_transport as original
from tests import test_brotli_decoder_provenance as decoder_tests
from tests.brotli_decoder_provenance import transport
FROZEN_PINS = dict(transport.PINS)


class DecoderTransportTests(original.TransportTests):
    def setUp(self):
        selected = patch.object(original, 'transport', transport)
        selected.start()
        self.addCleanup(selected.stop)
        super().setUp()

    def test_all_eight_transport_pins_match_exact_source_generation(self):
        # The predecessor's path routing is intentionally not reused.
        with patch.object(transport, 'PINS', FROZEN_PINS):
            decoder_tests.DecoderTests().test_entire_exact_dependency_graph_and_no_old_main()

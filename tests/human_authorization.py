# SPDX-License-Identifier: MIT
"""Developer acceptance guard, not a product auth API or password reader.

No prompt/process heuristic proves host authorization settled. Require a human
on a terminal before and after EACH potentially authorizing host effect. Never
retry, auto-confirm, expire the human wait, or run compensating effects after
the human stops. Read only fixed acknowledgement words, never OS passwords.
"""
import sys


class AuthorizationUnsettled(Exception):
    def __init__(self):
        super().__init__("human_authorization_unsettled")


class HumanAuthorization:
    PHASES = frozenset(("connect", "disconnect", "mode_change", "restore_mode", "socket_inspection",
                        "service_start", "service_stop"))

    def __init__(self, input_stream=None, output_stream=None):
        self.input = sys.stdin if input_stream is None else input_stream
        self.output = sys.stderr if output_stream is None else output_stream
        self.blocked = False
        self.in_flight = False

    def require_terminal(self):
        try:
            permitted = self.input.isatty() and self.output.isatty()
        except Exception:
            permitted = False
        if self.blocked or not permitted:
            self.blocked = True
            raise AuthorizationUnsettled()

    def _acknowledge(self, phase, after):
        self.require_terminal()
        word = "settled" if after else "ready"
        detail = ("Resolve ALL operating-system authorization dialogs first. "
                  "Confirm they are settled, including cancellation/failure."
                  if after else
                  "One host action may open authorization dialogs. "
                  "Confirm no earlier authorization is pending and you can attend.")
        try:
            self.output.write(f"[{phase}] {detail}\n"
                              f"Do NOT enter a password here. Type {word} to continue; "
                              "anything else stops automatic host actions: ")
            self.output.flush()
            answer = self.input.readline(33)
            accepted = answer in (word + "\n", word + "\r\n")
        except (Exception, KeyboardInterrupt):
            accepted = False
        if not accepted:
            self.blocked = True
            raise AuthorizationUnsettled()

    def step(self, phase, effect):
        if phase not in self.PHASES or self.in_flight or self.blocked:
            self.blocked = True
            raise AuthorizationUnsettled()
        self._acknowledge(phase, after=False)
        self.in_flight = True
        try:
            try:
                return effect()
            finally:
                # Even an exception/timeout/interrupt can leave an OS dialog.
                # A successful local CLI return is not an authorization proof.
                self._acknowledge(phase, after=True)
        finally:
            self.in_flight = False

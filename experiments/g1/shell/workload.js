// SPDX-License-Identifier: MIT
// Finite synthetic producer only; no timer, daemon, transport or credentials.
import { Freshness } from "./freshness.js";

export class UpdateView {
  freshness = new Freshness();
  pending = null;
  state = { phase: "unverified", connected: null, sample: 0 };

  attach() {
    this.freshness.attach("synthetic-source");
    this.pending = null;
    this.state = { phase: "unverified", connected: null, sample: 0 };
  }

  receive(ticket, snapshot) {
    if (!this.freshness.receive(ticket, snapshot)) return false;
    // One replacement slot regardless of producer rate. Admission happens
    // before replacement: an old epoch/revision cannot displace newer data.
    this.pending = {
      phase: ["connected", "connecting", "switching", "reconnecting", "unverified",
        "failed", "recovery", "disconnected"].includes(snapshot.phase) ? snapshot.phase : "unverified",
      connected: snapshot.phase === "connected" || snapshot.phase === "switching"
        ? snapshot.connected : null,
      sample: snapshot.revision,
    };
    return true;
  }

  flush() {
    if (this.pending === null) return false;
    this.state = this.pending;
    this.pending = null;
    return true;
  }

  lost() {
    this.freshness.lost();
    this.pending = null;
    this.state = { phase: "unverified", connected: null, sample: 0 };
  }

  burst(scene, count = 1000) {
    if (!Number.isInteger(count) || count < 1 || count > 1000) return false;
    const ticket = this.freshness.request();
    if (ticket === null) return false;
    const start = this.freshness.revision ?? 0;
    for (let offset = 1; offset <= count; offset += 1) {
      this.receive(ticket, { instance: ticket.instance, revision: start + offset,
        phase: scene.phase, connected: scene.connected });
    }
    return this.flush();
  }

  available() { return this.freshness.request() !== null; }
}

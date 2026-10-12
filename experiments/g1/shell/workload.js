// SPDX-License-Identifier: MIT
// Finite synthetic producer only; no timer, daemon, transport or credentials.
import { Freshness } from "./freshness.js";
import { Metadata, validId } from "./metadata.js";

export class UpdateView {
  constructor(metadata = new Metadata()) {
    this.metadata = metadata;
    this.admittedMetadata = metadata;
  }
  freshness = new Freshness();
  pending = null;
  state = { phase: "unverified", connected: null, sample: 0 };
  metadataFresh = false;
  generation = 0;
  admittedGeneration = 0;

  attach() {
    this.freshness.attach("synthetic-source");
    this.pending = null;
    this.state = { phase: "unverified", connected: null, sample: 0 };
    this.metadataFresh = false;
    this.admittedMetadata = this.metadata;
    this.admittedGeneration = 0;
  }

  receive(ticket, snapshot, generation = this.admittedGeneration, rows = null) {
    if (!Number.isSafeInteger(generation) || generation < 1 || generation < this.admittedGeneration
      || (snapshot.connected !== null && !validId(snapshot.connected))) return false;
    const candidate = rows === null ? this.admittedMetadata : Metadata.validate(rows, this.admittedMetadata);
    if (candidate === null || (rows === null && generation !== this.admittedGeneration)
      || (generation === this.admittedGeneration && candidate.content !== this.admittedMetadata.content)) return false;
    if (!this.freshness.receive(ticket, snapshot)) return false;
    this.admittedGeneration = generation;
    this.admittedMetadata = candidate;
    // One replacement slot regardless of producer rate. Admission happens
    // before replacement: an old epoch/revision cannot displace newer data.
    this.pending = {
      phase: ["connected", "connecting", "switching", "reconnecting", "unverified",
        "failed", "recovery", "disconnected"].includes(snapshot.phase) ? snapshot.phase : "unverified",
      connected: snapshot.phase === "connected" || snapshot.phase === "switching"
        ? snapshot.connected : null,
      sample: snapshot.revision,
      metadata: candidate,
    };
    return true;
  }

  flush() {
    if (this.pending === null) return false;
    const { metadata, ...state } = this.pending;
    this.state = state;
    this.metadata = metadata;
    this.metadataFresh = true;
    this.generation = this.admittedGeneration;
    this.pending = null;
    return true;
  }

  lost() {
    this.freshness.lost();
    this.pending = null;
    this.state = { phase: "unverified", connected: null, sample: 0 };
    this.metadataFresh = false;
    this.admittedMetadata = this.metadata;
    this.admittedGeneration = 0;
  }

  burst(scene, count = 1000) {
    if (!Number.isInteger(count) || count < 1 || count > 1000) return false;
    const ticket = this.freshness.request();
    if (ticket === null) return false;
    const start = this.freshness.revision ?? 0;
    for (let offset = 1; offset <= count; offset += 1) {
      const initial = this.admittedGeneration === 0;
      this.receive(ticket, { instance: ticket.instance, revision: start + offset,
        phase: scene.phase, connected: scene.connected },
      initial ? 1 : this.admittedGeneration, initial ? this.metadata.rows : null);
    }
    return this.flush();
  }

  available() { return this.freshness.request() !== null; }

  replace(scene, rows) {
    const ticket = this.freshness.request();
    if (ticket === null) return false;
    if (!this.receive(ticket, { instance: ticket.instance, revision: (this.freshness.revision ?? 0) + 1,
      phase: scene.phase, connected: scene.connected }, this.admittedGeneration + 1, rows)) return false;
    return this.flush();
  }
}

// @ts-check
// SPDX-License-Identifier: MIT
// Synthetic G1a projection experiment. Not the OmaVLESS daemon protocol.

export class Freshness {
  epoch = 0;
  /** @type {string | null} */ instance = null;
  /** @type {number | null} */ revision = null;
  /** @type {string | null} */ confirmed = null;

  /** @param {string} instance */
  attach(instance) {
    this.epoch += 1;
    this.instance = instance;
    this.revision = null;
    this.confirmed = null;
  }

  request() {
    return this.instance === null ? null : { epoch: this.epoch, instance: this.instance };
  }

  /**
   * @param {{epoch: number, instance: string}} ticket
   * @param {{instance: string, revision: number, phase: string, connected: string | null}} snapshot
   */
  receive(ticket, snapshot) {
    if (ticket.epoch !== this.epoch || ticket.instance !== this.instance
      || snapshot.instance !== ticket.instance
      || !Number.isSafeInteger(snapshot.revision) || snapshot.revision < 0
      || (this.revision !== null && snapshot.revision <= this.revision)) return false;
    this.revision = snapshot.revision;
    this.confirmed = snapshot.phase === "connected" ? snapshot.connected : null;
    return true;
  }

  lost() {
    this.epoch += 1;
    this.instance = null;
    this.revision = null;
    this.confirmed = null;
  }
}

// SPDX-License-Identifier: MIT
// Consumer generation/admission, NOT the native sampler's kernel I/O permit.
import { MAX_THEME_SEQUENCE } from "./theme-source.js";
export const POLL_MS = 500;
export class ThemeFollow {
  constructor() { this.following = false; this.generation = 0; this.inflight = null; this.lastSource = 0; this.request = 0; }
  mode(following) {
    if (typeof following !== "boolean") return false;
    if (!following) this.following = false;
    if (this.generation === MAX_THEME_SEQUENCE) return false;
    this.generation++;
    this.following = following;
    // Manual/Cancel does not release an unfinished read.
    return true;
  }
  begin() {
    if (!this.following || this.inflight !== null) return null;
    if (this.request === MAX_THEME_SEQUENCE) { this.following = false; return null; }
    this.inflight = Object.freeze({ generation: this.generation, request: ++this.request });
    return this.inflight;
  }
  complete(ticket, sequence) {
    if (ticket === null || this.inflight !== ticket) return false;
    this.inflight = null;
    if (!this.following || this.generation !== ticket.generation || !Number.isSafeInteger(sequence)
      || sequence <= this.lastSource || sequence > MAX_THEME_SEQUENCE) return false;
    this.lastSource = sequence;
    return true;
  }
  failed(ticket) {
    if (ticket === null || this.inflight !== ticket) return false;
    this.inflight = null;
    if (!this.following || this.generation !== ticket.generation) return false;
    this.mode(false);
    return true;
  }
}

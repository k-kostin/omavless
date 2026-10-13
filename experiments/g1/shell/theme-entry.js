// SPDX-License-Identifier: MIT
// Proposed dedicated entry, not used by the bare-host manifest. Never launched.
import G1Trial from "./main.js";
import { samplePalette } from "g1-synthetic-theme";
import { installThemeSource } from "./theme-client.js";
export default class G1ThemeTrial extends G1Trial {
  init(props, cx) {
    super.init(props, cx);
    installThemeSource(this, samplePalette);
  }
}

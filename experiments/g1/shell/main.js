// @ts-check
// SPDX-License-Identifier: MIT
// Synthetic, read-only G1 comparison. No daemon/IPC, network or private data.
import { View } from "gpui-kit";
import { InputState, Scrollbar, h_flex, v_flex, v_virtual_list } from "gpui-base";
import { set_theme } from "gpui-base";
import {
  AppShell, Badge, Button, Label, ListRow, MutedText, Panel, TextField,
  Title, TitleBar, applyOmarchyRoles, applyOmarchyStyle, omarchyTheme,
} from "./vendor/omarchy-ui/src/index.js";
import { fixture, visibleProfiles } from "./data.js";

const COLORS = `
mode = "dark"
background = "#1a1b26"
foreground = "#c0caf5"
accent = "#7aa2f7"
red = "#f7768e"
green = "#9ece6a"
yellow = "#e0af68"
blue = "#7aa2f7"
magenta = "#bb9af7"
cyan = "#7dcfff"
`;
const SHELL = "[font]\nbase-size = 12\n[spacing]\nscale = 1\nscale-with-font = true\n";

const copy = {
  en: {
    profiles: "Profiles", details: "Details", sources: "Subscriptions / sources", selected: "Selected for inspection",
    connected: "Confirmed connection", none: "None confirmed", invalid: "No valid selection",
    source: "Source",
    note: "Click a row or press Enter in search to inspect; never connects", large: "10,005 synthetic rows · virtualized",
    readonly: "No VPN action exists in this experiment.", search: "Search sample profiles…",
    phases: { connected: "Connected (synthetic)", connecting: "Connecting… (synthetic)", switching: "Switching server… (synthetic)", reconnecting: "Reconnecting… (synthetic)", unverified: "State unverified (synthetic)", failed: "Connection failed (synthetic)", recovery: "Recovery required (synthetic)", disconnected: "Disconnected (synthetic)" },
    scenes: { connected: "connected", connecting: "connecting", switching: "switching", reconnecting: "reconnecting", unverified: "unverified", failed: "failed", recovery: "recovery", removed: "removed" },
  },
  ru: {
    profiles: "Профили", details: "Детали", sources: "Подписки / источники", selected: "Выбрано для просмотра",
    connected: "Подтверждённое соединение", none: "Нет подтверждённого соединения", invalid: "Нет выбранного профиля",
    source: "Источник",
    note: "Нажмите строку или Enter в поиске для просмотра; подключения нет", large: "10 005 демонстрационных строк · виртуализация",
    readonly: "В этом эксперименте нет управления VPN.", search: "Поиск демонстрационных профилей…",
    phases: { connected: "Подключено (макет)", connecting: "Подключаемся… (макет)", switching: "Меняем сервер… (макет)", reconnecting: "Переподключаемся… (макет)", unverified: "Состояние не подтверждено (макет)", failed: "Подключение не удалось (макет)", recovery: "Требуется восстановление (макет)", disconnected: "Отключено (макет)" },
    scenes: { connected: "подключено", connecting: "подключение", switching: "смена", reconnecting: "переподключение", unverified: "не проверено", failed: "сбой", recovery: "восстановление", removed: "удалён" },
  },
};

export default class G1Trial extends View {
  init(_props, cx) {
    const tokens = applyOmarchyStyle(SHELL, { cornerRadius: 0, fontFamily: "monospace" });
    applyOmarchyRoles(COLORS);
    const theme = omarchyTheme(COLORS, cx.theme(), tokens);
    if (theme) set_theme(theme);
    this.search = InputState.new({ placeholder: "Search / Поиск" });
    this.search.on("change", (_event, context) => context.notify());
    this.search.on("submit", (_event, context) => {
      this.selected = visibleProfiles(this.search.value(), this.large, this.collection)[0]?.id ?? null;
      context.notify();
    });
    this.scene = 0;
    this.selected = fixture.scenes[0].selected;
    this.locale = "en";
    this.large = false;
    this.collection = "all";
    this.cachedQuery = "";
    this.cachedLarge = false;
    this.cachedCollection = "all";
    this.visible = fixture.profiles;
  }

  render(cx) {
    const listHeight = Math.max(8, Math.min(24, window.viewport_size().height / window.rem_size() - 20)) * window.rem_size();
    const strings = copy[this.locale];
    const scene = fixture.scenes[this.scene];
    const connected = fixture.profiles.find((item) => item.id === scene.connected);
    const selected = fixture.profiles.find((item) => item.id === this.selected);
    const query = this.search.value();
    if (query !== this.cachedQuery || this.large !== this.cachedLarge || this.collection !== this.cachedCollection) {
      this.visible = visibleProfiles(query, this.large, this.collection);
      this.cachedQuery = query;
      this.cachedLarge = this.large;
      this.cachedCollection = this.collection;
    }
    const visible = this.visible;
    const colors = cx.theme().colors;
    const statusTone = scene.phase === "connected" ? "success"
      : ["unverified", "failed", "recovery"].includes(scene.phase) ? "danger" : "warning";
    const scenes = h_flex().flex_wrap().gap(6).children(fixture.scenes.map((item, index) =>
      new Button(`scene-${item.id}`).label(strings.scenes[item.id]).outlined().selected(index === this.scene)
        .onClick((_event, context) => {
          this.scene = index;
          this.selected = item.selected;
          context.notify();
        }).build(cx)));
    const collections = h_flex().flex_wrap().gap(6).children(fixture.collections.map((item) =>
      new Button(`collection-${item.id}`)
        .label(item.id === "all" ? (this.locale === "ru" ? "Все" : "All")
          : item.id === "local" ? (this.locale === "ru" ? "Локальные" : "Local") : item.name)
        .outlined().selected(item.id === this.collection)
        .onClick((_event, context) => { this.collection = item.id; context.notify(); }).build(cx)));

    const row = (profile) => new ListRow(`profile-${profile.id}`)
      .selected(this.selected === profile.id)
      .child(v_flex().min_w_0()
        .child(new Label(profile.name).truncate().build(cx))
        .child(new MutedText(profile.host).size("caption").truncate().build(cx)))
      .child(profile.id === scene.connected
        ? new Badge(`connected-${profile.id}`).label(this.locale === "ru" ? "Подключено" : "Connected").tone("success").build(cx)
        : new MutedText("").build(cx))
      .build(cx).h(52);

    const list = v_flex().flex_1().min_h_0().gap(8)
      .child(new MutedText(strings.sources).build(cx))
      .child(collections)
      .child(new TextField().state(this.search).build(cx))
      .child(new MutedText(this.large
        ? (this.locale === "ru" ? `${visible.length} демонстрационных строк · виртуализация`
          : `${visible.length} synthetic rows · virtualized`) : strings.note).build(cx))
      .child(v_flex().relative().h(listHeight).min_h_0().overflow_hidden()
        .child(v_virtual_list("g1-profile-list", visible.length, 52,
          (index) => visible[index].id,
          (range) => Array.from({ length: range.end - range.start }, (_unused, offset) =>
            row(visible[range.start + offset])))
          .size_full().on_item_click((id, context) => { this.selected = id; context.notify(); }))
        .child(Scrollbar.vertical("g1-profile-list").absolute().inset_0()));
    const details = v_flex().gap(12)
      .child(new MutedText(strings.selected).build(cx))
      .child(new Label(selected?.name ?? strings.invalid).build(cx))
      .child(new MutedText(strings.source).build(cx))
      .child(new Label(selected?.subscription || (this.locale === "ru" ? "Локальный" : "Local")).build(cx))
      .child(new MutedText(strings.connected).build(cx))
      .child(new Label(connected?.name ?? strings.none).build(cx))
      .child(new MutedText(strings.readonly).build(cx));

    // Let native flex layout react to window geometry. A Shell view snapshot is
    // not rebuilt on resize, so a JS viewport breakpoint remains stale until
    // another action happens to refresh the view.
    const panels = h_flex().flex_1().flex_wrap()
      .items_start().min_h_0().min_w_0().gap(12)
      .child(new Panel("profiles").title(strings.profiles).content(list).build(cx)
        .min_w("30rem").flex_basis("30rem").flex_grow(1))
      .child(new Panel("details").title(strings.details).content(details).build(cx)
        .min_w("30rem").flex_basis("30rem").flex_grow(1));
    const body = v_flex().size_full().min_h_0().p(18).gap(12)
      .overflow_y_scrollbar()
      .child(new Badge("state").label(strings.phases[scene.phase]).tone(statusTone).build(cx))
      .child(scenes)
      .child(new Button("large-list").label(this.large ? "5 samples" : "10k samples").outlined()
        .onClick((_event, context) => { this.large = !this.large; context.notify(); }).build(cx))
      .child(panels);
    return new AppShell()
      .top(new TitleBar().brand(new Title("OmaVLESS · G1").build(cx))
        .actions(new Button("locale").label(this.locale === "ru" ? "EN" : "RU").outlined()
          .onClick((_event, context) => { this.locale = this.locale === "ru" ? "en" : "ru"; context.notify(); }).build(cx))
        .build(cx))
      .content(body)
      .build(cx);
  }
}

// @ts-check
// SPDX-License-Identifier: MIT
// Synthetic, read-only G1 comparison. No daemon/IPC, network or private data.
import { View } from "gpui-kit";
import { InputState, Scrollbar, VirtualListScrollHandle, h_flex, v_flex, v_virtual_list } from "gpui-base";
import { set_theme } from "gpui-base";
import {
  AppShell, Badge, Button, Label, ListRow, MutedText, Panel, TextField,
  Title, TitleBar, applyOmarchyRoles, applyOmarchyStyle, omarchyTheme,
} from "./vendor/omarchy-ui/src/index.js";
import { connectionPresentation, fixture, nextHighlightedProfile, profileById, ProfileCache } from "./data.js";
import { BROKEN, DARK, LIGHT } from "./palette.js";
import { UpdateView } from "./workload.js";
import { Metadata } from "./metadata.js";
import { plainKey, navigatePage } from "./keyboard.js";
import { prepareTheme, paletteText, ThemeUpdates } from "./theme-source.js";
import { manualTheme, followTheme } from "./theme-client.js";
const SHELL = "[font]\nbase-size = 12\n[spacing]\nscale = 1\nscale-with-font = true\n";

const copy = {
  en: {
    profiles: "Profiles", details: "Details", sources: "Subscriptions / sources", selected: "Selected for inspection",
    connected: "Confirmed connection", previous: "Previous server · not verified now", none: "None confirmed", invalid: "No valid selection",
    source: "Source",
    note: "Tab to list: arrows move, Enter inspects; Page Down opens Details", large: "10,005 synthetic rows · virtualized", noResults: "No matching profiles",
    readonly: "No VPN action exists in this experiment.", search: "Search sample profiles…",
    theme: "Synthetic theme", dark: "Dark", light: "Light", broken: "Broken → default", fallback: "Default palette restored",
    themeFollow: "Read public palette", themeFollowing: "Polling owned public sample", themeManual: "Manual palette · source paused",
    largeList: "Show generated samples", smallList: "Hide generated samples",
    viewDetails: "Details", viewProfiles: "Profiles", returnHint: "Page Up or Profiles returns to the list",
    burst: "Simulate 1,000 updates", lost: "Lose sample source", attach: "Read fresh sample", sample: "Sample update",
    renameSample: "Rename inspected sample", removeSample: "Remove inspected sample", replaceSample: "Replace sample collection", resetSample: "Restore fixture list", cached: "Cached sample list · source not verified", missingProfile: "not in current sample",
    phases: { connected: "Connected (synthetic)", connecting: "Connecting… (synthetic)", switching: "Switching server… (synthetic)", reconnecting: "Reconnecting… (synthetic)", unverified: "State unverified (synthetic)", failed: "Connection failed (synthetic)", recovery: "Recovery required (synthetic)", disconnected: "Disconnected (synthetic)" },
    scenes: { connected: "connected", connecting: "connecting", switching: "switching", reconnecting: "reconnecting", unverified: "unverified", failed: "failed", recovery: "recovery", removed: "removed" },
  },
  ru: {
    profiles: "Профили", details: "Детали", sources: "Подписки / источники", selected: "Выбрано для просмотра",
    connected: "Подтверждённое соединение", previous: "Прежний сервер · сейчас не подтверждён", none: "Нет подтверждённого соединения", invalid: "Нет выбранного профиля",
    source: "Источник",
    note: "Tab — к списку, стрелки — выбор, Enter — просмотр; Page Down — детали", large: "10 005 демонстрационных строк · виртуализация", noResults: "Профили не найдены",
    readonly: "В этом эксперименте нет управления VPN.", search: "Поиск демонстрационных профилей…",
    theme: "Тема макета", dark: "Тёмная", light: "Светлая", broken: "Сбой → стандартная", fallback: "Стандартная палитра восстановлена",
    themeFollow: "Читать публичную палитру", themeFollowing: "Чтение принадлежащего макету образца", themeManual: "Ручная палитра · источник приостановлен",
    largeList: "Показать сгенерированные строки", smallList: "Скрыть сгенерированные строки",
    viewDetails: "Детали", viewProfiles: "Профили", returnHint: "Page Up или «Профили» вернёт к списку",
    burst: "Смоделировать 1 000 обновлений", lost: "Потерять источник макета", attach: "Прочитать свежий макет", sample: "Обновление макета",
    renameSample: "Переименовать просматриваемый макет", removeSample: "Удалить просматриваемый макет", replaceSample: "Заменить состав макета подписки", resetSample: "Вернуть исходный список макета", cached: "Сохранённый список макета · источник не подтверждён", missingProfile: "нет в текущем макете",
    phases: { connected: "Подключено (макет)", connecting: "Подключаемся… (макет)", switching: "Меняем сервер… (макет)", reconnecting: "Переподключаемся… (макет)", unverified: "Состояние не подтверждено (макет)", failed: "Подключение не удалось (макет)", recovery: "Требуется восстановление (макет)", disconnected: "Отключено (макет)" },
    scenes: { connected: "подключено", connecting: "подключение", switching: "смена", reconnecting: "переподключение", unverified: "не проверено", failed: "сбой", recovery: "восстановление", removed: "удалён" },
  },
};

export default class G1Trial extends View {
  init(_props, cx) {
    const tokens = applyOmarchyStyle(SHELL, { cornerRadius: 0, fontFamily: "monospace" });
    this.baseTheme = cx.theme();
    this.themeTokens = tokens;
    this.themeChoice = "dark";
    this.themeFallback = false;
    this.themeUpdates = new ThemeUpdates();
    this.themeUpdates.receive(1, prepareTheme(DARK));
    this.themeUpdates.flush();
    applyOmarchyRoles(DARK);
    const theme = omarchyTheme(DARK, this.baseTheme, tokens);
    if (theme) set_theme(theme);
    this.search = InputState.new({ placeholder: "Search / Поиск" });
    this.search.on("change", (_event, context) => context.notify());
    this.search.on("submit", (_event, context) => {
      const first = this.profileCache.get(this.search.value(), this.large, this.collection, this.updates.metadata)[0];
      if (first) {
        this.selected = first.id;
        this.highlighted = first.id;
        this.listScroll.scroll_to_item(0);
      }
      context.notify();
    });
    this.panelFocus = cx.focus_handle();
    this.listFocus = cx.focus_handle();
    this.listScroll = VirtualListScrollHandle.new();
    this.panelFocus.focus();
    this.scene = 0;
    this.detailsOnly = false;
    this.selected = fixture.scenes[0].selected;
    this.highlighted = this.selected;
    this.locale = "en";
    this.large = false;
    this.collection = "all";
    this.profileCache = new ProfileCache();
    this.updates = new UpdateView(Metadata.fixture());
    this.updates.attach();
    this.updates.burst(fixture.scenes[0], 1);
  }

  applyPalette(choice, context) {
    manualTheme(this);
    const source = choice === "light" ? LIGHT : choice === "broken" ? BROKEN : DARK;
    this.themeChoice = choice;
    this.applyThemeSample(this.themeUpdates.sequence + 1, prepareTheme(source), context);
    context.notify();
  }

  applyThemeSample(sequence, prepared, context) {
    if (!this.themeUpdates.receive(sequence, prepared)) return;
    const delivery = this.themeUpdates.flush();
    if (delivery === null) return;
    if (delivery.paletteChanged) {
      const source = paletteText(delivery.sample.palette);
      const theme = omarchyTheme(source, this.baseTheme, this.themeTokens);
      if (!theme) return;
      set_theme(theme);
      applyOmarchyRoles(source);
    }
    this.themeFallback = delivery.sample.status !== "good";
    if (delivery.paletteChanged || delivery.statusChanged) context.notify();
  }

  replaceMetadata(action, context) {
    if (!this.updates.available() || !this.updates.metadataFresh) return;
    const metadata = this.updates.metadata;
    const rows = action === "rename" ? metadata.renamed(this.selected)
      : action === "remove" ? metadata.removed(this.selected)
      : action === "replace" ? metadata.replacedCollection() : Metadata.fixture().rows;
    if (rows !== null && this.updates.replace(fixture.scenes[this.scene], rows)) {
      this.selected = this.updates.metadata.selection(this.selected);
      context.notify();
    }
  }

  render(cx) {
    const strings = copy[this.locale];
    const scene = this.updates.state;
    const connection = connectionPresentation(scene);
    const metadata = this.updates.metadata;
    this.selected = metadata.selection(this.selected);
    const connected = profileById(connection.id, metadata);
    const connectedName = connected?.name ?? (connection.id === null ? strings.none : `${connection.id} · ${strings.missingProfile}`);
    const selected = profileById(this.selected, metadata);
    const query = this.search.value();
    const visible = this.profileCache.get(query, this.large, this.collection, metadata);
    if (!this.profileCache.ids.has(this.highlighted)) {
      this.highlighted = visible[0]?.id ?? null;
    }
    const colors = cx.theme().colors;
    const statusTone = scene.phase === "connected" ? "success"
      : ["failed", "recovery"].includes(scene.phase) ? "danger" : "warning";
    const scenes = h_flex().flex_wrap().gap(6).children(fixture.scenes.map((item, index) =>
      new Button(`scene-${item.id}`).label(strings.scenes[item.id]).outlined().selected(index === this.scene)
        .onClick((_event, context) => {
          this.scene = index;
          this.selected = item.selected;
          this.updates.attach();
          this.updates.burst(item, 1);
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
      .child(connection.kind === "confirmed" && profile.id === connection.id
        ? new Badge(`connected-${profile.id}`).label(this.locale === "ru" ? "Подключено" : "Connected").tone("success").build(cx)
        : new MutedText("").build(cx))
      .build(cx).h(52)
      .when(this.listFocus.is_focused() && this.highlighted === profile.id,
        (element) => element.border(1).border_color(colors.ring));

    const navigateList = (event, context) => {
      const key = plainKey(event);
      if (key === "down" || key === "up") {
        const id = nextHighlightedProfile(visible, this.highlighted, key === "down" ? 1 : -1);
        if (id !== null) {
          this.highlighted = id;
          this.listScroll.scroll_to_item(visible.findIndex((profile) => profile.id === id), "center");
          context.notify();
        }
        context.stop_propagation();
      } else if (key === "enter" && visible.some((profile) => profile.id === this.highlighted)) {
        this.selected = this.highlighted;
        context.notify();
        context.stop_propagation();
      } else if (key === "escape") {
        this.panelFocus.focus();
        context.stop_propagation();
      }
    };

    const list = v_flex().flex_1().min_h_0().min_w_0().gap(8).p(12)
      .child(new MutedText(strings.sources).build(cx))
      .child(collections)
      .child(new TextField().state(this.search).build(cx))
      .child(new MutedText(this.large
        ? (this.locale === "ru" ? `${visible.length} демонстрационных строк · виртуализация`
          : `${visible.length} synthetic rows · virtualized`) : strings.note).build(cx))
      .child(v_flex().relative().h("24rem").min_h_0().overflow_hidden()
        .track_focus(this.listFocus).tab_stop(true).on_key_down(navigateList)
        .focus((appearance) => appearance.border(1).border_color(colors.ring))
        .when(visible.length === 0, (element) => element.child(new MutedText(strings.noResults).build(cx).p(12)))
        .when(visible.length > 0, (element) => element
          .child(v_virtual_list("g1-profile-list", visible.length, 52,
            (index) => visible[index].id,
            (range) => Array.from({ length: range.end - range.start }, (_unused, offset) =>
              row(visible[range.start + offset])))
            .size_full().track_scroll(this.listScroll)
            .on_item_click((id, context) => { this.highlighted = id; this.selected = id; context.notify(); }))
          .child(Scrollbar.vertical("g1-profile-list").absolute().inset_0())));
    const details = v_flex().min_w_0().gap(12).p(12)
      .child(new MutedText(strings.selected).build(cx))
      .child(new Label(selected?.name ?? strings.invalid).build(cx))
      .child(new MutedText(strings.source).build(cx))
      .child(new Label(selected === null ? "—" : selected.subscription || (this.locale === "ru" ? "Локальный" : "Local")).build(cx))
      .child(new MutedText(connection.kind === "previous" ? strings.previous : strings.connected).build(cx))
      .child(new Label(connectedName).build(cx))
      .child(new MutedText(strings.readonly).build(cx));

    // Let native flex layout react to window geometry. A Shell view snapshot is
    // not rebuilt on resize, so a JS viewport breakpoint remains stale until
    // another action happens to refresh the view.
    // A wrapped Details panel must contribute to the outer scroll extent in
    // a short window. Flexing this row to the viewport clipped it below the
    // Profiles panel even though the body advertised vertical scrolling.
    const panels = this.detailsOnly ? null : h_flex().flex_none().flex_wrap()
      .items_start().min_w_0().gap(12)
      .child(v_flex().min_w_0().flex_basis("30rem").flex_grow(1)
        .child(new Panel("profiles").title(strings.profiles).content(list).build(cx).min_w_0()))
      .child(v_flex().min_w_0().flex_basis("30rem").flex_grow(1)
        .child(new Panel("details").title(strings.details).content(details).build(cx).min_w_0()));
    // The pinned Shell host does not expose a script-controlled ScrollHandle
    // for an ordinary outer scroll area. A compact read-only Details page is
    // the keyboard fallback; it is an explicit page transition, not scroll.
    const body = v_flex().size_full().min_h_0().p(18).gap(12)
      .track_focus(this.panelFocus)
      .overflow_y_scrollbar()
      .child(new Badge("state").label(strings.phases[scene.phase]).tone(statusTone).build(cx));
    if (!this.updates.metadataFresh) body.child(new MutedText(strings.cached).build(cx));
    if (this.detailsOnly) {
      body.child(new MutedText(strings.returnHint).build(cx))
        .child(new Panel("details-page").title(strings.details).content(details).build(cx).min_w_0());
    } else {
      body.child(scenes)
        .child(h_flex().flex_wrap().gap(6)
          .child(new Button("update-burst").label(strings.burst).outlined()
            .disabled(!this.updates.available())
            .onClick((_event, context) => { this.updates.burst(fixture.scenes[this.scene]); context.notify(); }).build(cx))
          .child(new Button("source-loss").label(strings.lost).outlined()
            .disabled(!this.updates.available())
            .onClick((_event, context) => { this.updates.lost(); context.notify(); }).build(cx))
          .child(new Button("source-attach").label(strings.attach).outlined()
            .onClick((_event, context) => {
              this.updates.attach(); this.updates.burst(fixture.scenes[this.scene], 1); context.notify();
            }).build(cx)))
        .child(new MutedText(`${strings.sample}: ${scene.sample}`).build(cx))
        .child(h_flex().flex_wrap().gap(6)
          .children([["rename", strings.renameSample], ["remove", strings.removeSample],
            ["replace", strings.replaceSample], ["reset", strings.resetSample]].map(([action, label]) =>
            new Button(`metadata-${action}`).label(label).outlined()
              .disabled(!this.updates.metadataFresh || !this.updates.available()
                || (["rename", "remove"].includes(action) && selected === null))
              .onClick((_event, context) => this.replaceMetadata(action, context)).build(cx))))
        .child(new Button("large-list").label(this.large ? strings.smallList : strings.largeList).outlined()
          .onClick((_event, context) => { this.large = !this.large; context.notify(); }).build(cx))
        .child(h_flex().flex_wrap().gap(6)
          .child(new MutedText(strings.theme).build(cx))
          .children([["dark", strings.dark], ["light", strings.light], ["broken", strings.broken]].map(([choice, label]) =>
            new Button(`theme-${choice}`).label(label).outlined().selected(this.themeChoice === choice)
              .onClick((_event, context) => this.applyPalette(choice, context)).build(cx))))
        .when(this.themeFallback, (element) => element.child(new MutedText(strings.fallback).build(cx)))
        .when(Boolean(this.themeFollow), (element) => element.child(h_flex().flex_wrap().gap(6)
          .child(new Button("theme-follow").label(strings.themeFollow).outlined()
            .selected(this.themeFollow.following)
            .onClick((_event, context) => followTheme(this, context)).build(cx))
          .child(new MutedText(this.themeFollow.following ? strings.themeFollowing : strings.themeManual).build(cx))))
        .child(panels);
    }
    return new AppShell()
      .top(new TitleBar().brand(new Title("OmaVLESS · G1").build(cx))
        .actions(h_flex().gap(6)
          .child(new Button("view-toggle")
            .label(this.detailsOnly ? strings.viewProfiles : strings.viewDetails).outlined()
            .onClick((_event, context) => {
              this.detailsOnly = !this.detailsOnly;
              this.panelFocus.focus();
              context.notify();
            }).build(cx))
          .child(new Button("locale").label(this.locale === "ru" ? "EN" : "RU").outlined()
            .onClick((_event, context) => { this.locale = this.locale === "ru" ? "en" : "ru"; context.notify(); }).build(cx)))
        .build(cx))
      .content(body)
      .build(cx)
      .on_key_down((event, context) => navigatePage(event, this, context));
  }
}

// SPDX-License-Identifier: MIT
//! Synthetic, read-only G1 comparison client. No OmaVLESS IPC or credentials.

use gpui_kit::base::input::{InputEvent, InputState};
use gpui_kit::{
    AnyElement, Context, Entity, IntoElement, ParentElement, Pixels, Render, Size, Styled, Window,
    WindowOptions, div, prelude::*, rems,
};
use gpui_omarchy::{ActiveTheme, ButtonVariant, button, focus_scope, input, panel, virtual_list};
use serde::Deserialize;
use std::rc::Rc;

#[derive(Clone, Deserialize)]
struct Profile {
    id: String,
    name: String,
    country: String,
    host: String,
    subscription: String,
}

#[derive(Clone, Deserialize)]
struct Scene {
    id: String,
    phase: String,
    selected: Option<String>,
    connected: Option<String>,
}

#[derive(Clone, Deserialize)]
struct Collection {
    id: String,
    name: String,
}

#[derive(Deserialize)]
struct Fixtures {
    schema: u32,
    profiles: Vec<Profile>,
    collections: Vec<Collection>,
    scenes: Vec<Scene>,
    large_list_count: usize,
}

impl Fixtures {
    fn load() -> Self {
        let data: Self = serde_json::from_str(include_str!("../../fixtures.json"))
            .expect("synthetic G1 fixtures must be valid");
        assert_eq!(data.schema, 1);
        data
    }

    fn profile(&self, id: &str) -> Option<&Profile> {
        self.profiles.iter().find(|profile| profile.id == id)
    }
}

struct Trial {
    fixtures: Fixtures,
    search: Entity<InputState>,
    scene: usize,
    selected: Option<String>,
    russian: bool,
    large: bool,
    collection: usize,
    cached_query: String,
    cached_large: bool,
    cached_collection: usize,
    visible: Rc<Vec<Profile>>,
    row_sizes: Rc<Vec<Size<Pixels>>>,
    row_rem_size: Pixels,
}

impl Trial {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let fixtures = Fixtures::load();
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search / Поиск"));
        cx.observe(&search, |_, _, cx| cx.notify()).detach();
        cx.subscribe(&search, |this, input, event: &InputEvent, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                let query = input.read(cx).value().to_string();
                this.selected = this
                    .visible_profiles(&query)
                    .first()
                    .map(|profile| profile.id.clone());
                cx.notify();
            }
        })
        .detach();
        let selected = fixtures.scenes[0].selected.clone();
        Self {
            visible: Rc::new(fixtures.profiles.clone()),
            fixtures,
            search,
            scene: 0,
            selected,
            russian: false,
            large: false,
            collection: 0,
            cached_query: String::new(),
            cached_large: false,
            cached_collection: 0,
            row_sizes: Rc::new(Vec::new()),
            row_rem_size: gpui_kit::px(0.),
        }
    }

    fn label<'a>(&self, english: &'a str, russian: &'a str) -> &'a str {
        if self.russian { russian } else { english }
    }

    fn scene(&self) -> &Scene {
        &self.fixtures.scenes[self.scene]
    }

    fn scene_label(&self, id: &str) -> &'static str {
        match (id, self.russian) {
            ("connected", false) => "connected",
            ("connected", true) => "подключено",
            ("connecting", false) => "connecting",
            ("connecting", true) => "подключение",
            ("switching", false) => "switching",
            ("switching", true) => "смена",
            ("reconnecting", false) => "reconnecting",
            ("reconnecting", true) => "переподключение",
            ("unverified", false) => "unverified",
            ("unverified", true) => "не проверено",
            ("failed", false) => "failed",
            ("failed", true) => "сбой",
            ("recovery", false) => "recovery",
            ("recovery", true) => "восстановление",
            ("removed", false) => "removed",
            ("removed", true) => "удалён",
            _ => "unknown",
        }
    }

    fn collection_label(&self, collection: &Collection) -> String {
        if collection.id == "all" {
            self.label("All", "Все").to_owned()
        } else if collection.id == "local" {
            self.label("Local", "Локальные").to_owned()
        } else {
            collection.name.clone()
        }
    }

    fn visible_profiles(&self, query: &str) -> Vec<Profile> {
        let query = query.to_lowercase();
        let collection = self.fixtures.collections[self.collection].id.as_str();
        let mut profiles = self.fixtures.profiles.clone();
        if self.large {
            profiles.extend((0..self.fixtures.large_list_count).map(|index| Profile {
                id: format!("generated-{index:05}"),
                name: format!("Synthetic node {index:05}"),
                country: "Example".into(),
                host: format!("node-{index:05}.example"),
                subscription: "Generated samples".into(),
            }));
        }
        profiles
            .into_iter()
            .filter(|profile| {
                let in_collection = match collection {
                    "all" => true,
                    "sample" => profile.subscription == "Sample collection",
                    "generated" => profile.subscription == "Generated samples",
                    "local" => profile.subscription.is_empty(),
                    _ => false,
                };
                in_collection
                    && [
                        &profile.name,
                        &profile.country,
                        &profile.host,
                        &profile.subscription,
                    ]
                    .iter()
                    .any(|value| value.to_lowercase().contains(&query))
            })
            .collect()
    }

    fn update_visible(&mut self, query: &str, window: &Window) {
        let changed = self.cached_query != query
            || self.cached_large != self.large
            || self.cached_collection != self.collection;
        if changed {
            self.visible = Rc::new(self.visible_profiles(query));
            self.cached_query = query.to_owned();
            self.cached_large = self.large;
            self.cached_collection = self.collection;
        }
        if changed
            || self.row_rem_size != window.rem_size()
            || self.row_sizes.len() != self.visible.len()
        {
            let width = rems(36.).to_pixels(window.rem_size());
            let height = rems(3.5).to_pixels(window.rem_size());
            self.row_sizes = Rc::new(
                (0..self.visible.len())
                    .map(|_| gpui_kit::size(width, height))
                    .collect(),
            );
            self.row_rem_size = window.rem_size();
        }
    }

    fn render_profile(
        &mut self,
        profile: Profile,
        connected: Option<&str>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected = self.selected.as_deref() == Some(profile.id.as_str());
        let is_connected = connected == Some(profile.id.as_str());
        let theme = cx.omarchy().clone();
        let id = profile.id.clone();
        let marker = if is_connected {
            self.label("Connected", "Подключено")
        } else {
            ""
        };
        div()
            .id(format!("profile-{id}"))
            .flex()
            .items_center()
            .justify_between()
            .gap(rems(0.75))
            .min_w_0()
            .w_full()
            .h(rems(3.5))
            .p(rems(0.625))
            .border_1()
            .border_color(if selected { theme.accent } else { theme.border })
            .bg(if selected {
                theme.selection
            } else {
                theme.background
            })
            .hover(|style| style.bg(theme.surface))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.selected = Some(id.clone());
                cx.notify();
            }))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_color(theme.foreground)
                            .child(profile.name),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_color(theme.secondary)
                            .text_size(rems(0.6875))
                            .child(profile.host),
                    ),
            )
            .child(
                div()
                    .text_color(theme.success)
                    .text_size(rems(0.6875))
                    .child(marker),
            )
            .into_any_element()
    }
}

impl Render for Trial {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.omarchy().clone();
        let scene = self.scene().clone();
        let query = self.search.read(cx).value().to_string();
        self.update_visible(&query, window);
        let visible = self.visible.clone();
        let selected = self
            .selected
            .as_deref()
            .and_then(|id| self.fixtures.profile(id))
            .cloned();
        let connected = scene
            .connected
            .as_deref()
            .and_then(|id| self.fixtures.profile(id))
            .cloned();
        let status = match scene.phase.as_str() {
            "connected" => self.label("Connected (synthetic)", "Подключено (макет)"),
            "connecting" => self.label("Connecting… (synthetic)", "Подключаемся… (макет)"),
            "switching" => self.label("Switching server… (synthetic)", "Меняем сервер… (макет)"),
            "reconnecting" => self.label("Reconnecting… (synthetic)", "Переподключаемся… (макет)"),
            "unverified" => self.label(
                "State unverified (synthetic)",
                "Состояние не подтверждено (макет)",
            ),
            "failed" => self.label(
                "Connection failed (synthetic)",
                "Подключение не удалось (макет)",
            ),
            "recovery" => self.label(
                "Recovery required (synthetic)",
                "Требуется восстановление (макет)",
            ),
            _ => self.label("Disconnected (synthetic)", "Отключено (макет)"),
        };
        let status_color = match scene.phase.as_str() {
            "connected" => theme.success,
            "connecting" | "switching" | "reconnecting" => theme.warning,
            "unverified" | "failed" | "recovery" => theme.danger,
            _ => theme.secondary,
        };
        let mut scene_buttons = Vec::<AnyElement>::new();
        for index in 0..self.fixtures.scenes.len() {
            let id = self.fixtures.scenes[index].id.clone();
            scene_buttons.push(
                button(
                    format!("scene-{id}"),
                    self.scene_label(&id),
                    if self.scene == index {
                        ButtonVariant::Primary
                    } else {
                        ButtonVariant::Outline
                    },
                    cx,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.scene = index;
                    this.selected = this.fixtures.scenes[index].selected.clone();
                    cx.notify();
                }))
                .into_any_element(),
            );
        }
        let mut collection_buttons = Vec::<AnyElement>::new();
        for index in 0..self.fixtures.collections.len() {
            let collection = &self.fixtures.collections[index];
            collection_buttons.push(
                button(
                    format!("collection-{}", collection.id),
                    self.collection_label(collection),
                    if self.collection == index {
                        ButtonVariant::Primary
                    } else {
                        ButtonVariant::Outline
                    },
                    cx,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.collection = index;
                    cx.notify();
                }))
                .into_any_element(),
            );
        }
        let list_note = if self.large {
            if self.russian {
                format!(
                    "{} демонстрационных строк · виртуализация",
                    self.visible.len()
                )
            } else {
                format!("{} synthetic rows · virtualized", self.visible.len())
            }
        } else {
            self.label(
                "Click a row or press Enter in search to inspect; never connects",
                "Нажмите строку или Enter в поиске для просмотра; подключения нет",
            )
            .to_owned()
        };
        let selected_name = selected
            .as_ref()
            .map(|profile| profile.name.as_str())
            .unwrap_or(self.label("No valid selection", "Нет выбранного профиля"));
        let selected_source = selected
            .as_ref()
            .map(|profile| {
                if profile.subscription.is_empty() {
                    self.label("Local", "Локальный")
                } else {
                    profile.subscription.as_str()
                }
            })
            .unwrap_or("—");
        let connected_name = connected
            .as_ref()
            .map(|profile| profile.name.as_str())
            .unwrap_or(self.label("None confirmed", "Нет подтверждённого соединения"));
        let narrow = window.bounds().size.width < rems(60.).to_pixels(window.rem_size());
        let viewport_rems = window.viewport_size().height.as_f32() / window.rem_size().as_f32();
        let list_height = rems((viewport_rems - 27.).clamp(8., 24.));
        let connection_id = scene.connected.clone();
        let list = virtual_list(
            cx.entity(),
            "g1-profile-list",
            self.row_sizes.clone(),
            move |this, range, _, cx| {
                range
                    .map(|index| {
                        this.render_profile(visible[index].clone(), connection_id.as_deref(), cx)
                    })
                    .collect::<Vec<_>>()
            },
            cx,
        );
        let profile_panel = panel(self.label("Profiles", "Профили"), cx)
            .min_w(rems(16.))
            .flex_1()
            .child(
                div()
                    .text_color(theme.secondary)
                    .child(self.label("Subscriptions / sources", "Подписки / источники")),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(rems(0.35))
                    .children(collection_buttons),
            )
            .child(input("g1-search", &self.search, window, cx))
            .child(div().text_color(theme.secondary).child(list_note))
            .child(list.h(list_height).min_h_0());
        let details = panel(self.label("Details", "Детали"), cx)
            .min_w(rems(16.))
            .flex_1()
            .child(
                div()
                    .text_color(theme.secondary)
                    .child(self.label("Selected for inspection", "Выбрано для просмотра")),
            )
            .child(div().min_w_0().truncate().child(selected_name.to_owned()))
            .child(
                div()
                    .text_color(theme.secondary)
                    .child(self.label("Source", "Источник")),
            )
            .child(div().min_w_0().truncate().child(selected_source.to_owned()))
            .child(
                div()
                    .text_color(theme.secondary)
                    .child(self.label("Confirmed connection", "Подтверждённое соединение")),
            )
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_color(theme.success)
                    .child(connected_name.to_owned()),
            )
            .child(div().text_color(theme.secondary).child(self.label(
                "No VPN action exists in this experiment.",
                "В этом эксперименте нет управления VPN.",
            )));
        focus_scope("g1-trial")
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .font_family(theme.font.clone())
            .p(rems(1.25))
            .gap(rems(0.75))
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(rems(0.5))
                    .child(div().text_size(rems(1.25)).child("OmaVLESS · G1"))
                    .child(
                        button(
                            "locale",
                            if self.russian { "EN" } else { "RU" },
                            ButtonVariant::Outline,
                            cx,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.russian = !this.russian;
                            cx.notify();
                        })),
                    ),
            )
            .child(div().text_color(status_color).child(status))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(rems(0.4))
                    .children(scene_buttons),
            )
            .child(
                button(
                    "large",
                    if self.large {
                        "5 samples"
                    } else {
                        "10k samples"
                    },
                    ButtonVariant::Outline,
                    cx,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.large = !this.large;
                    cx.notify();
                })),
            )
            .child(
                div()
                    .id("g1-panels")
                    .flex()
                    .when(narrow, |layout| layout.flex_col().overflow_y_scroll())
                    .when(!narrow, |layout| layout.flex_row())
                    .min_h_0()
                    .flex_1()
                    .gap(rems(0.75))
                    .child(profile_panel)
                    .child(details),
            )
    }
}

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_omarchy::init(cx);
            cx.open_window(WindowOptions::default(), |window, cx| {
                cx.new(|cx| Trial::new(window, cx))
            })
            .expect("open synthetic G1 window");
            cx.activate(true);
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_and_connection_are_distinct_fixture_facts() {
        let fixture = Fixtures::load();
        assert_ne!(fixture.scenes[0].selected, fixture.scenes[0].connected);
        assert!(
            fixture
                .profile(fixture.scenes[0].selected.as_deref().unwrap())
                .is_some()
        );
        assert!(
            fixture
                .profile(fixture.scenes[0].connected.as_deref().unwrap())
                .is_some()
        );
        let removed = fixture
            .scenes
            .iter()
            .find(|scene| scene.id == "removed")
            .unwrap();
        assert!(
            fixture
                .profile(removed.selected.as_deref().unwrap())
                .is_none()
        );
        assert!(fixture.scenes.iter().any(|scene| scene.phase == "recovery"));
        assert_eq!(fixture.collections.len(), 4);
        assert!(fixture.large_list_count >= 10_000);
    }
}

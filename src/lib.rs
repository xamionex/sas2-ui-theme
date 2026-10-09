//! Shared UI theming for the Salt and Sacrifice tools.
//!
//! Provides a serde-serializable [`Theme`] of color overrides layered on top of egui's dark/light presets,
//! the "UI Theme" editor window (Settings -> UI Theme) and a swatch/color-picker widget used to edit individual colors.
//!
//! The color picker is a small menu with a saturation/value square, a vertical alpha bar, hex and RGBA fields, and hue/saturation/value gradient sliders.

use egui::ecolor::Hsva;
use egui::{
    Align, Color32, Context, DragValue, Layout, Mesh, Painter, Popup, PopupCloseBehavior, Rect,
    Response, Rgba, Sense, Shape, Slider, Stroke, StrokeKind, Ui, Vec2, Visuals, lerp, pos2,
    remap_clamp, vec2,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Vertices per axis for the gradient pickers. A multiple of 6 so hue peaks (every 60 degrees) are hit exactly.
const GRADIENT_STEPS: u32 = 6 * 6;

/// Fixed width of the color editor popup. Without it the gradient sliders stretch to the width of the whole window the popup is anchored in.
const POPUP_WIDTH: f32 = 250.0;

/// Size of the little preview swatch shown at the top of the picker.
const PREVIEW_SIZE: f32 = 40.0;

/// Side length of the saturation/value square picker.
const SV_SIZE: f32 = 200.0;

/// Width of the vertical alpha bar next to the saturation/value square.
const ALPHA_BAR_WIDTH: f32 = 22.0;

/// Width of a single RGBA channel field.
const RGBA_FIELD_WIDTH: f32 = 44.0;

/// Radius of the round handle in the saturation/value square.
/// Kept small so it does not poke out of the popup when the color sits in a corner (for example egui's additive faint background).
const SV_HANDLE_RADIUS: f32 = 5.0;

/// A color that serializes as premultiplied sRGBA bytes (`[r, g, b, a]`).
///
/// egui stores colors premultiplied, so the raw bytes are serialized to round-trip every color exactly (including additive colors, whose alpha is 0 but whose RGB still matters).
/// The editor shows and accepts colors as ordinary unmultiplied `#RRGGBBAA`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThemeColor(pub Color32);

impl From<Color32> for ThemeColor {
    fn from(value: Color32) -> Self {
        Self(value)
    }
}

impl From<ThemeColor> for Color32 {
    fn from(value: ThemeColor) -> Self {
        value.0
    }
}

impl Serialize for ThemeColor {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.to_array().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ThemeColor {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let [r, g, b, a] = <[u8; 4]>::deserialize(deserializer)?;
        Ok(Self(Color32::from_rgba_premultiplied(r, g, b, a)))
    }
}

/// The four colors of one widget interaction state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WidgetColors {
    /// Background of widgets that must have one (slider track, checkbox box, ...).
    pub bg_fill: ThemeColor,
    /// Background of widgets that may be transparent (buttons, ...).
    pub weak_bg_fill: ThemeColor,
    /// Border/stroke color.
    pub bg_stroke: ThemeColor,
    /// Foreground/text color of the interactive part.
    pub fg_stroke: ThemeColor,
}

impl Default for WidgetColors {
    fn default() -> Self {
        Self::from_visuals(&Visuals::dark().widgets.inactive)
    }
}

impl WidgetColors {
    fn from_visuals(w: &egui::style::WidgetVisuals) -> Self {
        Self {
            bg_fill: w.bg_fill.into(),
            weak_bg_fill: w.weak_bg_fill.into(),
            bg_stroke: w.bg_stroke.color.into(),
            fg_stroke: w.fg_stroke.color.into(),
        }
    }

    fn apply(&self, w: &mut egui::style::WidgetVisuals) {
        w.bg_fill = self.bg_fill.0;
        w.weak_bg_fill = self.weak_bg_fill.0;
        w.bg_stroke.color = self.bg_stroke.0;
        w.fg_stroke.color = self.fg_stroke.0;
    }
}

/// A full UI theme.
///
/// Non-color properties (corner radii, shadow geometry, spacing, stroke widths, ...) come from the selected base preset, only the colors below are overridden.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Theme {
    /// Base preset: dark (true) or light (false).
    pub dark_mode: bool,

    // Text
    pub override_text_color: Option<ThemeColor>,
    pub weak_text_color: Option<ThemeColor>,
    pub hyperlink_color: ThemeColor,
    pub warn_fg_color: ThemeColor,
    pub error_fg_color: ThemeColor,

    // Backgrounds
    pub window_fill: ThemeColor,
    pub panel_fill: ThemeColor,
    pub faint_bg_color: ThemeColor,
    pub extreme_bg_color: ThemeColor,
    pub code_bg_color: ThemeColor,
    pub text_edit_bg_color: Option<ThemeColor>,

    // Widgets
    pub noninteractive: WidgetColors,
    pub inactive: WidgetColors,
    pub hovered: WidgetColors,
    pub active: WidgetColors,
    pub open: WidgetColors,

    // Selection
    pub selection_bg_fill: ThemeColor,
    pub selection_stroke: ThemeColor,

    // Window chrome
    pub window_stroke: ThemeColor,
    pub window_shadow: ThemeColor,
    pub popup_shadow: ThemeColor,
    pub text_cursor: ThemeColor,

    // Misc
    pub disabled_alpha: f32,
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

impl Theme {
    pub fn dark() -> Self {
        Self::from_visuals(&Visuals::dark())
    }

    pub fn light() -> Self {
        Self::from_visuals(&Visuals::light())
    }

    /// Capture every themed value from an existing [`Visuals`].
    pub fn from_visuals(v: &Visuals) -> Self {
        Self {
            dark_mode: v.dark_mode,
            override_text_color: v.override_text_color.map(ThemeColor::from),
            weak_text_color: v.weak_text_color.map(ThemeColor::from),
            hyperlink_color: v.hyperlink_color.into(),
            warn_fg_color: v.warn_fg_color.into(),
            error_fg_color: v.error_fg_color.into(),
            window_fill: v.window_fill.into(),
            panel_fill: v.panel_fill.into(),
            faint_bg_color: v.faint_bg_color.into(),
            extreme_bg_color: v.extreme_bg_color.into(),
            code_bg_color: v.code_bg_color.into(),
            text_edit_bg_color: v.text_edit_bg_color.map(ThemeColor::from),
            noninteractive: WidgetColors::from_visuals(&v.widgets.noninteractive),
            inactive: WidgetColors::from_visuals(&v.widgets.inactive),
            hovered: WidgetColors::from_visuals(&v.widgets.hovered),
            active: WidgetColors::from_visuals(&v.widgets.active),
            open: WidgetColors::from_visuals(&v.widgets.open),
            selection_bg_fill: v.selection.bg_fill.into(),
            selection_stroke: v.selection.stroke.color.into(),
            window_stroke: v.window_stroke.color.into(),
            window_shadow: v.window_shadow.color.into(),
            popup_shadow: v.popup_shadow.color.into(),
            text_cursor: v.text_cursor.stroke.color.into(),
            disabled_alpha: v.disabled_alpha,
        }
    }

    /// Build [`Visuals`] from the base preset with this theme's colors applied.
    pub fn to_visuals(&self) -> Visuals {
        let mut v = if self.dark_mode {
            Visuals::dark()
        } else {
            Visuals::light()
        };
        v.dark_mode = self.dark_mode;
        v.override_text_color = self.override_text_color.map(|c| c.0);
        v.weak_text_color = self.weak_text_color.map(|c| c.0);
        v.hyperlink_color = self.hyperlink_color.0;
        v.warn_fg_color = self.warn_fg_color.0;
        v.error_fg_color = self.error_fg_color.0;
        v.window_fill = self.window_fill.0;
        v.panel_fill = self.panel_fill.0;
        v.faint_bg_color = self.faint_bg_color.0;
        v.extreme_bg_color = self.extreme_bg_color.0;
        v.code_bg_color = self.code_bg_color.0;
        v.text_edit_bg_color = self.text_edit_bg_color.map(|c| c.0);
        v.window_stroke.color = self.window_stroke.0;
        v.window_shadow.color = self.window_shadow.0;
        v.popup_shadow.color = self.popup_shadow.0;
        v.selection.bg_fill = self.selection_bg_fill.0;
        v.selection.stroke.color = self.selection_stroke.0;
        v.text_cursor.stroke.color = self.text_cursor.0;
        v.disabled_alpha = self.disabled_alpha;
        self.noninteractive.apply(&mut v.widgets.noninteractive);
        self.inactive.apply(&mut v.widgets.inactive);
        self.hovered.apply(&mut v.widgets.hovered);
        self.active.apply(&mut v.widgets.active);
        self.open.apply(&mut v.widgets.open);
        v
    }

    /// Apply this theme to the context, pinning the theme preference so the system theme does not override it.
    pub fn apply(&self, ctx: &Context) {
        let visuals = self.to_visuals();
        let theme = if self.dark_mode {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        };
        ctx.set_theme(theme);
        ctx.set_visuals_of(theme, visuals);
    }

    /// Restore egui's built-in dark/light visuals and follow the system theme again.
    pub fn reset_context(ctx: &Context) {
        ctx.set_visuals_of(egui::Theme::Dark, Visuals::dark());
        ctx.set_visuals_of(egui::Theme::Light, Visuals::light());
        ctx.set_theme(egui::ThemePreference::System);
    }
}

/// What the theme window did this frame.
#[derive(Clone, Copy, Default)]
pub struct ThemeResponse {
    /// A color or the base preset changed; the theme should be marked customized and saved.
    pub changed: bool,
    /// The user asked to stop overriding colors and follow the system theme again.
    pub reset_default: bool,
}

/// Show the "UI Theme" editor window.
/// `open` is the window's visibility flag.
pub fn show_theme_window(ctx: &Context, open: &mut bool, theme: &mut Theme) -> ThemeResponse {
    let mut response = ThemeResponse::default();
    if !*open {
        return response;
    }

    egui::Window::new("UI Theme")
        .open(open)
        .default_width(460.0)
        .collapsible(false)
        .resizable(true)
        .show(ctx, |ui| {
            response = theme_ui(ui, theme);
        });

    response
}

fn theme_ui(ui: &mut Ui, theme: &mut Theme) -> ThemeResponse {
    let mut changed = false;
    let mut reset_default = false;

    ui.spacing_mut().item_spacing = vec2(8.0, 6.0);

    ui.horizontal_wrapped(|ui| {
        ui.label("Base theme:");
        if ui.selectable_label(theme.dark_mode, "Dark").clicked() && !theme.dark_mode {
            *theme = Theme::dark();
            changed = true;
        }
        if ui.selectable_label(!theme.dark_mode, "Light").clicked() && theme.dark_mode {
            *theme = Theme::light();
            changed = true;
        }
        if ui
            .button("Reset colors")
            .on_hover_text("Reset every color to the base theme")
            .clicked()
        {
            *theme = if theme.dark_mode {
                Theme::dark()
            } else {
                Theme::light()
            };
            changed = true;
        }
        if ui
            .button("Use system default")
            .on_hover_text("Stop overriding colors and follow the system light/dark theme")
            .clicked()
        {
            reset_default = true;
        }
    });

    let base = if theme.dark_mode {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    let defaults = Theme::from_visuals(&base);

    ui.separator();

    // Use non-floating scroll bars so the swatches are not drawn underneath the scroll handle.
    ui.style_mut().spacing.scroll.floating = false;

    egui::ScrollArea::vertical()
        .max_height(560.0)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::CollapsingHeader::new("Text")
                .default_open(true)
                .show(ui, |ui| {
                    optional_color_row(
                        ui,
                        "Text color",
                        "override_text_color",
                        &mut theme.override_text_color,
                        defaults.noninteractive.fg_stroke.0,
                        &mut changed,
                    );
                    optional_color_row(
                        ui,
                        "Weak text color",
                        "weak_text_color",
                        &mut theme.weak_text_color,
                        defaults
                            .weak_text_color
                            .map(|c| c.0)
                            .unwrap_or(defaults.noninteractive.fg_stroke.0),
                        &mut changed,
                    );
                    color_row(
                        ui,
                        "Hyperlink",
                        "hyperlink_color",
                        &mut theme.hyperlink_color,
                        defaults.hyperlink_color.0,
                        &mut changed,
                    );
                    color_row(
                        ui,
                        "Warning text",
                        "warn_fg_color",
                        &mut theme.warn_fg_color,
                        defaults.warn_fg_color.0,
                        &mut changed,
                    );
                    color_row(
                        ui,
                        "Error text",
                        "error_fg_color",
                        &mut theme.error_fg_color,
                        defaults.error_fg_color.0,
                        &mut changed,
                    );
                });

            egui::CollapsingHeader::new("Backgrounds")
                .default_open(true)
                .show(ui, |ui| {
                    color_row(
                        ui,
                        "Window",
                        "window_fill",
                        &mut theme.window_fill,
                        defaults.window_fill.0,
                        &mut changed,
                    );
                    color_row(
                        ui,
                        "Panel",
                        "panel_fill",
                        &mut theme.panel_fill,
                        defaults.panel_fill.0,
                        &mut changed,
                    );
                    color_row(
                        ui,
                        "Faint background",
                        "faint_bg_color",
                        &mut theme.faint_bg_color,
                        defaults.faint_bg_color.0,
                        &mut changed,
                    );
                    color_row(
                        ui,
                        "Extreme background",
                        "extreme_bg_color",
                        &mut theme.extreme_bg_color,
                        defaults.extreme_bg_color.0,
                        &mut changed,
                    );
                    color_row(
                        ui,
                        "Code background",
                        "code_bg_color",
                        &mut theme.code_bg_color,
                        defaults.code_bg_color.0,
                        &mut changed,
                    );
                    optional_color_row(
                        ui,
                        "Text edit background",
                        "text_edit_bg_color",
                        &mut theme.text_edit_bg_color,
                        defaults.extreme_bg_color.0,
                        &mut changed,
                    );
                });

            egui::CollapsingHeader::new("Selection")
                .default_open(true)
                .show(ui, |ui| {
                    color_row(
                        ui,
                        "Background",
                        "selection_bg_fill",
                        &mut theme.selection_bg_fill,
                        defaults.selection_bg_fill.0,
                        &mut changed,
                    );
                    color_row(
                        ui,
                        "Text",
                        "selection_stroke",
                        &mut theme.selection_stroke,
                        defaults.selection_stroke.0,
                        &mut changed,
                    );
                });

            egui::CollapsingHeader::new("Window")
                .default_open(false)
                .show(ui, |ui| {
                    color_row(
                        ui,
                        "Border",
                        "window_stroke",
                        &mut theme.window_stroke,
                        defaults.window_stroke.0,
                        &mut changed,
                    );
                    color_row(
                        ui,
                        "Shadow",
                        "window_shadow",
                        &mut theme.window_shadow,
                        defaults.window_shadow.0,
                        &mut changed,
                    );
                    color_row(
                        ui,
                        "Popup shadow",
                        "popup_shadow",
                        &mut theme.popup_shadow,
                        defaults.popup_shadow.0,
                        &mut changed,
                    );
                    color_row(
                        ui,
                        "Text cursor",
                        "text_cursor",
                        &mut theme.text_cursor,
                        defaults.text_cursor.0,
                        &mut changed,
                    );
                });

            egui::CollapsingHeader::new("Widgets")
                .default_open(false)
                .show(ui, |ui| {
                    widget_colors_ui(
                        ui,
                        "Non-interactive (labels, panels)",
                        "noninteractive",
                        &mut theme.noninteractive,
                        &defaults.noninteractive,
                        &mut changed,
                    );
                    widget_colors_ui(
                        ui,
                        "Inactive (buttons at rest)",
                        "inactive",
                        &mut theme.inactive,
                        &defaults.inactive,
                        &mut changed,
                    );
                    widget_colors_ui(
                        ui,
                        "Hovered",
                        "hovered",
                        &mut theme.hovered,
                        &defaults.hovered,
                        &mut changed,
                    );
                    widget_colors_ui(
                        ui,
                        "Active (pressed)",
                        "active",
                        &mut theme.active,
                        &defaults.active,
                        &mut changed,
                    );
                    widget_colors_ui(
                        ui,
                        "Open (open menus)",
                        "open",
                        &mut theme.open,
                        &defaults.open,
                        &mut changed,
                    );
                });

            egui::CollapsingHeader::new("Misc")
                .default_open(false)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Disabled alpha");
                        let mut alpha = theme.disabled_alpha;
                        if ui
                            .add(Slider::new(&mut alpha, 0.0..=1.0).fixed_decimals(2))
                            .changed()
                        {
                            theme.disabled_alpha = alpha;
                            changed = true;
                        }
                        if ui.button("Reset").clicked() {
                            theme.disabled_alpha = defaults.disabled_alpha;
                            changed = true;
                        }
                    });
                });
        });

    ThemeResponse {
        changed,
        reset_default,
    }
}

fn widget_colors_ui(
    ui: &mut Ui,
    title: &str,
    key: &str,
    colors: &mut WidgetColors,
    defaults: &WidgetColors,
    changed: &mut bool,
) {
    egui::CollapsingHeader::new(title)
        .default_open(false)
        .id_salt(key)
        .show(ui, |ui| {
            color_row(
                ui,
                "Background",
                &format!("{key}_bg"),
                &mut colors.bg_fill,
                defaults.bg_fill.0,
                changed,
            );
            color_row(
                ui,
                "Weak background",
                &format!("{key}_weak_bg"),
                &mut colors.weak_bg_fill,
                defaults.weak_bg_fill.0,
                changed,
            );
            color_row(
                ui,
                "Border",
                &format!("{key}_border"),
                &mut colors.bg_stroke,
                defaults.bg_stroke.0,
                changed,
            );
            color_row(
                ui,
                "Foreground/text",
                &format!("{key}_fg"),
                &mut colors.fg_stroke,
                defaults.fg_stroke.0,
                changed,
            );
        });
}

/// A label plus a right-aligned color swatch and a per-color Reset button.
fn color_row(
    ui: &mut Ui,
    label: &str,
    key: &str,
    color: &mut ThemeColor,
    default: Color32,
    changed: &mut bool,
) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if color_swatch(ui, key, &mut color.0) {
                *changed = true;
            }
            if ui
                .small_button("Reset")
                .on_hover_text("Reset this color to the base theme value")
                .clicked()
            {
                *color = ThemeColor(default);
                *changed = true;
            }
        });
    });
}

/// A checkbox label plus a right-aligned swatch, for colors that can be left unset (egui falls back to a derived value when the override is `None`).
fn optional_color_row(
    ui: &mut Ui,
    label: &str,
    key: &str,
    color: &mut Option<ThemeColor>,
    default: Color32,
    changed: &mut bool,
) {
    ui.horizontal(|ui| {
        let mut enabled = color.is_some();
        if ui.checkbox(&mut enabled, label).changed() {
            *color = if enabled {
                Some(ThemeColor(default))
            } else {
                None
            };
            *changed = true;
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            match color.as_mut() {
                Some(value) => {
                    if color_swatch(ui, key, &mut value.0) {
                        *changed = true;
                    }
                }
                None => color_swatch_disabled(ui, default),
            }
        });
    });
}

fn swatch_size(ui: &Ui) -> Vec2 {
    let height = ui.spacing().interact_size.y;
    Vec2::new((height * 1.7).round(), height)
}

/// Draw a checkerboard (so alpha is visible) topped with the color.
fn paint_swatch(ui: &Ui, rect: Rect, color: Color32) {
    paint_color_at(ui.painter(), color, rect);
    let visuals = ui.visuals();
    ui.painter().rect_stroke(
        rect,
        3.0,
        Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color),
        StrokeKind::Inside,
    );
}

/// Paint a color over a checkerboard so transparency is visible.
fn paint_color_at(painter: &Painter, color: Color32, rect: Rect) {
    if color.is_opaque() {
        painter.rect_filled(rect, 3.0, color);
    } else {
        background_checkers(painter, rect);
        painter.rect_filled(rect, 3.0, color);
    }
}

/// The classic two-tone checkerboard used behind translucent colors.
///
/// Cells are clipped to `rect` (so a narrow, tall rect cannot spill cells outside its bounds) and
/// the board is inset a little so its square corners stay inside the rounded swatch border.
fn background_checkers(painter: &Painter, rect: Rect) {
    let rect = rect.shrink(1.0);
    if !rect.is_positive() {
        return;
    }

    const CELL: f32 = 8.0;
    let dark = Color32::from_gray(32);
    let bright = Color32::from_gray(128);

    let mut mesh = Mesh::default();
    mesh.add_colored_rect(rect, dark);

    let mut y = rect.top();
    let mut row = 0;
    while y < rect.bottom() - 0.01 {
        let y1 = (y + CELL).min(rect.bottom());
        let mut x = rect.left();
        let mut col = 0;
        while x < rect.right() - 0.01 {
            let x1 = (x + CELL).min(rect.right());
            if (row + col) % 2 == 0 {
                mesh.add_colored_rect(Rect::from_min_max(pos2(x, y), pos2(x1, y1)), bright);
            }
            x = x1;
            col += 1;
        }
        y = y1;
        row += 1;
    }

    painter.add(Shape::mesh(mesh));
}

/// Pick black or white depending on which contrasts with `color`.
fn contrast_color(color: Color32) -> Color32 {
    if Rgba::from(color).intensity() < 0.5 {
        Color32::WHITE
    } else {
        Color32::BLACK
    }
}

/// Clickable swatch that opens the color editor popup. Returns true when the color changed.
fn color_swatch(ui: &mut Ui, key: &str, color: &mut Color32) -> bool {
    let (rect, response) = ui.allocate_exact_size(swatch_size(ui), Sense::click());
    paint_swatch(ui, rect, *color);

    let visuals = ui.style().interact(&response);
    ui.painter()
        .rect_stroke(rect, 3.0, visuals.bg_stroke, StrokeKind::Inside);

    let mut changed = false;
    Popup::menu(&response)
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            changed = color_edit_popup(ui, key, color);
        });
    changed
}

/// Non-interactive swatch shown when an optional override is off.
fn color_swatch_disabled(ui: &mut Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(swatch_size(ui), Sense::hover());
    paint_swatch(ui, rect, color.gamma_multiply(0.35));
    ui.painter().rect_stroke(
        rect,
        3.0,
        Stroke::new(1.0, ui.visuals().weak_text_color()),
        StrokeKind::Inside,
    );
}

/// The small color editor menu.
///
/// Layout: preview swatch + value readout, a saturation/value square with a vertical alpha bar, hex and RGBA fields, then hue/saturation/value gradient sliders.
fn color_edit_popup(ui: &mut Ui, key: &str, color: &mut Color32) -> bool {
    let original = *color;
    // A fixed width keeps the gradient sliders from stretching to the width of the whole window.
    ui.set_min_width(POPUP_WIDTH);
    ui.set_max_width(POPUP_WIDTH);
    ui.spacing_mut().item_spacing = vec2(6.0, 6.0);

    // Keep an Hsva working copy so dragging hue is not lost when the color becomes gray or black.
    let hsva_id = ui.make_persistent_id(("theme_hsva", key));
    let cached = ui.data(|d| d.get_temp::<([u8; 4], Hsva)>(hsva_id));
    let mut hsva = match cached {
        Some((bytes, h)) if bytes == color.to_array() => h,
        _ => Hsva::from(*color),
    };

    // Top: small preview swatch and the hex readout.
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(PREVIEW_SIZE), Sense::hover());
        paint_color_at(ui.painter(), *color, rect);
        ui.painter().rect_stroke(
            rect,
            3.0,
            Stroke::new(1.0, ui.visuals().weak_text_color()),
            StrokeKind::Inside,
        );
        ui.add_space(4.0);
        ui.label(egui::RichText::new(format_hex(*color)).monospace());
    });

    ui.add_space(2.0);

    // Saturation/value square and a vertical alpha bar.
    let before_pick = hsva;
    ui.horizontal(|ui| {
        sv_square(ui, &mut hsva, SV_SIZE);
        ui.add_space(4.0);
        alpha_bar(ui, &mut hsva, SV_SIZE);
    });
    if hsva != before_pick {
        *color = Color32::from(hsva);
    }

    ui.separator();

    // Hex field.
    // The buffer lives in egui memory so partial input survives while typing, it is regenerated from the color whenever the field is not focused.
    let buffer_id = ui.make_persistent_id(("theme_hex", key));
    let mut buffer = ui
        .data(|d| d.get_temp::<String>(buffer_id))
        .unwrap_or_else(|| format_hex(*color));
    let mut rgb_edited = false;
    ui.horizontal(|ui| {
        ui.label("HEX");
        let response = ui.add_sized(
            vec2(86.0, ui.spacing().interact_size.y),
            egui::TextEdit::singleline(&mut buffer).hint_text("#RRGGBBAA"),
        );
        if response.changed() {
            if let Some(parsed) = parse_clipboard_color(&buffer) {
                if parsed != *color {
                    *color = parsed;
                    rgb_edited = true;
                }
            }
        }
        if ui
            .button("Copy")
            .on_hover_text("Copy the hex color")
            .clicked()
        {
            ui.ctx().copy_text(format_hex(*color));
        }
        if ui
            .button("Paste")
            .on_hover_text("Paste a hex or RGBA color")
            .clicked()
        {
            if let Some(text) = read_clipboard() {
                if let Some(parsed) = parse_clipboard_color(&text) {
                    if parsed != *color {
                        *color = parsed;
                        rgb_edited = true;
                    }
                }
            }
        }
        if !response.has_focus() {
            buffer = format_hex(*color);
        }
    });
    ui.data_mut(|d| d.insert_temp(buffer_id, buffer));

    // RGBA (0-255).
    let [mut r, mut g, mut b, mut a] = color.to_srgba_unmultiplied();
    let field = vec2(RGBA_FIELD_WIDTH, ui.spacing().interact_size.y);
    ui.horizontal(|ui| {
        ui.label("RGBA");
        let mut edited = false;
        edited |= ui
            .add_sized(
                field,
                DragValue::new(&mut r)
                    .range(0..=255)
                    .speed(0.6)
                    .prefix("R "),
            )
            .changed();
        edited |= ui
            .add_sized(
                field,
                DragValue::new(&mut g)
                    .range(0..=255)
                    .speed(0.6)
                    .prefix("G "),
            )
            .changed();
        edited |= ui
            .add_sized(
                field,
                DragValue::new(&mut b)
                    .range(0..=255)
                    .speed(0.6)
                    .prefix("B "),
            )
            .changed();
        edited |= ui
            .add_sized(
                field,
                DragValue::new(&mut a)
                    .range(0..=255)
                    .speed(0.6)
                    .prefix("A "),
            )
            .changed();
        if edited {
            *color = color_from_unmultiplied(r, g, b, a);
            rgb_edited = true;
        }
    });
    ui.horizontal(|ui| {
        if ui
            .button("Copy")
            .on_hover_text("Copy the RGBA values")
            .clicked()
        {
            ui.ctx().copy_text(format_rgba(*color));
        }
        if ui
            .button("Paste")
            .on_hover_text("Paste RGBA values or a hex color")
            .clicked()
        {
            if let Some(text) = read_clipboard() {
                if let Some(parsed) = parse_clipboard_color(&text) {
                    if parsed != *color {
                        *color = parsed;
                        rgb_edited = true;
                    }
                }
            }
        }
    });

    if rgb_edited {
        // Re-derive the HSV working copy from the typed color.
        hsva = Hsva::from(*color);
    }

    ui.separator();

    // Hue / saturation / value gradient sliders.
    let before_sliders = hsva;
    let hue = hsva.h;
    let sat = hsva.s;
    let val = hsva.v;
    labeled_slider(ui, "Hue", &mut hsva.h, |h| {
        Color32::from(Hsva::new(h, 1.0, 1.0, 1.0))
    });
    labeled_slider(ui, "Saturation", &mut hsva.s, |s| {
        Color32::from(Hsva::new(hue, s, val, 1.0))
    });
    labeled_slider(ui, "Value", &mut hsva.v, |v| {
        Color32::from(Hsva::new(hue, sat, v, 1.0))
    });
    if hsva != before_sliders {
        *color = Color32::from(hsva);
    }

    ui.data_mut(|d| d.insert_temp(hsva_id, (color.to_array(), hsva)));

    *color != original
}

/// A label followed by a gradient slider that takes the remaining popup width.
fn labeled_slider(ui: &mut Ui, label: &str, value: &mut f32, color_at: impl Fn(f32) -> Color32) {
    ui.horizontal(|ui| {
        ui.add_sized(
            vec2(78.0, ui.spacing().interact_size.y),
            egui::Label::new(label),
        );
        gradient_slider(ui, value, color_at);
    });
}

/// A one dimensional gradient slider (used for hue, saturation and value).
fn gradient_slider(ui: &mut Ui, value: &mut f32, color_at: impl Fn(f32) -> Color32) -> Response {
    let desired = vec2(
        ui.available_width().max(40.0),
        ui.spacing().interact_size.y.max(18.0),
    );
    let (rect, response) = ui.allocate_at_least(desired, Sense::click_and_drag());

    if let Some(mpos) = response.interact_pointer_pos() {
        *value = remap_clamp(mpos.x, rect.left()..=rect.right(), 0.0..=1.0);
    }

    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact(&response);

        let mut mesh = Mesh::default();
        for i in 0..=GRADIENT_STEPS {
            let t = i as f32 / GRADIENT_STEPS as f32;
            let color = color_at(t);
            let x = lerp(rect.left()..=rect.right(), t);
            mesh.colored_vertex(pos2(x, rect.top()), color);
            mesh.colored_vertex(pos2(x, rect.bottom()), color);
            if i < GRADIENT_STEPS {
                mesh.add_triangle(2 * i, 2 * i + 1, 2 * i + 2);
                mesh.add_triangle(2 * i + 1, 2 * i + 2, 2 * i + 3);
            }
        }
        ui.painter().add(Shape::mesh(mesh));

        ui.painter()
            .rect_stroke(rect, 2.0, visuals.bg_stroke, StrokeKind::Inside);

        // Handle: a small triangle sitting at the bottom of the slider, pointing up at the value.
        let x = lerp(rect.left()..=rect.right(), *value);
        let r = (rect.height() / 4.0).max(4.0);
        let base_y = rect.bottom() - 1.0;
        let apex_y = base_y - r * 1.5;
        let picked = color_at(*value);
        ui.painter().add(Shape::convex_polygon(
            vec![pos2(x, apex_y), pos2(x + r, base_y), pos2(x - r, base_y)],
            picked,
            Stroke::new(1.0, contrast_color(picked)),
        ));
    }

    response
}

/// A square saturation (x) / value (y) picker for the current hue.
fn sv_square(ui: &mut Ui, hsva: &mut Hsva, size: f32) -> Response {
    let (rect, response) = ui.allocate_at_least(Vec2::splat(size), Sense::click_and_drag());

    if let Some(mpos) = response.interact_pointer_pos() {
        hsva.s = remap_clamp(mpos.x, rect.left()..=rect.right(), 0.0..=1.0);
        hsva.v = remap_clamp(mpos.y, rect.bottom()..=rect.top(), 0.0..=1.0);
    }

    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact(&response);

        let mut mesh = Mesh::default();
        for xi in 0..=GRADIENT_STEPS {
            for yi in 0..=GRADIENT_STEPS {
                let xt = xi as f32 / GRADIENT_STEPS as f32;
                let yt = yi as f32 / GRADIENT_STEPS as f32;
                let color = Color32::from(Hsva::new(hsva.h, xt, yt, 1.0));
                let x = lerp(rect.left()..=rect.right(), xt);
                let y = lerp(rect.bottom()..=rect.top(), yt);
                mesh.colored_vertex(pos2(x, y), color);
                if xi < GRADIENT_STEPS && yi < GRADIENT_STEPS {
                    let tl = yi * (GRADIENT_STEPS + 1) + xi;
                    mesh.add_triangle(tl, tl + 1, tl + GRADIENT_STEPS + 1);
                    mesh.add_triangle(tl + 1, tl + GRADIENT_STEPS + 1, tl + GRADIENT_STEPS + 2);
                }
            }
        }
        ui.painter().add(Shape::mesh(mesh));

        ui.painter()
            .rect_stroke(rect, 2.0, visuals.bg_stroke, StrokeKind::Inside);

        let x = lerp(rect.left()..=rect.right(), hsva.s);
        let y = lerp(rect.bottom()..=rect.top(), hsva.v);
        let picked = Color32::from(Hsva::new(hsva.h, hsva.s, hsva.v, 1.0));
        ui.painter().circle(
            pos2(x, y),
            SV_HANDLE_RADIUS,
            picked,
            Stroke::new(1.0, contrast_color(picked)),
        );
    }

    response
}

/// A vertical alpha bar for the current color (opaque at the top, transparent at the bottom).
fn alpha_bar(ui: &mut Ui, hsva: &mut Hsva, height: f32) -> Response {
    let (rect, response) =
        ui.allocate_at_least(vec2(ALPHA_BAR_WIDTH, height), Sense::click_and_drag());

    if let Some(mpos) = response.interact_pointer_pos() {
        hsva.a = remap_clamp(mpos.y, rect.bottom()..=rect.top(), 0.0..=1.0);
    }

    // Additive colors (egui stores them with a negative alpha sentinel) have no real alpha.
    // Clamp it for the handle so it sits at the bottom of the bar instead of being extrapolated below it.
    let display_alpha = hsva.a.clamp(0.0, 1.0);

    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact(&response);
        background_checkers(ui.painter(), rect);

        let opaque = Hsva { a: 1.0, ..*hsva };
        let mut mesh = Mesh::default();
        for i in 0..=GRADIENT_STEPS {
            let t = i as f32 / GRADIENT_STEPS as f32;
            let color = Color32::from(Hsva { a: t, ..opaque });
            let y = lerp(rect.bottom()..=rect.top(), t);
            mesh.colored_vertex(pos2(rect.left(), y), color);
            mesh.colored_vertex(pos2(rect.right(), y), color);
            if i < GRADIENT_STEPS {
                mesh.add_triangle(2 * i, 2 * i + 1, 2 * i + 2);
                mesh.add_triangle(2 * i + 1, 2 * i + 2, 2 * i + 3);
            }
        }
        ui.painter().add(Shape::mesh(mesh));

        ui.painter()
            .rect_stroke(rect, 2.0, visuals.bg_stroke, StrokeKind::Inside);

        // Handle: a horizontal marker at the current alpha, drawn in the opaque color so it is always visible against the gradient.
        let y = lerp(rect.bottom()..=rect.top(), display_alpha);
        let marker = Rect::from_min_max(
            pos2(rect.left() + 1.0, y - 1.5),
            pos2(rect.right() - 1.0, y + 1.5),
        );
        let handle = Color32::from(opaque);
        ui.painter().rect_filled(marker, 0.0, handle);
        ui.painter().rect_stroke(
            marker,
            0.0,
            Stroke::new(1.0, contrast_color(handle)),
            StrokeKind::Inside,
        );
    }

    response
}

/// Format a color as `#RRGGBB`, or `#RRGGBBAA` when it is not fully opaque.
pub fn format_hex(color: Color32) -> String {
    let [r, g, b, a] = color.to_srgba_unmultiplied();
    if a == 255 {
        format!("#{r:02X}{g:02X}{b:02X}")
    } else {
        format!("#{r:02X}{g:02X}{b:02X}{a:02X}")
    }
}

/// Format a color as unmultiplied `r, g, b, a` (0-255 each).
pub fn format_rgba(color: Color32) -> String {
    let [r, g, b, a] = color.to_srgba_unmultiplied();
    format!("{r}, {g}, {b}, {a}")
}

/// Parse `r,g,b` or `r,g,b,a` (comma, space, tab or semicolon separated, 0-255 each), optionally wrapped as `RGB(r,g,b)` or `RGBA(r,g,b,a)`.
pub fn parse_rgba_list(text: &str) -> Option<Color32> {
    let text = text.trim();
    let inner = strip_rgb_function(text).unwrap_or(text);
    let parts: Vec<&str> = inner
        .split([',', ' ', '\t', ';'])
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    let value = |i: usize| parts[i].parse::<u8>().ok();
    match parts.len() {
        3 => Some(Color32::from_rgb(value(0)?, value(1)?, value(2)?)),
        4 => Some(color_from_unmultiplied(
            value(0)?,
            value(1)?,
            value(2)?,
            value(3)?,
        )),
        _ => None,
    }
}

/// If `text` is `RGB(...)` or `RGBA(...)`, return the contents of the parentheses.
fn strip_rgb_function(text: &str) -> Option<&str> {
    for prefix in ["rgba", "rgb"] {
        if let Some(head) = text.get(..prefix.len()) {
            if head.eq_ignore_ascii_case(prefix) {
                let rest = text[prefix.len()..].trim();
                if let Some(inner) = rest.strip_prefix('(').and_then(|r| r.strip_suffix(')')) {
                    return Some(inner.trim());
                }
            }
        }
    }
    None
}

/// Parse a color pasted from the clipboard: either a hex string or `r,g,b[,a]`.
pub fn parse_clipboard_color(text: &str) -> Option<Color32> {
    let text = text.trim();
    parse_hex(text).or_else(|| parse_rgba_list(text))
}

/// Read the system clipboard as text, if a clipboard is available.
fn read_clipboard() -> Option<String> {
    arboard::Clipboard::new().ok()?.get_text().ok()
}

/// Parse `#RGB`, `#RGBA`, `#RRGGBB` or `#RRGGBBAA` (the leading `#` is optional).
pub fn parse_hex(text: &str) -> Option<Color32> {
    let s = text.trim().trim_start_matches('#');
    // Reject anything that is not ASCII hex first, so the byte-index slicing below can never split a multibyte UTF-8 character and panic.
    if !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).ok();
    let nibble = |i: usize| u8::from_str_radix(&s[i..i + 1], 16).ok().map(|v| v * 17);
    match s.len() {
        3 => Some(Color32::from_rgb(nibble(0)?, nibble(1)?, nibble(2)?)),
        4 => Some(color_from_unmultiplied(
            nibble(0)?,
            nibble(1)?,
            nibble(2)?,
            nibble(3)?,
        )),
        6 => Some(Color32::from_rgb(byte(0)?, byte(2)?, byte(4)?)),
        8 => Some(color_from_unmultiplied(
            byte(0)?,
            byte(2)?,
            byte(4)?,
            byte(6)?,
        )),
        _ => None,
    }
}

/// Build a color from unmultiplied RGBA bytes.
///
/// egui stores colors premultiplied and represents "additive" colors as `[r, g, b, 0]`, which is exactly what `format_hex` and `format_rgba` write out.
/// Using premultiplied bytes for zero alpha keeps `#RRGGBB00` and `r, g, b, 0` round-tripping an additive color instead of collapsing it to transparent black.
pub fn color_from_unmultiplied(r: u8, g: u8, b: u8, a: u8) -> Color32 {
    if a == 0 {
        Color32::from_rgba_premultiplied(r, g, b, 0)
    } else {
        Color32::from_rgba_unmultiplied(r, g, b, a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trip() {
        let color = Color32::from_rgb(18, 52, 86);
        assert_eq!(format_hex(color), "#123456");
        assert_eq!(parse_hex("#123456"), Some(color));
        assert_eq!(parse_hex("123456"), Some(color));
    }

    #[test]
    fn hex_alpha_round_trip() {
        let color = Color32::from_rgba_unmultiplied(18, 52, 86, 128);
        assert_eq!(format_hex(color), "#12345680");
        assert_eq!(parse_hex("#12345680"), Some(color));
    }

    #[test]
    fn hex_shorthand() {
        assert_eq!(parse_hex("#abc"), Some(Color32::from_rgb(0xAA, 0xBB, 0xCC)));
        assert_eq!(
            parse_hex("#abc8"),
            Some(Color32::from_rgba_unmultiplied(0xAA, 0xBB, 0xCC, 0x88))
        );
    }

    #[test]
    fn invalid_hex() {
        assert_eq!(parse_hex("nope"), None);
        assert_eq!(parse_hex("#12"), None);
    }

    #[test]
    fn hex_rejects_non_ascii_without_panicking() {
        assert_eq!(parse_hex("a\u{e9}"), None);
        assert_eq!(parse_hex("#ff\u{e9}"), None);
    }

    #[test]
    fn dark_theme_round_trips_through_visuals() {
        let theme = Theme::dark();
        let visuals = theme.to_visuals();
        assert_eq!(theme, Theme::from_visuals(&visuals));
    }

    #[test]
    fn theme_serde_round_trip() {
        let mut theme = Theme::dark();
        theme.hyperlink_color = ThemeColor(Color32::from_rgb(1, 2, 3));
        theme.override_text_color =
            Some(ThemeColor(Color32::from_rgba_unmultiplied(10, 20, 30, 128)));
        theme.window_shadow = ThemeColor(Color32::from_black_alpha(80));
        theme.disabled_alpha = 0.42;
        let json = serde_json::to_string_pretty(&theme).unwrap();
        let back: Theme = serde_json::from_str(&json).unwrap();
        assert_eq!(theme, back);
    }

    #[test]
    fn theme_serde_accepts_partial_and_missing_fields() {
        // Older configs (or hand-edited ones) may omit fields.
        let back: Theme = serde_json::from_str("{\"dark_mode\": false}").unwrap();
        assert!(!back.dark_mode);
        assert_eq!(back.panel_fill, Theme::dark().panel_fill);
    }

    #[test]
    fn contrast_color_is_readable() {
        assert_eq!(contrast_color(Color32::BLACK), Color32::WHITE);
        assert_eq!(contrast_color(Color32::WHITE), Color32::BLACK);
    }

    #[test]
    fn rgba_list_parsing() {
        assert_eq!(
            parse_rgba_list("255, 0, 0"),
            Some(Color32::from_rgb(255, 0, 0))
        );
        assert_eq!(
            parse_rgba_list("18 52 86 128"),
            Some(Color32::from_rgba_unmultiplied(18, 52, 86, 128))
        );
        assert_eq!(parse_rgba_list("300, 0, 0"), None);
        assert_eq!(parse_rgba_list("1, 2"), None);
    }

    #[test]
    fn rgba_format_round_trips_via_list() {
        let color = Color32::from_rgba_unmultiplied(18, 52, 86, 128);
        let text = format_rgba(color);
        assert_eq!(text, "18, 52, 86, 128");
        assert_eq!(parse_rgba_list(&text), Some(color));
    }

    #[test]
    fn rgba_zero_alpha_round_trips_additive() {
        // The RGBA editor and paste must not collapse a zero-alpha additive color to all zeros.
        let additive = Color32::from_rgba_premultiplied(5, 5, 5, 0);
        assert_eq!(format_rgba(additive), "5, 5, 5, 0");
        assert_eq!(parse_rgba_list("5, 5, 5, 0"), Some(additive));
        assert_eq!(parse_clipboard_color("5, 5, 5, 0"), Some(additive));
    }

    #[test]
    fn paste_detects_hex_and_rgba_interchangeably() {
        // A hex string pasted into the RGBA control (and an RGBA string into the hex control) is detected either way, since both route through `parse_clipboard_color`.
        assert_eq!(
            parse_clipboard_color("#FF8800"),
            Some(Color32::from_rgb(0xFF, 0x88, 0x00))
        );
        assert_eq!(
            parse_clipboard_color("255, 136, 0"),
            Some(Color32::from_rgb(0xFF, 0x88, 0x00))
        );
        assert_eq!(
            parse_clipboard_color("rgba(255, 136, 0, 128)"),
            Some(Color32::from_rgba_unmultiplied(255, 136, 0, 128))
        );
    }

    #[test]
    fn clipboard_color_accepts_hex_or_list() {
        assert_eq!(
            parse_clipboard_color("  #123456 "),
            Some(Color32::from_rgb(0x12, 0x34, 0x56))
        );
        assert_eq!(
            parse_clipboard_color("050505"),
            Some(Color32::from_rgb(5, 5, 5))
        );
        assert_eq!(
            parse_clipboard_color("18, 52, 86"),
            Some(Color32::from_rgb(18, 52, 86))
        );
        assert_eq!(parse_clipboard_color("not a color"), None);
    }

    #[test]
    fn hex_zero_alpha_round_trips_additive() {
        // egui's default faint background is the additive color [5, 5, 5, 0].
        let additive = Color32::from_rgba_premultiplied(5, 5, 5, 0);
        assert_eq!(format_hex(additive), "#05050500");
        assert_eq!(parse_hex("#05050500"), Some(additive));
        assert_eq!(parse_hex("05050500"), Some(additive));
    }

    #[test]
    fn rgb_function_form_parsing() {
        assert_eq!(
            parse_rgba_list("RGB(1, 2, 3)"),
            Some(Color32::from_rgb(1, 2, 3))
        );
        assert_eq!(
            parse_rgba_list("rgba(1, 2, 3, 4)"),
            Some(Color32::from_rgba_unmultiplied(1, 2, 3, 4))
        );
        assert_eq!(
            parse_clipboard_color("RGB(255,0,0)"),
            Some(Color32::from_rgb(255, 0, 0))
        );
        assert_eq!(
            parse_clipboard_color("RGBA(0, 0, 0, 128)"),
            Some(Color32::from_rgba_unmultiplied(0, 0, 0, 128))
        );
    }
}

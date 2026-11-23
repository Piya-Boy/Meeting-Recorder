//! Fixed-grid table rows. Header and body rows share one column spec so cells
//! always line up, regardless of content width.

use egui::{Color32, Rect, Response, RichText, Sense, Ui, UiBuilder};

use crate::theme::ThemeColors;

#[derive(Clone, Copy)]
pub enum Width {
    Fixed(f32),
    /// Takes whatever is left after fixed columns (at most one per table).
    Flex,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Right,
}

#[derive(Clone, Copy)]
pub struct Column {
    pub width: Width,
    pub align: Align,
}

impl Column {
    pub const fn fixed(width: f32, align: Align) -> Self {
        Self {
            width: Width::Fixed(width),
            align,
        }
    }

    pub const fn flex() -> Self {
        Self {
            width: Width::Flex,
            align: Align::Left,
        }
    }
}

const PAD_X: f32 = 16.0;
const GAP: f32 = 12.0;

fn column_rects(total: Rect, cols: &[Column]) -> Vec<Rect> {
    let inner_w = (total.width() - PAD_X * 2.0).max(0.0);
    let gaps = GAP * cols.len().saturating_sub(1) as f32;
    let fixed: f32 = cols
        .iter()
        .map(|c| match c.width {
            Width::Fixed(w) => w,
            Width::Flex => 0.0,
        })
        .sum();
    let flex_w = (inner_w - fixed - gaps).max(40.0);
    let mut x = total.left() + PAD_X;
    cols.iter()
        .map(|c| {
            let w = match c.width {
                Width::Fixed(w) => w,
                Width::Flex => flex_w,
            };
            let r = Rect::from_min_max(
                egui::pos2(x, total.top()),
                egui::pos2(x + w, total.bottom()),
            );
            x += w + GAP;
            r
        })
        .collect()
}

/// Cell cursor handed to the row-building closure.
pub struct Cells<'a> {
    ui: &'a mut Ui,
    rects: Vec<Rect>,
    cols: &'a [Column],
    next: usize,
}

impl Cells<'_> {
    /// Adds the next cell. Content is vertically centred in the row.
    pub fn cell(&mut self, add: impl FnOnce(&mut Ui)) {
        let Some(rect) = self.rects.get(self.next).copied() else {
            return;
        };
        let align = self.cols[self.next].align;
        self.next += 1;
        let layout = match align {
            Align::Left => egui::Layout::left_to_right(egui::Align::Center),
            Align::Right => egui::Layout::right_to_left(egui::Align::Center),
        };
        let mut child = self
            .ui
            .new_child(UiBuilder::new().max_rect(rect).layout(layout));
        child.set_clip_rect(rect.intersect(self.ui.clip_rect()));
        add(&mut child);
    }

    /// Adds the next cell as a vertical stack (e.g. title over subtitle),
    /// centred using the caller's estimate of the stack's height.
    pub fn stack(&mut self, content_h: f32, add: impl FnOnce(&mut Ui)) {
        let Some(rect) = self.rects.get(self.next).copied() else {
            return;
        };
        self.next += 1;
        let layout = egui::Layout::top_down(egui::Align::Min);
        let mut child = self
            .ui
            .new_child(UiBuilder::new().max_rect(rect).layout(layout));
        child.set_clip_rect(rect.intersect(self.ui.clip_rect()));
        child.spacing_mut().item_spacing.y = 2.0;
        child.add_space(((rect.height() - content_h) / 2.0).max(0.0));
        add(&mut child);
    }
}

/// Header row (uppercase caption labels on the variant surface).
pub fn header_row(ui: &mut Ui, theme: &ThemeColors, cols: &[Column], labels: &[&str]) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 40.0), Sense::hover());
    ui.painter().rect_filled(
        rect,
        egui::CornerRadius {
            nw: 8,
            ne: 8,
            sw: 0,
            se: 0,
        },
        theme.surface_elevated,
    );
    ui.painter().hline(
        rect.x_range(),
        rect.bottom(),
        egui::Stroke::new(1.0, theme.outline),
    );
    let mut cells = Cells {
        ui,
        rects: column_rects(rect, cols),
        cols,
        next: 0,
    };
    for label in labels {
        cells.cell(|ui| {
            ui.label(
                RichText::new(crate::i18n::tr(label).to_uppercase())
                    .size(11.0)
                    .family(crate::theme::bold_family())
                    .color(theme.on_surface_variant),
            );
        });
    }
}

/// Body row. `selected` tints the row; hover highlight is automatic.
/// Returns the row response (for click handling on the row itself).
pub fn body_row(
    ui: &mut Ui,
    theme: &ThemeColors,
    cols: &[Column],
    height: f32,
    selected: bool,
    last: bool,
    build: impl FnOnce(&mut Cells),
) -> Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), height), Sense::hover());
    let fill = if selected {
        Color32::from_rgba_unmultiplied(
            theme.primary_container.r(),
            theme.primary_container.g(),
            theme.primary_container.b(),
            110,
        )
    } else if ui.rect_contains_pointer(rect) {
        Color32::from_rgba_unmultiplied(
            theme.surface_elevated.r(),
            theme.surface_elevated.g(),
            theme.surface_elevated.b(),
            120,
        )
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, 0, fill);
    if !last {
        ui.painter().hline(
            rect.x_range(),
            rect.bottom(),
            egui::Stroke::new(1.0, theme.outline_variant),
        );
    }
    let mut cells = Cells {
        ui,
        rects: column_rects(rect, cols),
        cols,
        next: 0,
    };
    build(&mut cells);
    response
}

/// Rounded, bordered container that clips the table.
pub fn table_frame(ui: &mut Ui, theme: &ThemeColors, add: impl FnOnce(&mut Ui)) {
    egui::Frame::NONE
        .fill(theme.surface)
        .stroke(egui::Stroke::new(1.0, theme.outline))
        .corner_radius(egui::CornerRadius::same(8))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.set_width(ui.available_width());
            add(ui);
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flex_column_absorbs_remaining_width() {
        let total = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 40.0));
        let cols = [
            Column::flex(),
            Column::fixed(100.0, Align::Left),
            Column::fixed(200.0, Align::Right),
        ];
        let rects = column_rects(total, &cols);
        assert_eq!(rects.len(), 3);
        // 800 - 2*PAD_X - 100 - 200 - 2*GAP
        assert_eq!(rects[0].width(), 800.0 - 32.0 - 300.0 - 24.0);
        assert_eq!(rects[1].width(), 100.0);
        assert_eq!(rects[2].width(), 200.0);
    }

    #[test]
    fn columns_never_overlap() {
        let total = Rect::from_min_size(egui::pos2(10.0, 0.0), egui::vec2(600.0, 40.0));
        let cols = [
            Column::fixed(120.0, Align::Left),
            Column::flex(),
            Column::fixed(90.0, Align::Right),
        ];
        let rects = column_rects(total, &cols);
        for pair in rects.windows(2) {
            assert!(pair[0].right() <= pair[1].left());
        }
    }

    #[test]
    fn tiny_width_keeps_minimum_flex() {
        let total = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(100.0, 40.0));
        let rects = column_rects(total, &[Column::flex(), Column::fixed(300.0, Align::Left)]);
        assert!(rects[0].width() >= 40.0);
    }
}

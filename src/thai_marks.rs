//! Thai marks that sit above one another.
//!
//! egui does no text shaping, so a tone mark that follows an upper vowel (as in "ที่") is drawn
//! at the same height, on top of it, and so is one before sara am (as in "น้ำ"). Just before a
//! frame is handed to the renderer, the drawing of each such mark is moved up until it clears
//! what is under it. The text itself is not touched, so copying and searching are unaffected.
//!
//! A line of egui's own height has no room above it for a raised mark: it would sit where the
//! line above writes its sara u, and read as one. So the second and later lines of a text are
//! only raised when they are tall enough, which a paragraph asks for with [`line_height`].

use std::sync::Arc;

use egui::epaint::text::{Glyph, LayoutJob, Row};
use egui::epaint::ClippedShape;
use egui::epaint::Galley;
use egui::layers::ShapeIdx;
use egui::{LayerId, Shape};

/// Overlap of two glyph boxes, in points, that is only rounding error.
const SLACK: f32 = 0.01;

/// Line height, in font sizes, of a Thai paragraph.
const THAI_LINE: f32 = 1.5;
/// The smallest line height, in font sizes, that has room to raise a mark in a later line.
const ROOM: f32 = 1.45;

/// The line height to ask for in a paragraph of this text and font size, or `None` to keep
/// egui's own, which is enough for text without Thai.
pub fn line_height(text: &str, size: f32) -> Option<f32> {
    text.chars()
        .any(|c| matches!(c, '\u{0E01}'..='\u{0E5B}'))
        .then_some(size * THAI_LINE)
}

/// Vowels and signs drawn above a consonant.
fn is_upper_vowel(c: char) -> bool {
    matches!(
        c,
        '\u{0E31}' | '\u{0E34}'..='\u{0E37}' | '\u{0E47}' | '\u{0E4D}'
    )
}

/// Tone marks and thanthakhat, which are written above those.
fn is_top_mark(c: char) -> bool {
    matches!(c, '\u{0E48}'..='\u{0E4C}')
}

/// Sara am carries a ring above the consonant before it.
const SARA_AM: char = '\u{0E33}';

/// Whether `text` has a mark with something under it. Most text has none and is left alone.
fn has_stacked_marks(text: &str) -> bool {
    let mut previous = ' ';
    text.chars().any(|c| {
        let stacked =
            (is_upper_vowel(previous) && is_top_mark(c)) || (is_top_mark(previous) && c == SARA_AM);
        previous = c;
        stacked
    })
}

/// Top and bottom of the drawing of `glyph`, in the coordinates of its text.
fn extent(glyph: &Glyph) -> Option<(f32, f32)> {
    (!glyph.uv_rect.is_nothing()).then(|| {
        let top = glyph.pos.y + glyph.uv_rect.offset.y;
        (top, top + glyph.uv_rect.size.y)
    })
}

/// How far up the mark at `i` has to move to clear what is under it; 0 if it already does.
/// The boxes only have to meet: a glyph's box already has a faint edge around its ink, so any
/// further space makes the mark look adrift.
fn lift_for(glyphs: &[Glyph], i: usize) -> f32 {
    if !is_top_mark(glyphs[i].chr) {
        return 0.0;
    }
    let Some((_, bottom)) = extent(&glyphs[i]) else {
        return 0.0;
    };
    let vowel = i
        .checked_sub(1)
        .map(|p| &glyphs[p])
        .filter(|g| is_upper_vowel(g.chr));
    let sara_am = glyphs.get(i + 1).filter(|g| g.chr == SARA_AM);
    [vowel, sara_am]
        .into_iter()
        .flatten()
        .filter_map(extent)
        .map(|(top, _)| bottom - top)
        .fold(0.0, f32::max)
}

/// Raises the stacked marks of one row, in its glyphs and in the mesh that is drawn. `above`
/// is the height of the row over it, if there is one.
fn raise_row(row: &mut Row, above: Option<f32>, job: &LayoutJob, pixels_per_point: f32) {
    // The mesh holds four vertices for each glyph that draws something, in glyph order.
    let mut vertex = row.visuals.glyph_vertex_range.start;
    let mut moved = false;
    for i in 0..row.glyphs.len() {
        if row.glyphs[i].uv_rect.is_nothing() {
            continue;
        }
        let quad = vertex..vertex + 4;
        vertex += 4;
        let lift = lift_for(&row.glyphs, i);
        if lift <= SLACK {
            continue;
        }
        let size = job
            .sections
            .get(row.glyphs[i].section_index as usize)
            .map_or(0.0, |s| s.format.font_id.size);
        if above.is_some_and(|height| height < size * ROOM) {
            continue;
        }
        let Some(vertices) = row.visuals.mesh.vertices.get_mut(quad) else {
            return;
        };
        // Whole pixels only, so the mark stays as sharp as the rest of the text.
        let lift = ((lift - SLACK) * pixels_per_point).ceil() / pixels_per_point;
        for v in vertices {
            v.pos.y -= lift;
        }
        row.glyphs[i].pos.y -= lift;
        moved = true;
    }
    if moved {
        row.visuals.mesh_bounds = row.visuals.mesh.calc_bounds();
    }
}

fn raise(galley: &mut Galley) {
    let pixels_per_point = galley.pixels_per_point;
    let mut above = None;
    for row in &mut galley.rows {
        let height = row.rect.height();
        raise_row(row, above.replace(height), &galley.job, pixels_per_point);
        // The renderer skips what is outside these bounds.
        galley.mesh_bounds = galley.mesh_bounds.union(row.visuals.mesh_bounds);
    }
}

fn raise_in(shape: &mut Shape) {
    match shape {
        // A galley is shared with egui's layout cache, so a copy is changed.
        Shape::Text(t) if has_stacked_marks(&t.galley.job.text) => {
            raise(Arc::make_mut(&mut t.galley))
        }
        Shape::Vec(shapes) => shapes.iter_mut().for_each(raise_in),
        _ => {}
    }
}

/// Raises the marks of one painted shape. A one-line input clips just above its text, which would
/// cut a raised mark: if the clip held all of the text, it follows the mark up. A clip belongs to
/// one shape, so nothing else comes into view, and text that was already cut off stays so.
fn raise_clipped(clipped: &mut ClippedShape) {
    let top = |shape: &Shape| match shape {
        Shape::Text(t) => Some(t.pos.y + t.galley.mesh_bounds.top()),
        _ => None,
    };
    let before = top(&clipped.shape);
    raise_in(&mut clipped.shape);
    if let (Some(before), Some(after)) = (before, top(&clipped.shape)) {
        let held = before >= clipped.clip_rect.top();
        if held && after < clipped.clip_rect.top() {
            clipped.clip_rect.min.y = after;
        }
    }
}

/// Raises the stacked marks in everything painted so far in this frame.
fn raise_all(ctx: &egui::Context) {
    // Panels paint on the background layer; windows, menus, tooltips and dialogs are areas.
    let mut layers: Vec<LayerId> = ctx.memory(|m| m.layer_ids().collect());
    if !layers.contains(&LayerId::background()) {
        layers.push(LayerId::background());
    }
    ctx.graphics_mut(|graphics| {
        for layer in layers {
            let Some(list) = graphics.get_mut(layer) else {
                continue;
            };
            for i in 0..list.next_idx().0 {
                list.mutate_shape(ShapeIdx(i), raise_clipped);
            }
        }
    });
}

/// Makes every frame of `ctx` draw stacked Thai marks apart.
pub fn install(ctx: &egui::Context) {
    // Running twice is harmless: a mark that has been raised needs no further lift.
    ctx.on_end_pass("thai marks", Arc::new(raise_all));
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use egui::epaint::Galley;

    fn app_context() -> egui::Context {
        let ctx = egui::Context::default();
        crate::theme::configure_fonts(&ctx);
        ctx
    }

    fn find(shape: &egui::Shape, text: &str) -> Option<Arc<Galley>> {
        match shape {
            egui::Shape::Text(t) if t.galley.job.text == text => Some(t.galley.clone()),
            egui::Shape::Vec(v) => v.iter().find_map(|s| find(s, text)),
            _ => None,
        }
    }

    /// The text of a label as a frame hands it to the renderer.
    fn drawn(ctx: &egui::Context, text: &str) -> Arc<Galley> {
        let frame = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.label(text);
            });
        });
        frame
            .shapes
            .iter()
            .find_map(|s| find(&s.shape, text))
            .expect("the label is painted")
    }

    /// For each character: top and bottom of what is drawn for it, read from the mesh.
    fn quads(galley: &Galley) -> Vec<(char, Option<(f32, f32)>)> {
        let mut out = Vec::new();
        for row in &galley.rows {
            let mut at = row.visuals.glyph_vertex_range.start;
            for glyph in &row.glyphs {
                if glyph.uv_rect.is_nothing() {
                    out.push((glyph.chr, None));
                    continue;
                }
                let ys = row.visuals.mesh.vertices[at..at + 4]
                    .iter()
                    .map(|v| v.pos.y);
                let top = ys.clone().fold(f32::INFINITY, f32::min);
                let bottom = ys.fold(f32::NEG_INFINITY, f32::max);
                out.push((glyph.chr, Some((top, bottom))));
                at += 4;
            }
        }
        out
    }

    /// The top and bottom of the `n`th character.
    fn quad(q: &[(char, Option<(f32, f32)>)], n: usize) -> (f32, f32) {
        q[n].1
            .unwrap_or_else(|| panic!("{:?} is not drawn", q[n].0))
    }

    #[test]
    fn a_mark_is_drawn_above_what_is_under_it() {
        let ctx = app_context();
        drawn(&ctx, "warm up"); // fonts are ready from the second frame
                                // (text, position of the mark, position of the vowel or sara am it has to clear)
        for (text, mark, under) in [
            ("ที่", 2, 1),       // sara ii + mai ek
            ("ทั่วไป", 2, 1),    // mai han akat + mai ek
            ("ชื้อ", 2, 1),      // sara uee + mai tho
            ("สิทธิ์", 5, 4),     // sara i + thanthakhat
            ("กิ๊ก", 2, 1),      // sara i + mai tri
            ("ก็่", 2, 1),       // mai taikhu + mai ek
            ("น้ำ", 1, 2),      // mai tho before sara am
            ("คิดว่าที่นั่น", 8, 7), // a word further into a line
        ] {
            let galley = drawn(&ctx, text);
            let q = quads(&galley);
            let (mark_top, mark_bottom) = quad(&q, mark);
            let (under_top, _) = quad(&q, under);
            assert!(
                mark_bottom <= under_top + 0.01,
                "{text}: {:?} ends at {mark_bottom}, {:?} starts at {under_top}",
                q[mark].0,
                q[under].0
            );
            assert!(
                galley.mesh_bounds.top() <= mark_top + 0.01,
                "{text}: the raised mark is outside the bounds that decide what is drawn"
            );
        }
    }

    #[test]
    fn text_without_stacked_marks_is_drawn_as_the_font_lays_it_out() {
        let ctx = app_context();
        drawn(&ctx, "warm up");
        // A tone mark straight on a consonant, sara am without one, and no Thai at all.
        for text in ["ก่อน", "ไม้", "ทำ", "ภาษา", "Recorder"] {
            let shown = drawn(&ctx, text);
            let font = egui::TextStyle::Body.resolve(&ctx.style());
            let laid_out =
                ctx.fonts(|f| f.layout_no_wrap(text.to_owned(), font, egui::Color32::WHITE));
            let relative = |g: &Galley| {
                let q = quads(g);
                let base = quad(&q, 0).0;
                q.iter()
                    .map(|(c, y)| (*c, y.map(|(t, b)| (t - base, b - base))))
                    .collect::<Vec<_>>()
            };
            assert_eq!(relative(&shown), relative(&laid_out), "{text}");
        }
    }

    /// The frame's drawing of a paragraph, with a line height asked for or egui's own.
    fn paragraph(ctx: &egui::Context, text: &str, line_height: Option<f32>) -> Arc<Galley> {
        let frame = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.label(
                    egui::RichText::new(text)
                        .size(15.0)
                        .line_height(line_height),
                );
            });
        });
        frame
            .shapes
            .iter()
            .find_map(|s| find(&s.shape, text))
            .expect("the paragraph is painted")
    }

    /// Where a character is drawn: the `n`th of those equal to `c`.
    fn nth(galley: &Galley, c: char, n: usize) -> (f32, f32) {
        quads(galley)
            .into_iter()
            .filter(|(x, _)| *x == c)
            .nth(n)
            .and_then(|(_, y)| y)
            .unwrap_or_else(|| panic!("{c:?} number {n} is not drawn"))
    }

    const MAI_EK: char = '\u{0E48}';
    const SARA_II: char = '\u{0E35}';
    const SARA_U: char = '\u{0E38}';

    #[test]
    fn a_line_without_room_above_keeps_its_marks_where_the_font_put_them() {
        let ctx = app_context();
        drawn(&ctx, "warm up");
        // Raised here, the second mai ek would sit where the first line's sara u goes.
        let galley = paragraph(&ctx, "ที่ทุก\nที่", None);
        let first = nth(&galley, MAI_EK, 0).1 - nth(&galley, SARA_II, 0).0;
        let second = nth(&galley, MAI_EK, 1).1 - nth(&galley, SARA_II, 1).0;
        assert!(first <= 0.01, "the first line is raised: {first}");
        assert!(second > 0.5, "the second line is left alone: {second}");
    }

    #[test]
    fn thai_paragraphs_get_room_to_raise_marks_on_every_line() {
        let ctx = app_context();
        drawn(&ctx, "warm up");
        let text = "ทุก\nที่";
        let galley = paragraph(&ctx, text, super::line_height(text, 15.0));
        let (mark_top, mark_bottom) = nth(&galley, MAI_EK, 0);
        assert!(
            mark_bottom <= nth(&galley, SARA_II, 0).0 + 0.01,
            "the second line is raised"
        );
        let above = nth(&galley, SARA_U, 0).1;
        assert!(
            mark_top >= above + 1.0,
            "the mark starts at {mark_top}, too close to the sara u above it, ending at {above}"
        );
    }

    #[test]
    fn only_thai_paragraphs_are_made_taller() {
        assert_eq!(super::line_height("Quarterly planning, part 2", 15.0), None);
        assert_eq!(super::line_height("ก่อน deploy", 15.0), Some(22.5));
    }

    #[test]
    fn marks_are_raised_in_windows_too() {
        let ctx = app_context();
        drawn(&ctx, "warm up");
        let mut seen = None;
        // A window is painted on a layer of its own, as menus, tooltips and dialogs are.
        for _ in 0..3 {
            let frame = ctx.run(egui::RawInput::default(), |ctx| {
                egui::Window::new("dialog").show(ctx, |ui| {
                    ui.label("ทั่วไป");
                });
            });
            seen = frame.shapes.iter().find_map(|s| find(&s.shape, "ทั่วไป"));
        }
        let q = quads(&seen.expect("the window is painted"));
        assert!(quad(&q, 2).1 <= quad(&q, 1).0 + 0.01);
    }

    /// The frame's drawing of a text together with where it is and what it is clipped to.
    fn placed(frame: &egui::FullOutput, text: &str) -> (egui::epaint::TextShape, egui::Rect) {
        frame
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.job.text == text => Some((t.clone(), s.clip_rect)),
                _ => None,
            })
            .expect("the text is painted")
    }

    #[test]
    fn a_one_line_input_does_not_cut_off_a_raised_mark() {
        let ctx = app_context();
        drawn(&ctx, "warm up");
        for size in [12.0, 13.0, 15.0] {
            let mut text = String::from("ที่ประชุม");
            let mut frame = None;
            for _ in 0..2 {
                frame = Some(ctx.run(egui::RawInput::default(), |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let font = egui::FontId::proportional(size);
                        ui.add(egui::TextEdit::singleline(&mut text).font(font));
                    });
                }));
            }
            let (shape, clip) = placed(&frame.expect("a frame"), &text);
            let highest = shape.pos.y + quad(&quads(&shape.galley), 2).0;
            assert!(
                highest >= clip.top() - 0.01,
                "size {size}: the mark starts at {highest}, the clip at {}",
                clip.top()
            );
        }
    }

    #[test]
    fn text_that_is_already_cut_off_stays_cut_off() {
        let ctx = app_context();
        drawn(&ctx, "warm up");
        // As text scrolled halfway out of a list: the clip starts below the top of the text.
        let cut = egui::Rect::from_min_max(egui::pos2(0.0, 108.0), egui::pos2(300.0, 200.0));
        let frame = ctx.run(egui::RawInput::default(), |ctx| {
            ctx.layer_painter(egui::LayerId::background())
                .with_clip_rect(cut)
                .text(
                    egui::pos2(10.0, 100.0),
                    egui::Align2::LEFT_TOP,
                    "ที่",
                    egui::FontId::proportional(15.0),
                    egui::Color32::WHITE,
                );
        });
        assert_eq!(placed(&frame, "ที่").1.top(), 108.0);
    }

    #[test]
    fn a_mark_is_not_raised_further_on_later_frames() {
        let ctx = app_context();
        drawn(&ctx, "warm up");
        let first = quads(&drawn(&ctx, "ที่นี่"));
        for _ in 0..3 {
            assert_eq!(quads(&drawn(&ctx, "ที่นี่")), first);
        }
    }

    #[test]
    fn installing_twice_changes_nothing() {
        let ctx = app_context();
        drawn(&ctx, "warm up");
        let once = quads(&drawn(&ctx, "ที่นี่"));
        super::install(&ctx);
        assert_eq!(quads(&drawn(&ctx, "ที่นี่")), once);
    }
}

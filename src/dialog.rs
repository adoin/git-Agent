use eframe::egui::{Pos2, Rect, Vec2};

/// Distance from the viewport top to every in-app dialog.
/// Adjust this single value to move all dialog top edges together.
pub const DIALOG_TOP_OFFSET: f32 = 84.0;

const DIALOG_EDGE_MARGIN: f32 = 12.0;

pub fn top_anchor_offset() -> Vec2 {
    Vec2::new(0.0, DIALOG_TOP_OFFSET)
}

pub fn top_anchored_rect(screen: Rect, requested_size: Vec2) -> Rect {
    let top = (screen.top() + DIALOG_TOP_OFFSET).min(screen.bottom());
    let max_width = (screen.width() - DIALOG_EDGE_MARGIN * 2.0).max(0.0);
    let max_height = (screen.bottom() - top - DIALOG_EDGE_MARGIN).max(0.0);
    let size = Vec2::new(
        requested_size.x.max(0.0).min(max_width),
        requested_size.y.max(0.0).min(max_height),
    );
    let min = Pos2::new(screen.center().x - size.x / 2.0, top);
    Rect::from_min_size(min, size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialog_top_stays_fixed_when_content_height_changes() {
        let screen = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(1000.0, 800.0));
        let short = top_anchored_rect(screen, Vec2::new(400.0, 180.0));
        let tall = top_anchored_rect(screen, Vec2::new(400.0, 520.0));

        assert_eq!(short.top(), screen.top() + DIALOG_TOP_OFFSET);
        assert_eq!(tall.top(), short.top());
        assert_eq!(tall.left(), short.left());
    }

    #[test]
    fn dialog_size_clamps_down_without_moving_its_top_edge() {
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(640.0, 480.0));
        let rect = top_anchored_rect(screen, Vec2::new(900.0, 900.0));

        assert_eq!(rect.top(), DIALOG_TOP_OFFSET);
        assert_eq!(rect.width(), screen.width() - DIALOG_EDGE_MARGIN * 2.0);
        assert_eq!(
            rect.height(),
            screen.bottom() - DIALOG_TOP_OFFSET - DIALOG_EDGE_MARGIN
        );
    }
}

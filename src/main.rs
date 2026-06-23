#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod git;
mod graph;
mod i18n;
mod theme;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Git Agent")
            .with_icon(app_icon_data())
            .with_decorations(false)
            .with_inner_size([1360.0, 860.0])
            .with_min_inner_size([980.0, 640.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Git Agent",
        options,
        Box::new(|cc| Ok(Box::new(app::GitAgentApp::new(cc)))),
    )
}

fn app_icon_data() -> eframe::egui::IconData {
    let size = 64;
    let mut rgba = vec![0_u8; size * size * 4];
    let teal = [69, 238, 216, 255];

    paint_line_color(&mut rgba, size, 24, 16, 24, 48, 4, teal);
    paint_quadratic_color(
        &mut rgba,
        size,
        (24.0, 34.0),
        (36.0, 34.0),
        (45.0, 21.0),
        4,
        teal,
    );

    for (x, y) in [(24, 16), (24, 48), (45, 21)] {
        paint_ring(&mut rgba, size, x, y, 9, 4, teal);
        clear_disc(&mut rgba, size, x, y, 5);
    }

    eframe::egui::IconData {
        rgba,
        width: size as u32,
        height: size as u32,
    }
}

fn paint_ring(
    rgba: &mut [u8],
    size: usize,
    cx: usize,
    cy: usize,
    radius: usize,
    width: usize,
    color: [u8; 4],
) {
    let outer_sq = (radius * radius) as isize;
    let inner = radius.saturating_sub(width);
    let inner_sq = (inner * inner) as isize;
    for y in cy.saturating_sub(radius)..=(cy + radius).min(size - 1) {
        for x in cx.saturating_sub(radius)..=(cx + radius).min(size - 1) {
            let dx = x as isize - cx as isize;
            let dy = y as isize - cy as isize;
            let dist_sq = dx * dx + dy * dy;
            if dist_sq <= outer_sq && dist_sq >= inner_sq {
                paint_pixel(rgba, size, x, y, color);
            }
        }
    }
}

fn clear_disc(rgba: &mut [u8], size: usize, cx: usize, cy: usize, radius: usize) {
    let radius_sq = (radius * radius) as isize;
    for y in cy.saturating_sub(radius)..=(cy + radius).min(size - 1) {
        for x in cx.saturating_sub(radius)..=(cx + radius).min(size - 1) {
            let dx = x as isize - cx as isize;
            let dy = y as isize - cy as isize;
            if dx * dx + dy * dy <= radius_sq {
                paint_pixel(rgba, size, x, y, [0, 0, 0, 0]);
            }
        }
    }
}

fn paint_line_color(
    rgba: &mut [u8],
    size: usize,
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
    radius: usize,
    color: [u8; 4],
) {
    let steps = x0.abs_diff(x1).max(y0.abs_diff(y1)).max(1);
    for step in 0..=steps {
        let t = step as f32 / steps as f32;
        let x = (x0 as f32 + (x1 as f32 - x0 as f32) * t).round() as usize;
        let y = (y0 as f32 + (y1 as f32 - y0 as f32) * t).round() as usize;
        paint_disc(rgba, size, x, y, radius, color);
    }
}

fn paint_quadratic_color(
    rgba: &mut [u8],
    size: usize,
    start: (f32, f32),
    control: (f32, f32),
    end: (f32, f32),
    radius: usize,
    color: [u8; 4],
) {
    let steps = 48;
    for step in 0..=steps {
        let t = step as f32 / steps as f32;
        let one_minus_t = 1.0 - t;
        let x =
            one_minus_t * one_minus_t * start.0 + 2.0 * one_minus_t * t * control.0 + t * t * end.0;
        let y =
            one_minus_t * one_minus_t * start.1 + 2.0 * one_minus_t * t * control.1 + t * t * end.1;
        paint_disc(
            rgba,
            size,
            x.round() as usize,
            y.round() as usize,
            radius,
            color,
        );
    }
}

fn paint_disc(rgba: &mut [u8], size: usize, cx: usize, cy: usize, radius: usize, color: [u8; 4]) {
    let radius_sq = (radius * radius) as isize;
    for y in cy.saturating_sub(radius)..=(cy + radius).min(size - 1) {
        for x in cx.saturating_sub(radius)..=(cx + radius).min(size - 1) {
            let dx = x as isize - cx as isize;
            let dy = y as isize - cy as isize;
            if dx * dx + dy * dy <= radius_sq {
                paint_pixel(rgba, size, x, y, color);
            }
        }
    }
}

fn paint_pixel(rgba: &mut [u8], size: usize, x: usize, y: usize, color: [u8; 4]) {
    let idx = (y * size + x) * 4;
    rgba[idx] = color[0];
    rgba[idx + 1] = color[1];
    rgba[idx + 2] = color[2];
    rgba[idx + 3] = color[3];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_icon_has_custom_git_mark() {
        let icon = app_icon_data();
        assert_eq!(icon.width, 64);
        assert_eq!(icon.height, 64);
        assert!(
            icon.rgba
                .chunks_exact(4)
                .any(|px| px == [69, 238, 216, 255])
        );
        assert!(icon.rgba.chunks_exact(4).any(|px| px[3] == 0));
        let logo = include_str!("../assets/icons/logo-ga.svg");
        assert!(logo.contains("stroke=\"#45EED8\""));
        assert!(logo.contains("<circle cx=\"45\" cy=\"21\""));
        assert!(logo.contains("fill=\"none\""));
        assert!(include_str!("main.rs").contains("with_decorations(false)"));
    }
}

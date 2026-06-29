#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

fn main() -> eframe::Result<()> {
    install_panic_logger();
    append_app_log(format!(
        "process start pid={} exe={} cwd={}",
        std::process::id(),
        std::env::current_exe()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|error| format!("<current_exe error: {error}>")),
        std::env::current_dir()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|error| format!("<current_dir error: {error}>"))
    ));

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Git Agent")
            .with_icon(app_icon_data())
            .with_decorations(false)
            .with_inner_size([1360.0, 860.0])
            .with_min_inner_size([980.0, 640.0]),
        ..Default::default()
    };

    let result = eframe::run_native(
        "Git Agent",
        options,
        Box::new(|cc| Ok(Box::new(git_agent::app::GitAgentApp::new(cc)))),
    );
    append_app_log(format!(
        "run_native returned {}",
        match &result {
            Ok(()) => "ok".to_owned(),
            Err(error) => format!("error: {error}"),
        }
    ));
    result
}

fn install_panic_logger() {
    std::panic::set_hook(Box::new(|info| {
        append_app_log(format!("panic: {info}"));
    }));
}

fn append_app_log(message: impl AsRef<str>) {
    let Some(path) = app_log_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "time-before-epoch".to_owned());
    let line = format!("[{timestamp}] {}\n", message.as_ref());
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut file| std::io::Write::write_all(&mut file, line.as_bytes()));
}

fn app_log_path() -> Option<std::path::PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(std::path::PathBuf::from))
        .or_else(|| std::env::current_dir().ok())
        .map(|base| base.join("data").join("app.log"))
}

fn app_icon_data() -> eframe::egui::IconData {
    let size = 64;
    let mut rgba = vec![0_u8; size * size * 4];
    let green = [21, 196, 151, 255];
    let blue = [47, 111, 234, 255];

    paint_line_color(&mut rgba, size, 23, 17, 23, 47, 4, blue);
    paint_line_color(&mut rgba, size, 23, 17, 23, 30, 4, green);
    paint_quadratic_color(
        &mut rgba,
        size,
        (23.0, 31.0),
        (31.0, 31.0),
        (42.0, 22.0),
        4,
        blue,
    );

    paint_ring(&mut rgba, size, 23, 17, 8, 4, green);
    clear_disc(&mut rgba, size, 23, 17, 4);
    paint_ring(&mut rgba, size, 23, 47, 8, 4, blue);
    clear_disc(&mut rgba, size, 23, 47, 4);
    paint_ring(&mut rgba, size, 42, 22, 8, 4, blue);
    clear_disc(&mut rgba, size, 42, 22, 4);

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
                .any(|px| px == [21, 196, 151, 255])
        );
        assert!(
            icon.rgba
                .chunks_exact(4)
                .any(|px| px == [47, 111, 234, 255])
        );
        assert_eq!(&icon.rgba[0..4], &[0, 0, 0, 0]);
        let logo = include_str!("../assets/icons/logo-ga.svg");
        assert!(!logo.contains("fill=\"#FFFFFF\""));
        assert!(!logo.contains("fill-opacity"));
        assert!(!logo.contains("<rect"));
        assert!(logo.contains("stroke=\"#15C497\""));
        assert!(logo.contains("stroke=\"#2F6FEA\""));
        assert!(logo.contains("<circle cx=\"42\" cy=\"22\""));
        assert!(logo.contains("fill=\"none\""));
        assert!(include_str!("main.rs").contains("with_decorations(false)"));
    }

    #[test]
    fn main_installs_file_logging_for_startup_and_panics() {
        let source = include_str!("main.rs");

        assert!(source.contains("fn app_log_path()"));
        assert!(source.contains("\"app.log\""));
        assert!(source.contains("std::panic::set_hook"));
        assert!(source.contains("process start"));
        assert!(source.contains("panic:"));
        assert!(source.contains("run_native returned"));
    }
}

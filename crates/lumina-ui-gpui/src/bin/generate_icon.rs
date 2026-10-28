use image::{imageops::FilterType, Rgba, RgbaImage};
use std::fs;
use std::path::Path;
use std::process::Command;

fn main() -> anyhow::Result<()> {
    println!("Generating Lumina macOS Retina App Icon...");

    let size = 1024u32;
    let mut master = RgbaImage::new(size, size);

    let cx = 512.0f64;
    let cy = 512.0f64;
    let radius = 460.0f64;
    let corner_radius = 210.0f64;

    // Draw macOS Squircle icon with gradient background
    for y in 0..size {
        for x in 0..size {
            let px = x as f64;
            let py = y as f64;

            // Distance to squircle box
            let dx = (px - cx).abs() - (radius - corner_radius);
            let dy = (py - cy).abs() - (radius - corner_radius);
            let dist = if dx > 0.0 && dy > 0.0 {
                (dx * dx + dy * dy).sqrt() - corner_radius
            } else if dx > 0.0 {
                dx - corner_radius
            } else if dy > 0.0 {
                dy - corner_radius
            } else {
                -corner_radius
            };

            if dist > 1.0 {
                // Outside squircle
                continue;
            }

            let alpha = if dist <= 0.0 {
                1.0
            } else {
                1.0 - dist // Anti-aliasing
            };

            // Deep space navy / dark indigo gradient
            let t = py / 1024.0;
            let r_base = (14.0 * (1.0 - t) + 8.0 * t) as u8;
            let g_base = (24.0 * (1.0 - t) + 12.0 * t) as u8;
            let b_base = (48.0 * (1.0 - t) + 24.0 * t) as u8;

            let mut r = r_base as f64;
            let mut g = g_base as f64;
            let mut b = b_base as f64;

            // Subtle border glow
            if dist > -8.0 && dist <= 0.0 {
                let border_t = (dist + 8.0) / 8.0;
                r += 40.0 * border_t;
                g += 80.0 * border_t;
                b += 160.0 * border_t;
            }

            // Radial distance from center
            let r_dist = ((px - cx) * (px - cx) + (py - cy) * (py - cy)).sqrt();

            // Circular camera lens / aperture ring (radius 220 to 320)
            if (220.0..=330.0).contains(&r_dist) {
                let ring_t = ((r_dist - 275.0) / 55.0).abs();
                let intensity = (1.0 - ring_t).clamp(0.0, 1.0);
                r = r * (1.0 - intensity * 0.7) + 37.0 * intensity * 0.7;
                g = g * (1.0 - intensity * 0.7) + 99.0 * intensity * 0.7;
                b = b * (1.0 - intensity * 0.7) + 235.0 * intensity * 0.7;
            }

            // Aperture blade lines (8 blades)
            let angle = (py - cy).atan2(px - cx);
            let blade_mod = (angle * 4.0).sin().abs();
            if r_dist < 260.0 && r_dist > 120.0 && blade_mod > 0.85 {
                r += 30.0 * blade_mod;
                g += 60.0 * blade_mod;
                b += 120.0 * blade_mod;
            }

            // Central 4-pointed radiant star / Lumina sparkle
            let star_dx = (px - cx).abs();
            let star_dy = (py - cy).abs();
            let _star_power = (star_dx * star_dy).powf(0.65);
            let star_diag = (star_dx + star_dy) / 2.0;

            if star_dx < 120.0 && star_dy < 12.0 {
                let glow = (1.0 - star_dx / 120.0) * (1.0 - star_dy / 12.0);
                r += 240.0 * glow;
                g += 200.0 * glow;
                b += 120.0 * glow;
            }
            if star_dy < 120.0 && star_dx < 12.0 {
                let glow = (1.0 - star_dy / 120.0) * (1.0 - star_dx / 12.0);
                r += 240.0 * glow;
                g += 200.0 * glow;
                b += 120.0 * glow;
            }
            if star_diag < 50.0 {
                let glow = 1.0 - star_diag / 50.0;
                r += 255.0 * glow;
                g += 230.0 * glow;
                b += 180.0 * glow;
            }

            // Center glow core
            if r_dist < 60.0 {
                let core_glow = (1.0 - r_dist / 60.0).powi(2);
                r += 255.0 * core_glow;
                g += 255.0 * core_glow;
                b += 255.0 * core_glow;
            }

            let r_final = (r.clamp(0.0, 255.0) * alpha) as u8;
            let g_final = (g.clamp(0.0, 255.0) * alpha) as u8;
            let b_final = (b.clamp(0.0, 255.0) * alpha) as u8;
            let a_final = (alpha * 255.0) as u8;

            master.put_pixel(x, y, Rgba([r_final, g_final, b_final, a_final]));
        }
    }

    // Target iconset folder
    let iconset_dir = Path::new("target/Lumina.iconset");
    let resources_dir = Path::new("resources");
    fs::create_dir_all(iconset_dir)?;
    fs::create_dir_all(resources_dir)?;

    let sizes = [
        ("icon_16x16.png", 16),
        ("icon_16x16@2x.png", 32),
        ("icon_32x32.png", 32),
        ("icon_32x32@2x.png", 64),
        ("icon_128x128.png", 128),
        ("icon_128x128@2x.png", 256),
        ("icon_256x256.png", 256),
        ("icon_256x256@2x.png", 512),
        ("icon_512x512.png", 512),
        ("icon_512x512@2x.png", 1024),
    ];

    for (name, s) in sizes {
        let resized = if s == 1024 {
            master.clone()
        } else {
            image::imageops::resize(&master, s, s, FilterType::Lanczos3)
        };
        let out_path = iconset_dir.join(name);
        resized.save(&out_path)?;
    }

    // Also save a preview 512x512 PNG in resources
    let preview_path = resources_dir.join("app_icon.png");
    image::imageops::resize(&master, 512, 512, FilterType::Lanczos3).save(&preview_path)?;

    // Run iconutil to produce Lumina.icns
    let icns_path = resources_dir.join("Lumina.icns");
    let status = Command::new("iconutil")
        .args(["-c", "icns", "target/Lumina.iconset", "-o"])
        .arg(&icns_path)
        .status()?;

    if status.success() {
        println!("Successfully generated {:?}", icns_path);
    } else {
        eprintln!("iconutil exited with status {:?}", status);
    }

    Ok(())
}

//! Harnais de benchmark sans dépendance pour le moteur KaractiRage.
//!
//! Mesure les phases individuelles de la boucle de frame (simulation, rastérisation,
//! encodage RGBA) ainsi que les primitives de rendu les plus chaudes, afin de
//! chiffrer chaque optimisation plutôt que de la supposer.
//!
//! Exécution : `cargo bench -p engine-core --bench engine_bench`

use std::hint::black_box;
use std::time::Instant;

use karactirage_engine::native_api::{NativeEngine, CANVAS_HEIGHT, CANVAS_WIDTH};
use karactirage_engine::renderer::Canvas;

/// Chronomètre une closure sur `iters` itérations et retourne la moyenne en microsecondes.
fn time_us<F: FnMut()>(iters: u32, mut f: F) -> f64 {
    // Échauffement
    for _ in 0..3 {
        f();
    }
    let start = Instant::now();
    for _ in 0..iters {
        black_box(f());
    }
    start.elapsed().as_secs_f64() * 1_000_000.0 / iters as f64
}

/// Exécute `samples` mesures indépendantes et retourne la meilleure (médiane basse).
fn best_of<F: FnMut() -> f64>(samples: usize, mut f: F) -> f64 {
    let mut results: Vec<f64> = (0..samples).map(|_| f()).collect();
    results.sort_by(|a, b| a.partial_cmp(b).unwrap());
    results[results.len() / 2]
}

fn main() {
    println!("==========================================================");
    println!("  KARACTIRAGE — Benchmark moteur (scène level_01)");
    println!("==========================================================\n");

    let mut engine = NativeEngine::init().expect("échec d'initialisation du moteur");

    let dt = 1.0 / 60.0;
    let iters: u32 = 60;

    // --- Phases de la frame (réutilise la même instance) ---------------------
    let update_us = best_of(5, || {
        time_us(iters, || {
            engine.game.update(dt, &engine.input);
            engine.input.clear_transient();
        })
    });

    let render_us = best_of(5, || {
        time_us(iters, || {
            engine.game.render(&mut engine.canvas);
        })
    });

    let transport_us = best_of(5, || {
        time_us(iters, || {
            engine.frame_bytes();
        })
    });

    let full_frame_us = best_of(5, || {
        time_us(iters, || {
            engine.game.update(dt, &engine.input);
            engine.input.clear_transient();
            engine.game.render(&mut engine.canvas);
            engine.frame_bytes();
        })
    });

    println!("--- Phases de la frame (µs / frame, médiane) ---");
    println!("  update   : {:>10.1} µs   (cible ≤ 200 µs)", update_us);
    println!("  render   : {:>10.1} µs   (cible ≤ 2 500 µs)", render_us);
    println!("  transport: {:>10.1} µs   (encode RGBA mono-passe)", transport_us);
    println!("  TOTAL    : {:>10.1} µs   (budget 60 FPS = 16 666 µs)", full_frame_us);
    println!();

    // --- Micro-benchmarks des primitives de rendu ----------------------------
    let mut canvas = Canvas::new(CANVAS_WIDTH, CANVAS_HEIGHT);

    let circle_us = best_of(5, || {
        time_us(200, || {
            canvas.draw_circle(600, 400, 42, 0x00221306);
        })
    });

    let blend_us = best_of(5, || {
        time_us(10_000, || {
            canvas.blend_pixel(600, 400, 0x00FFFFFF, 128);
        })
    });

    let text_us = best_of(5, || {
        time_us(100, || {
            canvas.draw_text_mono(24, 24, "KaractiRage — moteur 2D déterministe", 0x00A7F3D0, 12.0);
        })
    });

    println!("--- Primitives de rendu ---");
    println!("  draw_circle (r=42)     : {:>10.1} µs / op", circle_us);
    println!("  blend_pixel (alpha 128): {:>10.3} µs / op", blend_us);
    println!("  draw_text_mono (45 ch.) : {:>10.1} µs / op", text_us);
    println!();

    // --- Micro-benchmarks SVG (import + blit alpha + rig) ---------------------
    let svg_src = r##"<svg xmlns="http://www.w3.org/2000/svg" width="128" height="128" viewBox="0 0 128 128">
  <rect x="16" y="16" width="96" height="96" rx="8" fill="#0ea5e9"/>
  <circle cx="64" cy="64" r="32" fill="#f59e0b"/>
</svg>"##;

    let raster_us = best_of(5, || {
        time_us(20, || {
            black_box(
                karactirage_engine::vector::rasterize_svg(svg_src, Some((128, 128))).unwrap(),
            );
        })
    });

    // Blit alpha : sprite 64x64 opaque
    let sprite: Vec<u32> = (0..64 * 64).map(|i| 0xFFFF0000u32 | (i % 251) as u32).collect();
    let blit_us = best_of(5, || {
        time_us(200, || {
            canvas.draw_pixel_matrix_alpha(0, 0, 64, 64, &sprite, 64, 64);
        })
    });

    // Rig : 6 parties 24x24 en rotation légère
    let part: Vec<u32> = (0..24 * 24).map(|i| 0xFF123456u32 | (i % 7) as u32).collect();
    let rig_us = best_of(5, || {
        time_us(100, || {
            for k in 0..6 {
                let ang = (k as f32) * 0.1;
                canvas.draw_sprite_affine(
                    &part, 24, 24, 12.0, 12.0, ang, 1.0, 1.0,
                    100.0 + k as f32 * 30.0, 100.0,
                );
            }
        })
    });

    println!("--- Intégration SVG ---");
    println!("  rasterize_svg (128²)          : {:>10.1} µs / import", raster_us);
    println!("  draw_pixel_matrix_alpha (64²) : {:>10.1} µs / sprite", blit_us);
    println!("  draw_sprite_affine ×6 (rig)   : {:>10.1} µs / frame", rig_us);
    println!();

    // --- Budget 60 FPS -------------------------------------------------------
    let budget = 16_666.0;
    let margin = budget - full_frame_us;
    println!("==========================================================");
    println!(
        "  Marge 60 FPS : {:.1} µs ({:.1} % du budget)",
        margin,
        margin / budget * 100.0
    );
    println!("==========================================================");
}

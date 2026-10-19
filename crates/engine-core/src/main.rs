use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::Instant;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, Event, MouseButton, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::keyboard::PhysicalKey;
use winit::window::WindowBuilder;

mod adaptation;
mod ai_patch;
mod audio;
mod camera;
mod dialogue;
mod editor;
mod entity;
mod flocking;
mod game;
mod input;
mod inventory;
mod navigation;
mod renderer;
mod replay;
mod scene;
mod spatial;
mod telemetry;
mod exporter;
mod recipe;
pub mod ai_client;
pub mod project;

use game::GameState;
use input::InputState;
use renderer::Canvas;

const WINDOW_WIDTH: u32 = 1200;
const WINDOW_HEIGHT: u32 = 800;

fn main() {
    println!("============================================================");
    println!("⚡ KARACTIRAGE ENGINE 2D — ATELIER STUDIO UNIQUE (Dark Edition)");
    println!("============================================================");
    println!("📂  Hub Projets   : Gestionnaire de projets & scènes au démarrage");
    println!("🪄  Assistant     : Création de jeu guidée en 6 étapes [W]");
    println!("🤖  Assistant IA  : Génération par intention (Ollama, LM Studio, Claude, Gemini, DeepSeek) [I]");
    println!("🛠️  Mode Studio   : Basculer vers l'Atelier ECS & KVL [TAB]");
    println!("🎮 Mode Jeu      : Simulation déterministe à 60 FPS constants [TAB]");
    println!("📊 Télémétrie    : Profilage temps réel & budget 16.6ms [F1]");
    println!("📖 Guide Moteur  : Guide Pédagogique interactif 3 onglets [H]");
    println!("🌀 Multi-Genres  : 5 Genres en 1 clic (RPG, Plateforme, Shmup, Sokoban, Boss) [F4 / L]\n");

    let event_loop = EventLoop::new().expect("Création de l'EventLoop");
    let window = Rc::new(
        WindowBuilder::new()
            .with_title("⚡ KaractiRage Engine 2D — Studio & Runtime ECS (Dark Edition)")
            .with_inner_size(LogicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT))
            .with_min_inner_size(LogicalSize::new(1024, 640))
            .build(&event_loop)
            .expect("Création de la fenêtre"),
    );

    let context = softbuffer::Context::new(window.clone()).expect("Création du contexte softbuffer");
    let mut surface = softbuffer::Surface::new(&context, window.clone()).expect("Création de la surface");

    // Initialisation immédiate de la surface et du canvas selon la taille physique réelle et l'échelle Retina
    let init_sz = window.inner_size();
    if let (Some(w), Some(h)) = (NonZeroU32::new(init_sz.width), NonZeroU32::new(init_sz.height)) {
        let _ = surface.resize(w, h);
    }
    let init_scale = window.scale_factor().max(1.0);
    let init_logical_w = (init_sz.width as f64 / init_scale).round().max(400.0) as usize;
    let init_logical_h = (init_sz.height as f64 / init_scale).round().max(300.0) as usize;
    let mut canvas = Canvas::new(init_logical_w, init_logical_h);
    renderer::warmup_glyph_cache();

    let mut input = InputState::new();
    let mut game = GameState::load_from_scene("assets/scenes/level_01.scene.json")
        .expect("Erreur lors du chargement de la scène 'assets/scenes/level_01.scene.json'");
    game.editor.show_welcome_modal = true;
    let mut last_frame = Instant::now();

    event_loop.set_control_flow(ControlFlow::Poll);

    let _ = event_loop.run(move |event, elwt| {
        match event {
            Event::WindowEvent { event, window_id } if window_id == window.id() => match event {
                WindowEvent::CloseRequested => {
                    elwt.exit();
                }
                WindowEvent::Resized(size) => {
                    if let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) {
                        let _ = surface.resize(w, h);
                        let scale = window.scale_factor().max(1.0);
                        let logical_w = (size.width as f64 / scale).round().max(400.0) as usize;
                        let logical_h = (size.height as f64 / scale).round().max(300.0) as usize;
                        canvas.resize(logical_w, logical_h);
                    }
                }
                WindowEvent::CursorMoved { position, .. } => {
                    let scale = window.scale_factor().max(1.0);
                    input.on_mouse_move((position.x / scale) as f32, (position.y / scale) as f32);
                }
                WindowEvent::MouseInput { state, button, .. } => {
                    match button {
                        MouseButton::Left => match state {
                            ElementState::Pressed => input.on_mouse_down(),
                            ElementState::Released => input.on_mouse_up(),
                        },
                        MouseButton::Right => {
                            if state == ElementState::Pressed {
                                input.on_mouse_right_click();
                            }
                        }
                        _ => {}
                    }
                }
                WindowEvent::KeyboardInput { event: key_event, .. } => {
                    if let PhysicalKey::Code(key) = key_event.physical_key {
                        match key_event.state {
                            ElementState::Pressed => {
                                input.on_key_down(key);
                                // Capture universelle du texte produit par l'OS (AZERTY, QWERTY, accents français, chiffres, etc.)
                                if let Some(ref text) = key_event.text {
                                    for ch in text.chars() {
                                        if !ch.is_control() {
                                            input.on_char(ch);
                                        }
                                    }
                                } else if let winit::keyboard::Key::Character(ref smol) = key_event.logical_key {
                                    for ch in smol.chars() {
                                        if !ch.is_control() {
                                            input.on_char(ch);
                                        }
                                    }
                                }
                            }
                            ElementState::Released => input.on_key_up(key),
                        }
                    }
                }
                WindowEvent::RedrawRequested => {
                    // Rendu de la scène sur le canvas logiciel
                    game.render(&mut canvas);

                    // Copie et mise à l'échelle vers la surface native (ex: Retina 2x)
                    let (phys_w, phys_h) = {
                        let sz = window.inner_size();
                        (sz.width as usize, sz.height as usize)
                    };
                    if phys_w == 0 || phys_h == 0 {
                        return;
                    }

                    let sw = canvas.width;
                    let sh = canvas.height;

                    if let Ok(mut buffer) = surface.buffer_mut() {
                        let buf_len = buffer.len();
                        let expected_len = phys_w * phys_h;

                        if buf_len >= expected_len {
                            if phys_w == sw * 2 && phys_h == sh * 2 {
                                // Blit 2x optimisé pour écrans Retina
                                for sy in 0..sh {
                                    let src_row = &canvas.buffer[sy * sw..(sy + 1) * sw];
                                    let row1_offset = (sy * 2) * phys_w;
                                    let row2_offset = (sy * 2 + 1) * phys_w;
                                    for (sx, &p) in src_row.iter().enumerate() {
                                        let dx = sx * 2;
                                        buffer[row1_offset + dx] = p;
                                        buffer[row1_offset + dx + 1] = p;
                                        buffer[row2_offset + dx] = p;
                                        buffer[row2_offset + dx + 1] = p;
                                    }
                                }
                            } else if phys_w == sw && phys_h == sh {
                                let len = (sw * sh).min(buf_len).min(canvas.buffer.len());
                                buffer[..len].copy_from_slice(&canvas.buffer[..len]);
                            } else if sw > 0 && sh > 0 {
                                // Blit redimensionné générique avec garde-fous
                                for dy in 0..phys_h {
                                    let sy = (dy * sh) / phys_h;
                                    let src_row = &canvas.buffer[sy * sw..(sy + 1) * sw];
                                    let dst_row = &mut buffer[dy * phys_w..(dy + 1) * phys_w];
                                    for dx in 0..phys_w {
                                        let sx = (dx * sw) / phys_w;
                                        dst_row[dx] = src_row[sx];
                                    }
                                }
                            }
                        } else {
                            // Tampon en cours de resynchronisation suite à un redimensionnement
                            let safe_len = buf_len.min(canvas.buffer.len());
                            buffer[..safe_len].copy_from_slice(&canvas.buffer[..safe_len]);
                        }

                        let _ = buffer.present();
                    }
                }
                _ => {}
            },

            Event::AboutToWait => {
                let now = Instant::now();
                let dt = (now - last_frame).as_secs_f32().min(0.05);
                last_frame = now;

                // Mise à jour de la logique de jeu (Game Loop)
                game.update(dt, &input);
                input.clear_transient();

                window.request_redraw();
            }

            _ => {}
        }
    });
}

use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::Instant;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::keyboard::PhysicalKey;
use winit::window::WindowBuilder;

mod adaptation;
mod editor;
mod game;
mod input;
mod renderer;
mod scene;
mod telemetry;

use game::GameState;
use input::InputState;
use renderer::Canvas;

const WINDOW_WIDTH: u32 = 800;
const WINDOW_HEIGHT: u32 = 600;

fn main() {
    println!("=== KARACTIRAGE ENGINE 2D - DEMO INITIALE ===");
    println!("Démarrage du fenêtrage natif et du renderer de compatibilité...\n");

    let event_loop = EventLoop::new().expect("Création de l'EventLoop");
    let window = Rc::new(
        WindowBuilder::new()
            .with_title("KaractiRage 2D Engine (Low-End Profile - KVL Powered)")
            .with_inner_size(LogicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT))
            .build(&event_loop)
            .expect("Création de la fenêtre"),
    );

    let context = softbuffer::Context::new(window.clone()).expect("Création du contexte softbuffer");
    let mut surface = softbuffer::Surface::new(&context, window.clone()).expect("Création de la surface");

    let mut canvas = Canvas::new(WINDOW_WIDTH as usize, WINDOW_HEIGHT as usize);
    let mut input = InputState::new();
    let mut game = GameState::load_from_scene("assets/scenes/level_01.scene.json")
        .expect("Erreur lors du chargement de la scène 'assets/scenes/level_01.scene.json'");
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
                        surface.resize(w, h).unwrap();
                        canvas.resize(size.width as usize, size.height as usize);
                    }
                }
                WindowEvent::CursorMoved { position, .. } => {
                    input.on_mouse_move(position.x as f32, position.y as f32);
                }
                WindowEvent::MouseInput { state, .. } => {
                    match state {
                        ElementState::Pressed => input.on_mouse_down(),
                        ElementState::Released => input.on_mouse_up(),
                    }
                }
                WindowEvent::KeyboardInput { event: key_event, .. } => {
                    if let PhysicalKey::Code(key) = key_event.physical_key {
                        match key_event.state {
                            ElementState::Pressed => input.on_key_down(key),
                            ElementState::Released => input.on_key_up(key),
                        }
                    }
                }
                WindowEvent::RedrawRequested => {
                    // Rendu de la scène sur le canvas logiciel
                    game.render(&mut canvas);

                    // Copie du canvas vers le buffer de la surface native
                    if let (Some(w), Some(h)) = (
                        NonZeroU32::new(canvas.width as u32),
                        NonZeroU32::new(canvas.height as u32),
                    ) {
                        let mut buffer = surface.buffer_mut().unwrap();
                        let len = (w.get() * h.get()) as usize;
                        if buffer.len() >= len && canvas.buffer.len() >= len {
                            buffer[..len].copy_from_slice(&canvas.buffer[..len]);
                        }
                        buffer.present().unwrap();
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

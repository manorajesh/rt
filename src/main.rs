mod atlas;
mod render;
mod text;
mod config;

use render::State;
use text::Text;
use winit::application::ApplicationHandler;
use winit::event::{ ElementState, KeyEvent, WindowEvent };
use winit::event_loop::{ ActiveEventLoop, EventLoop };
use winit::keyboard::Key;
use winit::window::{ Window, WindowId };

pub async fn run() {
    let event_loop = EventLoop::new().unwrap();

    let mut window_state = StateApplication::new();
    let _ = event_loop.run_app(&mut window_state);
}

struct StateApplication<'a> {
    state: Option<State<'a>>,
    text: Option<Text>,
}

impl<'a> StateApplication<'a> {
    pub fn new() -> Self {
        Self {
            state: None,
            text: None,
        }
    }
}

impl<'a> ApplicationHandler for StateApplication<'a> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = event_loop
            .create_window(Window::default_attributes().with_title("rt"))
            .unwrap();
        self.state = Some(State::new(window));
        self.text = Some(Text::new(&self.state.as_ref().unwrap().user_config));
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent
    ) {
        let window = self.state.as_ref().unwrap().window();

        if window.id() == window_id {
            match event {
                WindowEvent::CloseRequested => {
                    event_loop.exit();
                }
                WindowEvent::Resized(physical_size) => {
                    self.state.as_mut().unwrap().resize(physical_size);
                }
                WindowEvent::RedrawRequested => {
                    self.state.as_mut().unwrap().render(self.text.as_ref().unwrap()).unwrap();
                }
                WindowEvent::KeyboardInput {
                    event: KeyEvent { logical_key: key, state: ElementState::Pressed, .. },
                    ..
                } =>
                    match key.as_ref() {
                        Key::Character(character) => {
                            self.text.as_mut().unwrap().push_str(character);
                            // self.text
                            //     .as_mut()
                            //     .unwrap()
                            //     .insert_char(0, 0, character.chars().next().unwrap());
                            window.request_redraw();
                        }
                        _ => (),
                    }
                _ => {}
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        let window = self.state.as_ref().unwrap().window();
        window.request_redraw();
    }
}

fn main() {
    env_logger::init();
    pollster::block_on(run());
}

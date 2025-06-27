// src/intro/intro_image.rs
//
// A simple module to load and display the intro image on the screen before the performance starts.
//
// The image file should be placed in the assets directory.

use nannou::{prelude::*, wgpu::Texture};

pub struct IntroImage {
    texture: Option<Texture>, // the loaded image
    image_path: String,       // the path to the image
    is_visible: bool,         // whether the image is visible
}

impl IntroImage {
    pub fn new(image_path: &str) -> Self {
        Self {
            texture: None,
            image_path: image_path.to_string(),
            is_visible: true,
        }
    }

    pub fn load(&mut self, app: &App) {
        let assets_path = app.assets_path().expect("Could not find assets directory");
        let image_path = assets_path.join(&self.image_path);

        if let Ok(texture) = Texture::from_path(app, image_path) {
            self.texture = Some(texture);
        } else {
            eprintln!("Failed to load image: {}", self.image_path);
        }
    }

    pub fn show(&mut self) {
        self.is_visible = true;
    }

    pub fn hide(&mut self) {
        self.is_visible = false;
    }

    pub fn toggle_visible(&mut self) {
        self.is_visible = !self.is_visible;
    }

    pub fn is_visible(&self) -> bool {
        self.is_visible
    }

    pub fn draw(&self, draw: &Draw, window_rect: Rect) {
        if !self.is_visible {
            return;
        }
        // Draw black background
        draw.background().color(BLACK);

        // Draw the image if loaded
        if let Some(texture) = &self.texture {
            let texture_size = texture.size();
            let width = texture_size[0];
            let height = texture_size[1];
            let scale = (window_rect.w() / width as f32).min(window_rect.h() / height as f32);

            draw.texture(texture)
                .wh(vec2(width as f32 * scale, height as f32 * scale))
                .x_y(0.0, 0.0);
        }
    }
}

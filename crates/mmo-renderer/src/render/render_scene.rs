use super::*;

impl MmoClientEngine {
    fn render_scene(&mut self) -> Camera {
        self.fb.clear_sky();
        let cam = self.current_camera();

        if self.joined {
            self.render_terrain(&cam);
            self.render_landmarks(&cam);
            self.render_forest_and_crystals(&cam);
        } else {
            self.render_lobby_room(&cam);
        }
        self.render_entities(&cam);
        self.render_vfx(&cam);
        cam
    }

    pub fn render(&mut self) {
        self.fb.set_gpu_scene_mode(false);
        let cam = self.render_scene();
        self.render_hud(&cam);
    }

    pub fn render_gpu(&mut self) {
        self.fb.set_gpu_scene_mode(true);
        let cam = self.render_scene();
        self.fb.begin_ui_capture();
        self.render_hud(&cam);
        self.fb.end_ui_capture();
        self.fb.set_gpu_scene_mode(false);
    }
}

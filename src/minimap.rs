use std::collections::VecDeque;
use std::f32::consts::{PI, TAU};
use std::path::{Path, PathBuf};

use hudhook::imgui::{Context, DrawListMut, FontSource, ImColor32, TextureId, Ui};
use hudhook::{ImguiRenderLoop, RenderContext};

use crate::config::Config;
use crate::dungeon::{self, DungeonMap};
use crate::game::{self, PointKind, Snapshot};
use crate::maptiles::{self, Loader, MapId, TILE, WINDOW};

/// Meters from the center to the rim at each zoom level.
const ZOOM_RANGES: [f32; 3] = [40.0, 80.0, 160.0];
const TRAIL_SPACING: f32 = 1.5;
const TRAIL_LENGTH: usize = 800;
const TELEPORT_DISTANCE: f32 = 150.0;
const FONT_SIZE: f32 = 13.0;
/// Radius of the map disc at 1080p.
const RADIUS: f32 = 96.0;
/// Map pixels (meters) the window must extend past the player: the widest zoom plus slack.
const WINDOW_MARGIN: f32 = ZOOM_RANGES[2] + 20.0;
/// Segments of the textured map disc.
const DISC_SEGMENTS: usize = 72;

const FRAME_PNG: &[u8] = include_bytes!("../assets/frame.png");
const ARROW_PNG: &[u8] = include_bytes!("../assets/arrow.png");
const BEACON_PNG: &[u8] = include_bytes!("../assets/marker.png");
/// Map radius as a fraction of frame.png's width: just past the widest point of its hole
/// (measured 0.387 to 0.406) so the map always reaches under the frame.
const FRAME_HOLE: f32 = 0.408;
/// Point of arrow.png it turns around: the middle of the arrowhead, not of the canvas.
const ARROW_PIVOT: [f32; 2] = [0.5, 603.0 / 1254.0];

const PALE_GOLD: [u8; 3] = [214, 196, 150];
const BRIGHT_GOLD: [u8; 3] = [255, 222, 150];
const BACKGROUND: [u8; 3] = [24, 20, 15];
const BLACK: [u8; 3] = [0, 0, 0];

#[cfg(test)]
mod dungeon_render_tests {
    use super::*;

    #[test]
    #[ignore = "Requires TOMBSWARD_NAV_TEST_FILE generated from the local game"]
    fn real_dungeon_generates_finite_imgui_draw_data_inside_frame() {
        let path = std::env::var_os("TOMBSWARD_NAV_TEST_FILE").expect("Set local fixture path");
        let map = DungeonMap::load(Path::new(&path)).unwrap();
        let local_position = std::array::from_fn(|axis| map.triangles[0].points.iter().map(|p| p[axis]).sum::<f32>()/3.0);
        let config = Config { trail: false, rotate: true, compass: true,
            hide_in_boss_fights: false, hide_in_caves: true, dungeon_mode: true,
            dungeon_height_band: 2.0, dungeon_floor_key: None, size: 1.0,
            margin: 24.0, toggle_key: None, zoom_key: None };
        let mut minimap = Minimap::new(Path::new("missing-test-game-directory"), config, Some(map));
        minimap.alpha = 1.0;
        let player = Snapshot { block_id: dungeon::PILOT_BLOCK, local_position,
            position: [local_position[0],local_position[2]], space: 30, map: None,
            facing: [0.,1.], camera_right: [1.,0.], camera_forward: [0.,1.],
            fade: 0., hud_visible: true, boss_fight: false, in_cave: true,
            graces: vec![], beacons: vec![], points: vec![] };
        assert!(minimap.dungeon_active(&player));
        let mut context = Context::create();
        context.set_ini_filename(None);
        context.io_mut().display_size = [800.,600.];
        context.io_mut().delta_time = 1./60.;
        context.fonts().build_rgba32_texture();
        for all in [false,true] {
            minimap.dungeon_all_floors = all;
            {
                let ui = context.frame();
                let draw = ui.get_foreground_draw_list();
                minimap.draw_dungeon(&draw,&player,[400.,300.],96.,1.);
            }
            let data = context.render();
            assert!(data.total_vtx_count > 0 && data.total_idx_count > 0);
            for list in data.draw_lists() {
                for v in list.vtx_buffer() {
                    assert!(v.pos.iter().all(|x| x.is_finite()));
                    let d = [v.pos[0]-400.,v.pos[1]-300.];
                    assert!(d[0]*d[0]+d[1]*d[1] <= 98.*98.);
                }
            }
        }
    }
}

pub struct Minimap {
    dungeon: Option<DungeonMap>,
    dungeon_all_floors: bool,
    config: Config,
    loader: Loader,
    frame: Option<TextureId>,
    arrow: Option<TextureId>,
    beacon: Option<TextureId>,
    map_texture: Option<TextureId>,
    /// What `map_texture` currently shows.
    map_window: Option<(MapId, [i32; 2])>,
    requested_window: Option<(MapId, [i32; 2])>,
    font_display_height: f32,
    last: Option<Snapshot>,
    camera_forward: [f32; 2],
    camera_right: [f32; 2],
    trail: VecDeque<[f32; 2]>,
    trail_space: i32,
    visible: bool,
    zoom: usize,
    alpha: f32,
    /// Displayed facing as a world angle, where 0 is north (+Z).
    facing_yaw: Option<f32>,
}

impl Minimap {
    pub fn new(game_dir: &Path, config: Config, dungeon: Option<DungeonMap>) -> Self {
        Self {
            dungeon,
            dungeon_all_floors: false,
            config,
            loader: Loader::spawn(game_dir),
            frame: None,
            arrow: None,
            beacon: None,
            map_texture: None,
            map_window: None,
            requested_window: None,
            font_display_height: 1080.0,
            last: None,
            camera_forward: [0.0, 1.0],
            camera_right: [1.0, 0.0],
            trail: VecDeque::new(),
            trail_space: 0,
            visible: true,
            zoom: 1,
            alpha: 0.0,
            facing_yaw: None,
        }
    }
}

impl ImguiRenderLoop for Minimap {
    fn initialize<'a>(&'a mut self, ctx: &mut Context, render_context: &'a mut dyn RenderContext) {
        ctx.set_ini_filename(None);
        self.font_display_height = ctx.io().display_size[1];
        // The first font added becomes the default one.
        let path = PathBuf::from(std::env::var_os("WINDIR").unwrap_or_default())
            .join(r"Fonts\georgiab.ttf");
        if let Ok(data) = std::fs::read(path) {
            ctx.fonts().add_font(&[FontSource::TtfData {
                data: &data,
                size_pixels: FONT_SIZE * self.font_display_height / 1080.0,
                config: None,
            }]);
        }
        self.frame = load_png(render_context, FRAME_PNG);
        self.arrow = load_png(render_context, ARROW_PNG);
        self.beacon = load_png(render_context, BEACON_PNG);
    }

    fn before_render<'a>(
        &'a mut self,
        ctx: &mut Context,
        render_context: &'a mut dyn RenderContext,
    ) {
        // The font is baked once, so follow later resolution changes by scaling it.
        let height = ctx.io().display_size[1];
        ctx.io_mut().font_global_scale = height / self.font_display_height * self.config.size;

        // hudhook uploads on the game's queue and waits for the GPU. On loading screens that
        // queue can be waiting on the game, which deadlocks it.
        if !self.last.as_ref().is_some_and(in_control) {
            return;
        }
        let Some(window) = self.loader.poll() else {
            return;
        };
        let size = WINDOW as u32;
        let uploaded = match self.map_texture {
            Some(texture) => render_context
                .replace_texture(texture, &window.rgba, size, size)
                .is_ok(),
            None => match render_context.load_texture(&window.rgba, size, size) {
                Ok(texture) => {
                    self.map_texture = Some(texture);
                    true
                }
                Err(_) => false,
            },
        };
        if uploaded {
            self.map_window = Some((window.map, window.origin));
        } else {
            // Ask again next frame instead of waiting until the player nears the window's edge.
            self.requested_window = None;
        }
    }

    fn render(&mut self, ui: &mut Ui) {
        let pressed = |key: Option<_>| key.is_some_and(|key| ui.is_key_pressed_no_repeat(key));
        if pressed(self.config.toggle_key) {
            self.visible = !self.visible;
        }
        if pressed(self.config.zoom_key) {
            self.zoom = (self.zoom + 1) % ZOOM_RANGES.len();
        }
        if pressed(self.config.dungeon_floor_key) {
            self.dungeon_all_floors = !self.dungeon_all_floors;
        }

        let snapshot = game::LATEST.lock().unwrap().clone();
        if let Some(snapshot) = &snapshot {
            self.update(snapshot);
        }
        // Characters turn instantly, so the arrow eases toward the facing in world space,
        // which keeps it from lagging behind camera turns.
        if let Some([x, z]) = snapshot.as_ref().and_then(|s| normalize(s.facing)) {
            let target = x.atan2(z);
            let current = self.facing_yaw.get_or_insert(target);
            *current += wrap_angle(target - *current) * (ui.io().delta_time * 14.0).min(1.0);
        }
        // Menus like the pause menu and world map draw where the minimap sits.
        let target = match &snapshot {
            Some(snapshot)
                if self.visible
                    && in_control(snapshot)
                    && !(self.config.hide_in_boss_fights && snapshot.boss_fight)
                    && !(self.config.hide_in_caves && snapshot.in_cave && !self.dungeon_active(snapshot)) =>
            {
                1.0
            }
            _ => 0.0,
        };
        self.alpha += (target - self.alpha) * (ui.io().delta_time * 12.0).min(1.0);
        // The last known state keeps drawing while the map fades out.
        if self.alpha < 0.01 {
            return;
        }
        if let Some(last) = &self.last {
            self.draw(ui, last);
        }
    }
}

impl Minimap {
    fn dungeon_active(&self, player: &Snapshot) -> bool {
        player.local_position.iter().all(|v| v.is_finite())
            && self.dungeon.as_ref().is_some_and(|map| map.block == player.block_id)
    }

    fn update(&mut self, snapshot: &Snapshot) {
        self.last = Some(snapshot.clone());
        // Without rotation the basis stays at its north-up default.
        if self.config.rotate {
            if let Some(forward) = normalize(snapshot.camera_forward) {
                self.camera_forward = forward;
            }
            if let Some(right) = normalize(snapshot.camera_right) {
                self.camera_right = right;
            }
        }

        // hudhook leaks the staging buffer of every upload, so only re-center the window near
        // its edge instead of on every cell crossing.
        if let Some(map) = snapshot.map.filter(|_| !self.dungeon_active(snapshot)) {
            let pixel = maptiles::map_pixel(snapshot.position);
            let near_edge = match self.requested_window {
                Some((requested_map, origin)) if requested_map == map => (0..2).any(|axis| {
                    let offset = pixel[axis] - (origin[axis] * TILE as i32) as f32;
                    !(WINDOW_MARGIN..WINDOW as f32 - WINDOW_MARGIN).contains(&offset)
                }),
                _ => true,
            };
            if near_edge {
                let cell = pixel.map(|p| (p / TILE as f32).floor() as i32);
                let wanted = (map, [cell[0] - 1, cell[1] - 1]);
                self.loader.request(wanted.0, wanted.1);
                self.requested_window = Some(wanted);
            }
        }

        if snapshot.space != self.trail_space {
            self.trail.clear();
            self.trail_space = snapshot.space;
        }
        if let Some(&last) = self.trail.back() {
            let distance = length([
                snapshot.position[0] - last[0],
                snapshot.position[1] - last[1],
            ]);
            if distance > TELEPORT_DISTANCE {
                self.trail.clear();
            } else if distance < TRAIL_SPACING {
                return;
            }
        }
        self.trail.push_back(snapshot.position);
        if self.trail.len() > TRAIL_LENGTH {
            self.trail.pop_front();
        }
    }

    /// Turns a horizontal world direction into a screen direction. The map turns with
    /// the camera: whatever the camera faces is up.
    fn to_screen(&self, [dx, dz]: [f32; 2]) -> [f32; 2] {
        let [rx, rz] = self.camera_right;
        let [fx, fz] = self.camera_forward;
        [dx * rx + dz * rz, -(dx * fx + dz * fz)]
    }

    /// The inverse of `to_screen`, since camera right and forward are orthonormal on the ground.
    fn to_world(&self, [sx, sy]: [f32; 2]) -> [f32; 2] {
        let [rx, rz] = self.camera_right;
        let [fx, fz] = self.camera_forward;
        [sx * rx - sy * fx, sx * rz - sy * fz]
    }

    /// Screen direction of a compass bearing, where 0 is north (+Z) and 90 is east (+X).
    fn bearing(&self, degrees: f32) -> [f32; 2] {
        let (sin, cos) = degrees.to_radians().sin_cos();
        self.to_screen([sin, cos])
    }

    fn color(&self, [r, g, b]: [u8; 3], opacity: f32) -> ImColor32 {
        ImColor32::from_rgba(
            r,
            g,
            b,
            ((opacity * self.alpha).clamp(0.0, 1.0) * 255.0) as u8,
        )
    }

    fn draw(&self, ui: &Ui, player: &Snapshot) {
        let [width, height] = ui.io().display_size;
        let screen_scale = height / 1080.0;
        let s = screen_scale * self.config.size;
        let radius = RADIUS * s;
        let frame_size = radius / FRAME_HOLE;
        let margin = self.config.margin * screen_scale;
        let center = [width - margin - frame_size / 2.0, margin + frame_size / 2.0];
        let draw_list = ui.get_foreground_draw_list();

        draw_list
            .add_circle(center, frame_size * 0.47, self.color(BLACK, 0.35))
            .filled(true)
            .num_segments(96)
            .build();
        let dungeon_active = self.dungeon_active(player);
        if dungeon_active || !self.draw_map(&draw_list, player, center, radius) {
            draw_list
                .add_circle(center, radius, self.color(BACKGROUND, 0.9))
                .filled(true)
                .num_segments(96)
                .build();
        }
        if dungeon_active {
            self.draw_dungeon(&draw_list, player, center, radius, s);
        }
        // Darken toward the rim so the frame sits on the map instead of cutting it.
        for i in 0..4 {
            draw_list
                .add_circle(
                    center,
                    radius - (6.0 + i as f32 * 3.0) * s,
                    self.color(BLACK, 0.22 - i as f32 * 0.05),
                )
                .num_segments(96)
                .thickness(4.0 * s)
                .build();
        }

        if self.config.trail && !dungeon_active {
            self.draw_trail(&draw_list, player, center, radius, s);
        }
        if !dungeon_active {
            self.draw_points(&draw_list, player, center, radius, s);
            self.draw_graces(&draw_list, player, center, radius, s);
            self.draw_beacons(ui, &draw_list, player, center, radius, s);
        }

        let letters = if self.config.compass {
            ["N", "E", "S", "W"].as_slice()
        } else {
            &[]
        };
        for (i, letter) in letters.iter().enumerate() {
            let direction = self.bearing(i as f32 * 90.0);
            let position = [
                center[0] + direction[0] * (radius - 14.0 * s),
                center[1] + direction[1] * (radius - 14.0 * s),
            ];
            draw_list
                .add_circle(position, 9.0 * s, self.color(BACKGROUND, 0.75))
                .filled(true)
                .build();
            let color = if i == 0 {
                self.color(BRIGHT_GOLD, 1.0)
            } else {
                self.color(PALE_GOLD, 0.9)
            };
            self.draw_text_centered(ui, &draw_list, position, color, letter);
        }

        self.draw_arrow(&draw_list, center, s);
        if dungeon_active {
            let label = if self.dungeon_all_floors { "Tombsward | all floors" } else { "Tombsward | nearby floor" };
            self.draw_text_centered(ui, &draw_list, [center[0], center[1] + radius + 30.0*s],
                self.color(PALE_GOLD, 1.0), label);
        }

        if let Some(frame) = self.frame {
            let half = frame_size / 2.0;
            draw_list
                .add_image(
                    frame,
                    [center[0] - half, center[1] - half],
                    [center[0] + half, center[1] + half],
                )
                .col(self.color([255, 255, 255], 1.0))
                .build();
        }
    }

    fn draw_dungeon(&self, draw: &DrawListMut, player: &Snapshot, center: [f32; 2], radius: f32, s: f32) {
        let Some(map) = &self.dungeon else { return; };
        let range = dungeon::ZOOM[self.zoom];
        let scale = radius / range;
        let floor = map.floor_height(player.local_position);
        let low = floor - self.config.dungeon_height_band;
        let high = floor + self.config.dungeon_height_band;
        let project = |p: [f32; 3]| {
            self.to_screen([p[0] - player.local_position[0], p[2] - player.local_position[2]]).map(|v| v * scale)
        };
        for triangle in &map.triangles {
            if (0..2).any(|axis| {
                let a = if axis == 0 { 0 } else { 2 };
                triangle.points.iter().all(|p| p[a] < player.local_position[a]-range) ||
                triangle.points.iter().all(|p| p[a] > player.local_position[a]+range)
            }) { continue; }
            let polygon = if self.dungeon_all_floors { triangle.points.to_vec() }
                else { dungeon::clip_height(&triangle.points, low, high) };
            if polygon.len() < 3 { continue; }
            let screen: Vec<_> = polygon.into_iter().map(project).collect();
            let mut clipped = dungeon::clip_disc(&screen, radius - s);
            if clipped.len() < 3 { continue; }
            // ImGui filled paths expect clockwise winding in screen coordinates.
            let area: f32 = (0..clipped.len()).map(|i| {
                let a=clipped[i]; let b=clipped[(i+1)%clipped.len()]; a[0]*b[1]-b[0]*a[1]
            }).sum();
            if area < 0.0 { clipped.reverse(); }
            for point in &mut clipped { point[0] += center[0]; point[1] += center[1]; }
            let tone = (triangle.section % 3) as u8 * 7;
            draw.add_polyline(clipped, self.color([132+tone, 119+tone, 89+tone], 1.0)).filled(true).build();
        }
        for &boundary in &map.boundaries {
            let line = if self.dungeon_all_floors { Some(boundary) }
                else { dungeon::clip_segment_height(boundary, low, high) };
            let Some(line) = line else { continue; };
            let Some(mut line) = dungeon::clip_segment_disc(line.map(project), radius-s) else { continue; };
            for p in &mut line { p[0]+=center[0]; p[1]+=center[1]; }
            draw.add_line(line[0], line[1], self.color(PALE_GOLD, 0.9)).thickness(1.2*s).build();
        }
    }

    /// A fan of wedges clips the texture to a circle and gives each rim vertex its own
    /// rotated UV. Returns false when there's no map for this spot.
    fn draw_map(
        &self,
        draw_list: &DrawListMut,
        player: &Snapshot,
        center: [f32; 2],
        radius: f32,
    ) -> bool {
        let (Some(texture), Some((map, origin)), Some(player_map)) =
            (self.map_texture, self.map_window, player.map)
        else {
            return false;
        };
        if map != player_map {
            return false;
        }
        let meters_per_pixel = ZOOM_RANGES[self.zoom] / radius;
        let player_pixel = maptiles::map_pixel(player.position);
        let uv = |screen_offset: [f32; 2]| {
            let world = self.to_world(screen_offset.map(|d| d * meters_per_pixel));
            let x = player_pixel[0] + world[0] - (origin[0] * TILE as i32) as f32;
            let y = player_pixel[1] + world[1] - (origin[1] * TILE as i32) as f32;
            [x / WINDOW as f32, 1.0 - y / WINDOW as f32]
        };
        let center_uv = uv([0.0, 0.0]);
        // A window from somewhere else would draw the wrong place until the new one arrives.
        if !center_uv.iter().all(|c| (0.0..=1.0).contains(c)) {
            return false;
        }
        let tint = self.color([236, 226, 210], 1.0);
        for i in 0..DISC_SEGMENTS {
            let rim = |k: usize| {
                let (sin, cos) = (k as f32 / DISC_SEGMENTS as f32 * TAU).sin_cos();
                [cos * radius, sin * radius]
            };
            let (a, b) = (rim(i), rim(i + 1));
            let (pa, pb) = (
                [center[0] + a[0], center[1] + a[1]],
                [center[0] + b[0], center[1] + b[1]],
            );
            draw_list
                .add_image_quad(texture, center, pa, pb, pb)
                .uv(center_uv, uv(a), uv(b), uv(b))
                .col(tint)
                .build();
        }
        true
    }

    fn draw_trail(
        &self,
        draw_list: &DrawListMut,
        player: &Snapshot,
        center: [f32; 2],
        radius: f32,
        s: f32,
    ) {
        let pixels_per_meter = radius / ZOOM_RANGES[self.zoom];
        let limit = radius - 6.0 * s;
        let project = |point: [f32; 2]| {
            let d = self.to_screen([point[0] - player.position[0], point[1] - player.position[1]]);
            [
                center[0] + d[0] * pixels_per_meter,
                center[1] + d[1] * pixels_per_meter,
            ]
        };
        let inside = |p: [f32; 2]| length([p[0] - center[0], p[1] - center[1]]) < limit;

        let points: Vec<[f32; 2]> = self
            .trail
            .iter()
            .map(|&p| project(p))
            .chain([center])
            .collect();
        for (i, segment) in points.windows(2).enumerate() {
            if !inside(segment[0]) || !inside(segment[1]) {
                continue;
            }
            let age = (i + 1) as f32 / (points.len() - 1) as f32;
            draw_list
                .add_line(
                    segment[0],
                    segment[1],
                    self.color([255, 236, 190], 0.10 + 0.55 * age),
                )
                .thickness(2.0 * s)
                .build();
        }
    }

    /// Dimmed once the game marks them done.
    fn draw_points(
        &self,
        draw_list: &DrawListMut,
        player: &Snapshot,
        center: [f32; 2],
        radius: f32,
        s: f32,
    ) {
        let pixels_per_meter = radius / ZOOM_RANGES[self.zoom];
        for point in &player.points {
            let d = self.to_screen([
                point.position[0] - player.position[0],
                point.position[1] - player.position[1],
            ]);
            if length(d) * pixels_per_meter > radius - 8.0 * s {
                continue;
            }
            let at = [
                center[0] + d[0] * pixels_per_meter,
                center[1] + d[1] * pixels_per_meter,
            ];
            let opacity = if point.cleared { 0.45 } else { 1.0 };
            let outline = self.color(BLACK, 0.8 * opacity);
            let dot = |r: f32, rgb: [u8; 3]| {
                draw_list
                    .add_circle(at, (r + 1.2) * s, outline)
                    .filled(true)
                    .build();
                draw_list
                    .add_circle(at, r * s, self.color(rgb, opacity))
                    .filled(true)
                    .build();
            };
            match point.kind {
                PointKind::Item => dot(2.4, [236, 190, 96]),
                PointKind::Npc => dot(3.0, [150, 196, 240]),
                PointKind::Invasion => dot(3.0, [214, 64, 52]),
                PointKind::Landmark => {
                    let r = 4.5 * s;
                    let diamond = vec![
                        [at[0], at[1] - r],
                        [at[0] + r, at[1]],
                        [at[0], at[1] + r],
                        [at[0] - r, at[1]],
                    ];
                    draw_list
                        .add_polyline(diamond.clone(), self.color([214, 190, 140], 0.85 * opacity))
                        .filled(true)
                        .build();
                    draw_list
                        .add_polyline([diamond.as_slice(), &diamond[..1]].concat(), outline)
                        .thickness(1.2 * s)
                        .build();
                }
                PointKind::Boss => {
                    let rgb = if point.cleared {
                        [150, 150, 150]
                    } else {
                        [200, 40, 36]
                    };
                    dot(4.0, [30, 10, 8]);
                    draw_list
                        .add_circle(at, 4.0 * s, self.color(rgb, opacity))
                        .thickness(1.8 * s)
                        .build();
                    draw_list
                        .add_circle(at, 1.6 * s, self.color(rgb, opacity))
                        .filled(true)
                        .build();
                }
                PointKind::Grace => {
                    draw_list
                        .add_circle(at, 4.5 * s, self.color([255, 222, 150], 0.8 * opacity))
                        .thickness(1.4 * s)
                        .build();
                }
            }
        }
    }

    /// Beacons out of range sit on the rim, pointing the way.
    fn draw_beacons(
        &self,
        ui: &Ui,
        draw_list: &DrawListMut,
        player: &Snapshot,
        center: [f32; 2],
        radius: f32,
        s: f32,
    ) {
        let Some(texture) = self.beacon else {
            return;
        };
        let pixels_per_meter = radius / ZOOM_RANGES[self.zoom];
        let rim = radius - 18.0 * s;
        for beacon in &player.beacons {
            let d = self.to_screen([
                beacon.position[0] - player.position[0],
                beacon.position[1] - player.position[1],
            ]);
            let mut offset = [d[0] * pixels_per_meter, d[1] * pixels_per_meter];
            let distance = length(offset);
            let inside = distance <= rim;
            if !inside {
                offset = [offset[0] / distance * rim, offset[1] / distance * rim];
            }
            let at = [center[0] + offset[0], center[1] + offset[1]];
            let half = if inside { 13.0 } else { 10.0 } * s;
            let opacity = if inside { 1.0 } else { 0.8 };
            draw_list
                .add_image(
                    texture,
                    [at[0] - half, at[1] - half],
                    [at[0] + half, at[1] + half],
                )
                .col(self.color([255, 255, 255], opacity))
                .build();
            // Too small for a digit inside, so it goes beside it like on the game's compass.
            let number = beacon.number.to_string();
            let label = [at[0] + half * 0.85, at[1] + half * 0.35];
            self.draw_text_centered(
                ui,
                draw_list,
                label,
                self.color([255, 244, 220], opacity),
                &number,
            );
        }
    }

    /// A glowing golden point, like the grace icons on the world map.
    fn draw_graces(
        &self,
        draw_list: &DrawListMut,
        player: &Snapshot,
        center: [f32; 2],
        radius: f32,
        s: f32,
    ) {
        let pixels_per_meter = radius / ZOOM_RANGES[self.zoom];
        for grace in &player.graces {
            let d = self.to_screen([grace[0] - player.position[0], grace[1] - player.position[1]]);
            if length(d) * pixels_per_meter > radius - 10.0 * s {
                continue;
            }
            let at = [
                center[0] + d[0] * pixels_per_meter,
                center[1] + d[1] * pixels_per_meter,
            ];
            let circle = |r: f32, color: ImColor32| {
                draw_list.add_circle(at, r * s, color).filled(true).build();
            };
            circle(10.0, self.color([255, 210, 110], 0.12));
            circle(6.5, self.color([255, 214, 120], 0.25));
            draw_list
                .add_circle(at, 4.5 * s, self.color([255, 232, 170], 0.95))
                .thickness(1.6 * s)
                .build();
            circle(2.2, self.color([255, 248, 222], 1.0));
        }
    }

    fn draw_arrow(&self, draw_list: &DrawListMut, center: [f32; 2], s: f32) {
        let Some(arrow) = self.arrow else {
            return;
        };
        let (sin, cos) = self.facing_yaw.unwrap_or(0.0).sin_cos();
        let forward = self.to_screen([sin, cos]);
        let right = [-forward[1], forward[0]];
        let size = 30.0 * s;
        // Image corners, placed around the pivot with the image's up along `forward`.
        let corner = |u: f32, v: f32| {
            let (x, y) = ((u - ARROW_PIVOT[0]) * size, (v - ARROW_PIVOT[1]) * size);
            [
                center[0] + right[0] * x - forward[0] * y,
                center[1] + right[1] * x - forward[1] * y,
            ]
        };
        draw_list
            .add_image_quad(
                arrow,
                corner(0.0, 0.0),
                corner(1.0, 0.0),
                corner(1.0, 1.0),
                corner(0.0, 1.0),
            )
            .col(self.color([255, 255, 255], 1.0))
            .build();
    }

    fn draw_text_centered(
        &self,
        ui: &Ui,
        draw_list: &DrawListMut,
        center: [f32; 2],
        color: ImColor32,
        text: &str,
    ) {
        let [w, h] = ui.calc_text_size(text);
        let x = (center[0] - w * 0.5).round();
        let y = (center[1] - h * 0.5).round();
        draw_list.add_text([x + 1.0, y + 1.0], self.color(BLACK, 0.8), text);
        draw_list.add_text([x, y], color, text);
    }
}

/// Normal gameplay: no menu changing the HUD and no fade covering the screen.
fn in_control(snapshot: &Snapshot) -> bool {
    snapshot.hud_visible && snapshot.fade < 0.05
}

fn load_png(render_context: &mut dyn RenderContext, bytes: &[u8]) -> Option<TextureId> {
    let mut decoder = png::Decoder::new(bytes);
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::ALPHA);
    let mut reader = decoder.read_info().ok()?;
    let mut rgba = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut rgba).ok()?;
    render_context
        .load_texture(&rgba[..info.buffer_size()], info.width, info.height)
        .ok()
}

fn length([x, y]: [f32; 2]) -> f32 {
    (x * x + y * y).sqrt()
}

/// Wraps an angle difference into [-PI, PI) so easing takes the short way around.
fn wrap_angle(angle: f32) -> f32 {
    (angle + PI).rem_euclid(TAU) - PI
}

fn normalize(v: [f32; 2]) -> Option<[f32; 2]> {
    let l = length(v);
    (l > 1e-3).then(|| [v[0] / l, v[1] / l])
}

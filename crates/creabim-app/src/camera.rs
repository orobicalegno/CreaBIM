use glam::{Mat4, Quat, Vec2, Vec3};

/// Orbit camera around a target point, Z up.
#[derive(Clone, Debug)]
pub struct OrbitCamera {
    pub target: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub fov_y: f32,
    /// Orthographic (parallel) projection instead of perspective. The
    /// visible height is the one perspective has at the target.
    pub ortho: bool,
}

/// The standard directions to look from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    Top,
    /// From straight below, north up.
    Bottom,
    Front,
    Right,
    Back,
    Left,
    /// The default three-quarter view from the south-west.
    Iso,
}

impl Look {
    pub const ALL: [Look; 7] = [
        Look::Top,
        Look::Bottom,
        Look::Front,
        Look::Right,
        Look::Back,
        Look::Left,
        Look::Iso,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Look::Top => "Top",
            Look::Bottom => "Bottom",
            Look::Front => "Front",
            Look::Right => "Right",
            Look::Back => "Back",
            Look::Left => "Left",
            Look::Iso => "Iso",
        }
    }

    /// Yaw and pitch of the eye round the target. Front looks north, from
    /// the south.
    fn angles(self) -> (f32, f32) {
        use std::f32::consts::FRAC_PI_2;
        match self {
            // straight down and straight up, exactly: a pitch short of
            // 90° shows vertical edges as short diagonals
            Look::Top => (-FRAC_PI_2, FRAC_PI_2),
            Look::Bottom => (FRAC_PI_2, -FRAC_PI_2),
            Look::Front => (-FRAC_PI_2, 0.0),
            Look::Right => (0.0, 0.0),
            Look::Back => (FRAC_PI_2, 0.0),
            Look::Left => (std::f32::consts::PI, 0.0),
            Look::Iso => (-2.2, 0.55),
        }
    }
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self {
            target: Vec3::new(5.0, 3.0, 2.0),
            yaw: -2.2,
            pitch: 0.55,
            distance: 24.0,
            fov_y: 45f32.to_radians(),
            ortho: false,
        }
    }
}

/// The camera comes no closer to its target than this.
const MIN_DISTANCE: f32 = 0.05;

/// Zooming toward a point stops this far from it.
const MIN_REACH: f32 = 0.15;

/// While walking, the target is this far ahead of the eye.
const WALK_AHEAD: f32 = 1.0;

/// Orbiting stops at straight down and straight up.
const PITCH_MAX: f32 = std::f32::consts::FRAC_PI_2;

/// Within this of an axis view (top, bottom, or level and square to an
/// axis) the camera shows the axis view exactly.
const AXIS_SNAP: f32 = 2.0 * std::f32::consts::PI / 180.0;

/// `a` snapped to the nearest multiple of `step` when it lies within
/// [`AXIS_SNAP`] of it.
fn snap_angle(a: f32, step: f32) -> f32 {
    let n = (a / step).round() * step;
    if (a - n).abs() < AXIS_SNAP {
        n
    } else {
        a
    }
}

impl OrbitCamera {
    /// The yaw and pitch the camera shows: the stored ones, snapped to the
    /// nearest axis view within about 2°. The stored angles stay as they
    /// are, so orbiting on moves off the axis view again.
    pub fn view_angles(&self) -> (f32, f32) {
        use std::f32::consts::FRAC_PI_2;
        let pitch = snap_angle(self.pitch, FRAC_PI_2).clamp(-FRAC_PI_2, FRAC_PI_2);
        let yaw = if pitch.abs() < AXIS_SNAP || pitch.abs() > FRAC_PI_2 - AXIS_SNAP {
            snap_angle(self.yaw, FRAC_PI_2)
        } else {
            self.yaw
        };
        (yaw, pitch)
    }

    pub fn eye(&self) -> Vec3 {
        let (yaw, pitch) = self.view_angles();
        let (sp, cp) = pitch.sin_cos();
        let (sy, cy) = yaw.sin_cos();
        self.target + Vec3::new(cp * cy, cp * sy, sp) * self.distance
    }

    /// The camera's up direction. Z, except straight down or up, where Z
    /// lies along the view: there the screen's up is the horizontal way
    /// the camera faces, so Top shows north (+Y) up.
    pub fn up(&self) -> Vec3 {
        let (yaw, pitch) = self.view_angles();
        let (sp, cp) = pitch.sin_cos();
        if cp > 1e-3 {
            return Vec3::Z;
        }
        let (sy, cy) = yaw.sin_cos();
        Vec3::new(-cy * sp.signum(), -sy * sp.signum(), 0.0)
    }

    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        let view = Mat4::look_at_rh(self.eye(), self.target, self.up());
        let aspect = aspect.max(0.01);
        let proj = if self.ortho {
            let h = self.half_height();
            // the near plane sits well behind the eye so nothing is clipped
            Mat4::orthographic_rh(-h * aspect, h * aspect, -h, h, -2000.0, 4000.0)
        } else {
            Mat4::perspective_rh(self.fov_y, aspect, 0.05, 2000.0)
        };
        proj * view
    }

    /// World-space ray through a point given in normalised device coordinates.
    pub fn ray(&self, ndc: Vec2, aspect: f32) -> (Vec3, Vec3) {
        let inv = self.view_proj(aspect).inverse();
        let near = inv.project_point3(ndc.extend(0.0));
        let far = inv.project_point3(ndc.extend(1.0));
        (near, (far - near).normalize())
    }

    /// Half the height seen at the target's distance.
    fn half_height(&self) -> f32 {
        self.distance * (self.fov_y / 2.0).tan()
    }

    /// Look from a standard direction, keeping the target and distance.
    pub fn look(&mut self, look: Look) {
        (self.yaw, self.pitch) = look.angles();
    }

    /// Orbit about `pivot` instead of the target, as Blender's "orbit
    /// around selection": the whole camera swings round the pivot, so the
    /// pivot stays where it is on screen.
    pub fn orbit_around(&mut self, pivot: Vec3, delta: Vec2) {
        let dyaw = -delta.x * 0.008;
        let pitch = (self.pitch + delta.y * 0.008).clamp(-PITCH_MAX, PITCH_MAX);
        let (sy, cy) = self.yaw.sin_cos();
        // the horizontal axis square to the view, defined at the poles too
        let axis = Vec3::new(sy, -cy, 0.0);
        let turn = Quat::from_rotation_z(dyaw) * Quat::from_axis_angle(axis, pitch - self.pitch);
        self.target = pivot + turn * (self.target - pivot);
        self.yaw += dyaw;
        self.pitch = pitch;
    }

    pub fn orbit(&mut self, delta: Vec2) {
        self.yaw -= delta.x * 0.008;
        self.pitch = (self.pitch + delta.y * 0.008).clamp(-PITCH_MAX, PITCH_MAX);
    }

    pub fn pan(&mut self, delta: Vec2, viewport_height: f32) {
        let eye = self.eye();
        let fwd = (self.target - eye).normalize();
        let right = fwd.cross(self.up()).normalize_or_zero();
        let up = right.cross(fwd);
        let k = 2.0 * self.distance * (self.fov_y / 2.0).tan() / viewport_height.max(1.0);
        self.target += (-right * delta.x + up * delta.y) * k;
    }

    pub fn zoom(&mut self, steps: f32) {
        self.distance =
            (self.distance * (1.0 - steps * 0.1).clamp(0.5, 1.5)).clamp(MIN_DISTANCE, 1500.0);
    }

    /// Zoom toward (or away from) `point`, a point of the model under the
    /// pointer: the whole camera moves along the line to it, so it stays
    /// where it is on screen, and the orbit target comes along, so zooming
    /// never stops short of what is pointed at. Closer than a hand's width
    /// it stops.
    pub fn zoom_toward(&mut self, point: Vec3, steps: f32) {
        let f = (1.0 - steps * 0.1).clamp(0.5, 1.5);
        let eye = self.eye();
        let reach = eye.distance(point);
        if f < 1.0 && reach * f < MIN_REACH {
            return;
        }
        self.target = point + (self.target - point) * f;
        self.distance = (self.distance * f).clamp(MIN_DISTANCE, 1500.0);
    }

    /// The depth of `point` in front of the camera, along the view.
    pub fn depth_of(&self, point: Vec3) -> f32 {
        let eye = self.eye();
        (point - eye).dot((self.target - eye).normalize_or_zero())
    }

    /// Pan so what was grabbed `depth` in front of the camera moves with
    /// the pointer: near things as much as the pointer, far ones too.
    pub fn pan_at(&mut self, delta: Vec2, viewport_height: f32, depth: f32) {
        let eye = self.eye();
        let fwd = (self.target - eye).normalize();
        let right = fwd.cross(self.up()).normalize_or_zero();
        let up = right.cross(fwd);
        // an orthographic view shows every depth the same size
        let depth = if self.ortho {
            self.distance
        } else {
            depth.max(MIN_REACH)
        };
        let k = 2.0 * depth * (self.fov_y / 2.0).tan() / viewport_height.max(1.0);
        self.target += (-right * delta.x + up * delta.y) * k;
    }

    /// Turn the head, the eye staying where it is: walking, looking round.
    pub fn look_round(&mut self, delta: Vec2) {
        let eye = self.eye();
        self.orbit_around(eye, -delta * 0.6);
        // the pivot is the eye: put it back exactly where it was
        let moved = self.eye() - eye;
        self.target -= moved;
    }

    /// Walk: `ahead` forward and `side` to the right along the ground,
    /// `rise` up, in metres; the view keeps its direction.
    pub fn walk(&mut self, ahead: f32, side: f32, rise: f32) {
        let (yaw, _) = self.view_angles();
        // the camera looks from the eye toward the target: opposite its yaw
        let fwd = -Vec3::new(yaw.cos(), yaw.sin(), 0.0);
        let right = fwd.cross(Vec3::Z).normalize_or_zero();
        self.target += fwd * ahead + right * side + Vec3::Z * rise;
    }

    /// Stand at the eye, looking the way the view looks, with the target a
    /// short way ahead: how walking starts, so turning the head turns
    /// about the eye and not a far point.
    pub fn stand(&mut self) {
        let eye = self.eye();
        let fwd = (self.target - eye).normalize_or(Vec3::X);
        self.distance = WALK_AHEAD;
        self.target = eye + fwd * WALK_AHEAD;
    }

    /// Zoom toward the point under the pointer (`ndc`, in normalised
    /// device coordinates), as plans do: the target slides toward where
    /// the pointer ray meets the plane through the target, so that point
    /// stays under the pointer.
    pub fn zoom_at(&mut self, steps: f32, ndc: Vec2, aspect: f32) {
        let before = self.distance;
        let (origin, dir) = self.ray(ndc, aspect);
        let fwd = (self.target - self.eye()).normalize_or_zero();
        self.zoom(steps);
        let along = dir.dot(fwd);
        if along.abs() < 1e-4 {
            return;
        }
        let at = origin + dir * ((self.target - origin).dot(fwd) / along);
        self.target = at + (self.target - at) * (self.distance / before);
    }

    /// Fit a bounding box in view, for a viewport of the given aspect ratio.
    pub fn frame(&mut self, lo: Vec3, hi: Vec3, aspect: f32) {
        self.target = (lo + hi) / 2.0;
        let radius = (hi - lo).length().max(1.0) / 2.0;
        let half_fov_x = ((self.fov_y / 2.0).tan() * aspect.max(0.1)).atan();
        let half_fov = half_fov_x.min(self.fov_y / 2.0);
        self.distance = radius / half_fov.sin() * 1.05;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zooming_toward_a_point_keeps_it_still_and_never_sticks() {
        let mut c = OrbitCamera::default();
        let aspect = 1.5;
        // a point well off the target, under the pointer
        let (o, d) = c.ray(Vec2::new(0.4, -0.2), aspect);
        let point = o + d * 9.0;
        let screen = |c: &OrbitCamera| c.view_proj(aspect).project_point3(point).truncate();
        let before = screen(&c);
        for _ in 0..60 {
            c.zoom_toward(point, 1.0);
            assert!((screen(&c) - before).length() < 1e-3, "{:?}", screen(&c));
        }
        // it got right up to it: no stop half a metre from the target
        assert!(c.eye().distance(point) < 0.3, "{}", c.eye().distance(point));
        // and backs away again
        for _ in 0..5 {
            c.zoom_toward(point, -3.0);
        }
        assert!(c.eye().distance(point) > 0.5, "{}", c.eye().distance(point));
    }

    #[test]
    fn panning_moves_what_was_grabbed_with_the_pointer() {
        let mut c = OrbitCamera::default();
        let aspect = 1.0;
        let h = 800.0;
        let (o, d) = c.ray(Vec2::ZERO, aspect);
        // grabbed 4 m in front of the eye, nearer than the target
        let grabbed = o + d * 4.0;
        let depth = c.depth_of(grabbed);
        let before = c.view_proj(aspect).project_point3(grabbed).truncate();
        // 100 pixels right
        c.pan_at(Vec2::new(100.0, 0.0), h, depth);
        let after = c.view_proj(aspect).project_point3(grabbed).truncate();
        // 100 px of 800 is a quarter of the NDC height of 2
        assert!(
            ((after.x - before.x) - 0.25).abs() < 1e-3,
            "{after:?} {before:?}"
        );
    }

    #[test]
    fn walking_moves_the_eye_and_looking_round_keeps_it() {
        let mut c = OrbitCamera::default();
        c.stand();
        let eye = c.eye();
        c.look_round(Vec2::new(40.0, 10.0));
        assert!(c.eye().distance(eye) < 1e-4);
        // forward along the ground, whatever the head's tilt
        let before = c.eye();
        c.walk(2.0, 0.0, 0.0);
        let step = c.eye() - before;
        assert!((step.length() - 2.0).abs() < 1e-4 && step.z.abs() < 1e-5);
        let fwd = c.target - c.eye();
        assert!(Vec2::new(step.x, step.y).dot(Vec2::new(fwd.x, fwd.y)) > 0.0);
        c.walk(0.0, 0.0, 0.5);
        assert!((c.eye().z - before.z - 0.5).abs() < 1e-4);
    }

    #[test]
    fn orbiting_round_a_pivot_keeps_it_on_screen() {
        let mut c = OrbitCamera::default();
        let pivot = Vec3::new(8.0, 1.0, 0.5);
        let at = |c: &OrbitCamera| c.view_proj(1.5).project_point3(pivot);
        let before = at(&c);
        c.orbit_around(pivot, Vec2::new(40.0, -25.0));
        let after = at(&c);
        assert!(
            (before - after).truncate().length() < 1e-3,
            "{before} {after}"
        );
        assert!((c.pitch - (0.55 - 0.2)).abs() < 1e-5);
    }

    #[test]
    fn the_top_view_looks_straight_down_with_north_up() {
        for ortho in [false, true] {
            let mut c = OrbitCamera {
                ortho,
                target: Vec3::ZERO,
                ..Default::default()
            };
            c.look(Look::Top);
            let vp = c.view_proj(1.5);
            // a vertical edge under the target is a point
            let foot = vp.project_point3(Vec3::ZERO);
            let head = vp.project_point3(Vec3::new(0.0, 0.0, 3.0));
            assert!((foot - head).truncate().length() < 1e-5, "{foot} {head}");
            // north (+Y) is up the screen, east (+X) to the right
            assert!(vp.project_point3(Vec3::Y).y > 0.1);
            assert!(vp.project_point3(Vec3::X).x > 0.05);
            c.look(Look::Bottom);
            let vp = c.view_proj(1.5);
            assert!(vp.project_point3(Vec3::Y).y > 0.1);
            let head = vp.project_point3(Vec3::new(0.0, 0.0, -3.0));
            assert!((vp.project_point3(Vec3::ZERO) - head).truncate().length() < 1e-5);
        }
    }

    #[test]
    fn orbiting_near_an_axis_view_snaps_to_it_and_on_past_it() {
        let mut c = OrbitCamera::default();
        c.look(Look::Front);
        // 1° off level and square: shown exactly level and square
        c.pitch = 1f32.to_radians();
        c.yaw += 1f32.to_radians();
        let (yaw, pitch) = c.view_angles();
        assert_eq!(pitch, 0.0);
        assert!((yaw + std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        // further off, it follows the orbit
        c.pitch = 5f32.to_radians();
        assert_eq!(c.view_angles().1, c.pitch);
        // orbiting up stops at straight down, and pans still work there
        c.orbit(Vec2::new(0.0, 1000.0));
        assert_eq!(c.view_angles().1, std::f32::consts::FRAC_PI_2);
        let before = c.target;
        c.pan(Vec2::new(10.0, 0.0), 600.0);
        assert!((c.target - before).length() > 1e-4);
        assert!((c.target - before).z.abs() < 1e-6);
    }

    #[test]
    fn zooming_keeps_the_point_under_the_pointer() {
        for ortho in [false, true] {
            let mut c = OrbitCamera {
                ortho,
                ..Default::default()
            };
            let ndc = Vec2::new(0.6, -0.4);
            let (o, d) = c.ray(ndc, 1.5);
            let fwd = (c.target - c.eye()).normalize();
            let p = o + d * ((c.target - o).dot(fwd) / d.dot(fwd));
            c.zoom_at(3.0, ndc, 1.5);
            assert!(c.distance < 24.0, "it zoomed in");
            let after = c.view_proj(1.5).project_point3(p).truncate();
            assert!((after - ndc).length() < 1e-3, "{ortho}: {after}");
        }
    }

    #[test]
    fn orthographic_rays_are_parallel() {
        let c = OrbitCamera {
            ortho: true,
            ..Default::default()
        };
        let (_, a) = c.ray(Vec2::new(-0.8, 0.3), 1.5);
        let (_, b) = c.ray(Vec2::new(0.7, -0.6), 1.5);
        assert!(a.dot(b) > 0.9999);
    }
}

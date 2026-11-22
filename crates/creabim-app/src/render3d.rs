//! The 3D viewport: renders the model with wgpu into an offscreen texture
//! that egui shows like an image. Any number of these can exist, which is
//! what lets one model appear in several live views at once.

use eframe::egui;
use eframe::egui_wgpu::{self, wgpu};
use glam::Mat4;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 3],
    /// The material's roughness, metallic and opacity.
    pub surface: [f32; 3],
}

/// Roughness, metallic and opacity of a surface without a material, and
/// of lines.
pub const PLAIN: [f32; 3] = [0.8, 0.0, 1.0];

/// How the 3D view draws the model.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RenderMode {
    /// Surfaces in their materials' colours.
    #[default]
    Shaded,
    /// The same with every edge drawn over them.
    ShadedEdges,
    /// White surfaces with their edges: a drawing in 3D.
    HiddenLine,
    /// Edges only, the hidden ones too: what is inside and behind shows.
    Wireframe,
    /// An architect's white model: every surface white, soft sky light,
    /// sun shadows and shading in the corners, faint edges.
    WhiteModel,
}

impl RenderMode {
    pub const ALL: [RenderMode; 5] = [
        RenderMode::Shaded,
        RenderMode::ShadedEdges,
        RenderMode::HiddenLine,
        RenderMode::Wireframe,
        RenderMode::WhiteModel,
    ];

    pub fn label(self) -> &'static str {
        match self {
            RenderMode::Shaded => "Shaded",
            RenderMode::ShadedEdges => "Shaded with edges",
            RenderMode::HiddenLine => "Hidden line",
            RenderMode::Wireframe => "Wireframe",
            RenderMode::WhiteModel => "White model",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            RenderMode::Shaded => "sun",
            RenderMode::ShadedEdges => "render-edges",
            RenderMode::HiddenLine => "render-hidden-line",
            RenderMode::Wireframe => "render-wireframe",
            RenderMode::WhiteModel => "render-hidden-line",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            RenderMode::Shaded => "Surfaces in their materials' colours",
            RenderMode::ShadedEdges => {
                "Surfaces in colour with every edge drawn: openings and shafts read clearly"
            }
            RenderMode::HiddenLine => "White surfaces and their visible edges, like a drawing",
            RenderMode::Wireframe => {
                "Edges only, the hidden ones too: see shafts and what is inside walls and slabs"
            }
            RenderMode::WhiteModel => {
                "A white study model: soft light, sun shadows, shaded corners and faint edges"
            }
        }
    }

    /// Are the model's edges drawn?
    pub fn edges(self) -> bool {
        self != RenderMode::Shaded
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],
    light: [f32; 4],
    eye: [f32; 4],
    sun_vp: [[f32; 4]; 4],
    sky_vp: [[[f32; 4]; 4]; SKY],
    /// Sun shadows on (x), sky shading strength (y), white model (z), the
    /// light maps' depth in metres (w).
    flags: [f32; 4],
    /// The background (rgb) the ground takes, where it catches shadows.
    ground: [f32; 4],
}

/// How the 3D view is lit: sun shadows, shading from the sky in corners
/// and under overhangs, the white model's look, and where the sun is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lighting {
    pub shadows: bool,
    /// 0 none to 1 full.
    pub ambient_occlusion: f32,
    pub white: bool,
    /// Toward the sun, in model coordinates.
    pub sun: [f32; 3],
}

impl Default for Lighting {
    fn default() -> Self {
        Self {
            shadows: false,
            ambient_occlusion: 0.0,
            white: false,
            sun: [0.4, -0.6, 0.85],
        }
    }
}

/// The directions the sky's shading is taken from.
const SKY: usize = 8;
const SUN_MAP: u32 = 2048;
const SKY_MAP: u32 = 1024;
/// The sky's maps: depth packed in four bytes, a format every graphics
/// backend can draw into and read back.
const SKY_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// A future that is ready at once (as wgpu's are on the desktop), waited for.
fn block_on<F: std::future::Future>(f: F) -> F::Output {
    let mut f = std::pin::pin!(f);
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    loop {
        if let std::task::Poll::Ready(v) = f.as_mut().poll(&mut cx) {
            return v;
        }
        std::thread::yield_now();
    }
}

/// What a depth map of the model sees from far along `dir`, the model's
/// box (`centre`, `radius`) filling it.
fn light_matrix(dir: glam::Vec3, centre: glam::Vec3, radius: f32) -> Mat4 {
    let d = dir.normalize_or(glam::Vec3::Z);
    let up = if d.z.abs() > 0.95 {
        glam::Vec3::Y
    } else {
        glam::Vec3::Z
    };
    let view = Mat4::look_at_rh(centre + d * radius * 2.0, centre, up);
    let proj = Mat4::orthographic_rh(-radius, radius, -radius, radius, 0.01, radius * 4.0);
    proj * view
}

/// The sky's directions: round the compass, high and low in turn.
fn sky_dirs() -> [glam::Vec3; SKY] {
    std::array::from_fn(|k| {
        let az = k as f32 * std::f32::consts::TAU / SKY as f32;
        let alt: f32 = if k % 2 == 0 { 40f32 } else { 70f32 }.to_radians();
        glam::Vec3::new(alt.cos() * az.cos(), alt.cos() * az.sin(), alt.sin())
    })
}

const SHADOW_SHADER: &str = r#"
struct L { m: mat4x4<f32> };
@group(0) @binding(0) var<uniform> l: L;
@vertex fn vs_shadow(@location(0) pos: vec3<f32>) -> @builtin(position) vec4<f32> {
    return l.m * vec4<f32>(pos, 1.0);
}
// the sky's maps keep their depth as a colour, which every graphics
// backend can read back
@fragment fn fs_sky(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    var e = fract(clamp(p.z, 0.0, 0.999999) * vec4<f32>(1.0, 255.0, 65025.0, 16581375.0));
    e -= e.yzww * vec4<f32>(1.0 / 255.0, 1.0 / 255.0, 1.0 / 255.0, 0.0);
    return e;
}
"#;

const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const SAMPLES: u32 = 4;

const SHADER: &str = r#"
struct U {
    view_proj: mat4x4<f32>,
    light: vec4<f32>,
    eye: vec4<f32>,
    sun_vp: mat4x4<f32>,
    sky_vp: array<mat4x4<f32>, 8>,
    flags: vec4<f32>,
    ground: vec4<f32>,
};
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var sun_map: texture_depth_2d;
@group(0) @binding(2) var shadow_sampler: sampler_comparison;
@group(0) @binding(3) var sky_map: texture_2d_array<f32>;

fn map_uv(m: mat4x4<f32>, p: vec3<f32>) -> vec3<f32> {
    let c = m * vec4<f32>(p, 1.0);
    let q = c.xyz / c.w;
    return vec3<f32>(q.x * 0.5 + 0.5, 0.5 - q.y * 0.5, q.z);
}

fn inside(q: vec3<f32>) -> bool {
    return q.x > 0.0 && q.x < 1.0 && q.y > 0.0 && q.y < 1.0 && q.z < 1.0;
}

// How much sun reaches a point: 1 lit, 0 in shadow, soft at the edges.
fn sun_light(p: vec3<f32>, n: vec3<f32>) -> f32 {
    //LIT{
    if (u.flags.x < 0.5) { return 1.0; }
    let q = map_uv(u.sun_vp, p + n * 0.05);
    if (!inside(q)) { return 1.0; }
    let texel = 1.5 / 2048.0;
    var s = 0.0;
    for (var x = -1; x <= 1; x++) {
        for (var y = -1; y <= 1; y++) {
            s += textureSampleCompareLevel(sun_map, shadow_sampler, q.xy + vec2<f32>(f32(x), f32(y)) * texel, q.z - 0.0012);
        }
    }
    return s / 9.0;
    //}LIT
    return 1.0;
}

// How much of the sky a point sees: corners, the ground under an overhang
// and the inside of a recess see less of it. What covers a point shades it
// the less the farther above it is, so the shade gathers where things meet.
fn sky_light(p: vec3<f32>, n: vec3<f32>) -> f32 {
    //LIT{
    if (u.flags.y <= 0.0) { return 1.0; }
    var s = 0.0;
    var w = 0.0;
    for (var k = 0; k < 8; k++) {
        let q = map_uv(u.sky_vp[k], p + n * 0.08);
        if (!inside(q)) { continue; }
        // the sky from this side, as much as the face looks toward it
        let m = u.sky_vp[k];
        let toward = normalize(vec3<f32>(m[0].z, m[1].z, m[2].z));
        let facing = 0.35 + 0.65 * max(dot(n, -toward), 0.0);
        var occ = 0.0;
        for (var t = 0; t < 4; t++) {
            let o = vec2<f32>(f32(t % 2) - 0.5, f32(t / 2) - 0.5) * 3.0;
            let c = clamp(vec2<i32>(q.xy * 1024.0 + o), vec2<i32>(0), vec2<i32>(1023));
            let d = dot(textureLoad(sky_map, c, k, 0), vec4<f32>(1.0, 1.0 / 255.0, 1.0 / 65025.0, 1.0 / 16581375.0));
            let gap = (q.z - d) * u.flags.w;
            if (gap > 0.18) { occ += exp(-gap / 2.5); }
        }
        s += facing * (1.0 - occ / 4.0);
        w += facing;
    }
    if (w <= 0.0) { return 1.0; }
    return mix(1.0, s / w, u.flags.y);
    //}LIT
    return 1.0;
}

struct VIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) surface: vec3<f32>,
};
struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) world: vec3<f32>,
    @location(3) surface: vec3<f32>,
};

@vertex fn vs_main(v: VIn) -> VOut {
    var o: VOut;
    o.clip = u.view_proj * vec4<f32>(v.pos, 1.0);
    o.normal = v.normal;
    o.color = v.color;
    o.world = v.pos;
    o.surface = v.surface;
    return o;
}

// A simple studio look: the sky-and-sun lighting the view always had for
// diffuse surfaces, plus a highlight and a reflection of a soft sky that
// grow sharper as the surface gets smoother, tinted by the colour for
// metals. Colours are given in sRGB and shaded in linear space. The
// result is encoded back to sRGB: the offscreen image reaches the screen
// through egui with its values as they are, so without this every lit
// surface showed much darker than its colour.
@fragment fn fs_shaded(i: VOut) -> @location(0) vec4<f32> {
    var n = normalize(i.normal);
    let v = normalize(u.eye.xyz - i.world);
    // two-sided: light the face that is seen
    if (dot(n, v) < 0.0) { n = -n; }
    let l = normalize(u.light.xyz);
    let rough = clamp(i.surface.x, 0.04, 1.0);
    let metal = clamp(i.surface.y, 0.0, 1.0);
    let base = pow(i.color, vec3<f32>(2.2));
    let sh = sun_light(i.world, n);
    let ao = sky_light(i.world, n);
    let diffuse = max(dot(n, l), 0.0) * sh;
    let sky = 0.5 + 0.5 * n.z;
    if (u.flags.z > 0.5) {
        // the white model: soft sky, the sun's light and shadow, no shine
        let w = base * ((0.28 + 0.40 * sky) * ao * ao + 0.50 * diffuse * (0.6 + 0.4 * ao));
        return vec4<f32>(pow(max(w, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.2)), clamp(i.surface.z, 0.05, 1.0));
    }
    let lit = base * (1.0 - metal) * ((0.30 + 0.25 * sky) * ao + 0.55 * diffuse);
    let f0 = mix(vec3<f32>(0.04), base, metal);
    let fres = pow(1.0 - max(dot(n, v), 0.0), 5.0);
    let f = f0 + (vec3<f32>(1.0) - f0) * fres * (1.0 - rough);
    let h = normalize(l + v);
    let a = rough * rough;
    let shine = clamp(2.0 / (a * a) - 2.0, 1.0, 2048.0);
    let highlight = f * pow(max(dot(n, h), 0.0), shine) * (shine + 2.0) / 8.0 * diffuse;
    let r = reflect(-v, n);
    let env = mix(vec3<f32>(0.30, 0.30, 0.32), vec3<f32>(0.95, 0.97, 1.0), 0.5 + 0.5 * r.z);
    let reflection = f * env * (1.0 - 0.6 * rough);
    let c = lit + min(highlight, vec3<f32>(4.0)) * (1.0 - 0.5 * rough) + reflection;
    return vec4<f32>(pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.2)), clamp(i.surface.z, 0.05, 1.0));
}

@fragment fn fs_flat(i: VOut) -> @location(0) vec4<f32> {
    return vec4<f32>(pow(i.color, vec3<f32>(2.2)), 1.0);
}

// The ground: the background's own colour, but where the model's shadow
// and the shade under it fall
@fragment fn fs_ground(i: VOut) -> @location(0) vec4<f32> {
    let n = vec3<f32>(0.0, 0.0, 1.0);
    let sh = sun_light(i.world, n);
    let ao = sky_light(i.world, n);
    let k = (1.0 - 0.38 * (1.0 - sh)) * (0.40 + 0.60 * ao * ao);
    return vec4<f32>(u.ground.rgb * k, 1.0);
}
"#;

struct Mesh {
    vertices: wgpu::Buffer,
    indices: Option<wgpu::Buffer>,
    count: u32,
}

struct Targets {
    size: [u32; 2],
    msaa: wgpu::TextureView,
    resolve: wgpu::TextureView,
    depth: wgpu::TextureView,
}

pub struct Viewport3d {
    shaded: wgpu::RenderPipeline,
    /// See-through surfaces, blended over the rest after it is drawn.
    glass: wgpu::RenderPipeline,
    lines: wgpu::RenderPipeline,
    overlay_lines: wgpu::RenderPipeline,
    /// See-through surfaces drawn only where something nearer hides them.
    behind: wgpu::RenderPipeline,
    /// The selection drawn again over itself, tinted: depth equal passes,
    /// so it lies exactly on the element (MU39: a click no longer
    /// rebuilds the whole model's buffers).
    highlight: wgpu::RenderPipeline,
    ground: wgpu::RenderPipeline,
    shadow: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    /// The sun's depth map, and the sky's (a layer per direction).
    sun_view: wgpu::TextureView,
    sky_views: Vec<wgpu::TextureView>,
    sky_depth: wgpu::TextureView,
    sky: wgpu::RenderPipeline,
    /// The graphics card took the shadow shaders; without them the view
    /// draws as before, unshadowed.
    lit: bool,
    /// A light matrix buffer and bind group per map: sun first, then sky.
    light_bgs: Vec<(wgpu::Buffer, wgpu::BindGroup)>,
    /// The model's box (centre, radius), from its vertices.
    bounds: Option<(glam::Vec3, f32)>,
    ground_mesh: Option<Mesh>,
    /// The maps are made again when the model or the sun changes.
    sun_made: Option<[f32; 3]>,
    sky_made: bool,
    triangles: Option<Mesh>,
    glass_mesh: Option<Mesh>,
    line_mesh: Option<Mesh>,
    overlay_mesh: Option<Mesh>,
    behind_mesh: Option<Mesh>,
    sel_mesh: Option<Mesh>,
    sel_lines: Option<Mesh>,
    sel_top: Option<Mesh>,
    targets: Option<Targets>,
    texture_id: Option<egui::TextureId>,
}

impl Viewport3d {
    /// The renderer, with sun shadows and the sky's shading when the
    /// graphics card takes them, and without them otherwise. Graphics
    /// errors later on go to the activity log rather than stopping a frame.
    pub fn new(device: &wgpu::Device) -> Self {
        device.on_uncaptured_error(std::sync::Arc::new(|e: wgpu::Error| {
            static SEEN: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            if SEEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed) < 20 {
                eprintln!("graphics error: {e}");
                crate::actlog::line(format!("graphics error: {e}"));
            }
        }));
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let lit = Self::build(device, true);
        match block_on(scope.pop()) {
            None => lit,
            Some(e) => {
                let why = e.to_string();
                eprintln!("3D shadows off, the graphics card refused them: {why}");
                crate::actlog::line(format!(
                    "3D shadows off, the graphics card refused them: {}",
                    why.lines().take(4).collect::<Vec<_>>().join(" ")
                ));
                drop(lit);
                Self::build(device, false)
            }
        }
    }

    /// Whether sun shadows and the sky's shading work on this card.
    pub fn lit(&self) -> bool {
        self.lit
    }

    fn build(device: &wgpu::Device, lit: bool) -> Self {
        let source = if lit {
            SHADER.to_string()
        } else {
            // the light maps' reading left out
            let mut out = String::new();
            let mut skip = false;
            for line in SHADER.lines() {
                if line.contains("//LIT{") {
                    skip = true;
                } else if line.contains("//}LIT") {
                    skip = false;
                } else if !skip {
                    out.push_str(line);
                    out.push('\n');
                }
            }
            out
        };
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("creabim_shader"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("creabim_uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let depth_map = |size: u32, layers: u32| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some("creabim_shadow_map"),
                size: wgpu::Extent3d {
                    width: size,
                    height: size,
                    depth_or_array_layers: layers,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
        };
        let sun_tex = depth_map(SUN_MAP, 1);
        let sky_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("creabim_sky_maps"),
            size: wgpu::Extent3d {
                width: SKY_MAP,
                height: SKY_MAP,
                depth_or_array_layers: SKY as u32,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SKY_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let sky_depth = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("creabim_sky_depth"),
                size: wgpu::Extent3d {
                    width: SKY_MAP,
                    height: SKY_MAP,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&Default::default());
        let sun_view = sun_tex.create_view(&Default::default());
        let sky_array = sky_tex.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let sky_views: Vec<wgpu::TextureView> = (0..SKY as u32)
            .map(|k| {
                sky_tex.create_view(&wgpu::TextureViewDescriptor {
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_array_layer: k,
                    array_layer_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("creabim_shadow_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        let tex_entry = |binding, dim| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Depth,
                view_dimension: dim,
                multisampled: false,
            },
            count: None,
        };
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("creabim_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                tex_entry(1, wgpu::TextureViewDimension::D2),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("creabim_bg"),
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniforms.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&sun_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&shadow_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&sky_array),
                },
            ],
        });
        // the depth maps' own pass: positions only, through a light matrix
        let shadow_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("creabim_shadow"),
            source: wgpu::ShaderSource::Wgsl(SHADOW_SHADER.into()),
        });
        let light_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("creabim_light_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let light_bgs: Vec<(wgpu::Buffer, wgpu::BindGroup)> = (0..=SKY)
            .map(|_| {
                let b = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("creabim_light"),
                    size: 64,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                let g = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    layout: &light_bgl,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: b.as_entire_binding(),
                    }],
                });
                (b, g)
            })
            .collect();
        let shadow_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("creabim_shadow_layout"),
            bind_group_layouts: &[Some(&light_bgl)],
            immediate_size: 0,
        });
        let shadow = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("creabim_shadow"),
            layout: Some(&shadow_layout),
            vertex: wgpu::VertexState {
                module: &shadow_module,
                entry_point: Some("vs_shadow"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3],
                })],
                compilation_options: Default::default(),
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: wgpu::DepthBiasState {
                    constant: 2,
                    slope_scale: 2.0,
                    clamp: 0.0,
                },
            }),
            multisample: Default::default(),
            fragment: None,
            multiview_mask: None,
            cache: None,
        });
        let sky = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("creabim_sky"),
            layout: Some(&shadow_layout),
            vertex: wgpu::VertexState {
                module: &shadow_module,
                entry_point: Some("vs_shadow"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3],
                })],
                compilation_options: Default::default(),
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shadow_module,
                entry_point: Some("fs_sky"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: SKY_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            multiview_mask: None,
            cache: None,
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("creabim_layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let pipeline = |topology, fragment: &str, depth_compare, blend: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(fragment),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs_main"),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Vertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x3, 3 => Float32x3],
                    })],
                    compilation_options: Default::default(),
                },
                primitive: wgpu::PrimitiveState {
                    topology,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(depth_compare != wgpu::CompareFunction::Always && !blend),
                    depth_compare: Some(depth_compare),
                    stencil: Default::default(),
                    // surfaces sit a hair behind their own edges, so edges
                    // drawn over them don't flicker
                    bias: if topology == wgpu::PrimitiveTopology::TriangleList {
                        wgpu::DepthBiasState {
                            constant: 2,
                            slope_scale: 1.5,
                            clamp: 0.0,
                        }
                    } else {
                        Default::default()
                    },
                }),
                multisample: wgpu::MultisampleState {
                    count: SAMPLES,
                    ..Default::default()
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(fragment),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: COLOR_FORMAT,
                        blend: blend.then_some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        Self {
            shaded: pipeline(
                wgpu::PrimitiveTopology::TriangleList,
                "fs_shaded",
                wgpu::CompareFunction::Less,
                false,
            ),
            glass: pipeline(
                wgpu::PrimitiveTopology::TriangleList,
                "fs_shaded",
                wgpu::CompareFunction::Less,
                true,
            ),
            lines: pipeline(
                wgpu::PrimitiveTopology::LineList,
                "fs_flat",
                wgpu::CompareFunction::LessEqual,
                false,
            ),
            overlay_lines: pipeline(
                wgpu::PrimitiveTopology::LineList,
                "fs_flat",
                wgpu::CompareFunction::Always,
                false,
            ),
            behind: pipeline(
                wgpu::PrimitiveTopology::TriangleList,
                "fs_shaded",
                wgpu::CompareFunction::Greater,
                true,
            ),
            highlight: pipeline(
                wgpu::PrimitiveTopology::TriangleList,
                "fs_shaded",
                wgpu::CompareFunction::LessEqual,
                true,
            ),
            ground: pipeline(
                wgpu::PrimitiveTopology::TriangleList,
                "fs_ground",
                wgpu::CompareFunction::Less,
                false,
            ),
            shadow,
            uniforms,
            bind_group,
            sun_view,
            sky_views,
            sky_depth,
            sky,
            lit,
            light_bgs,
            bounds: None,
            ground_mesh: None,
            sun_made: None,
            sky_made: false,
            triangles: None,
            glass_mesh: None,
            line_mesh: None,
            overlay_mesh: None,
            behind_mesh: None,
            sel_mesh: None,
            sel_lines: None,
            sel_top: None,
            targets: None,
            texture_id: None,
        }
    }

    /// Replace the geometry: shaded triangles (opaque ones, and see-through
    /// ones indexing the same vertices), depth-tested lines, and lines
    /// drawn on top of everything (2D drawings shown in 3D space).
    pub fn set_geometry(
        &mut self,
        device: &wgpu::Device,
        vertices: &[Vertex],
        indices: &[u32],
        see_through: &[u32],
        lines: &[Vertex],
        overlay: &[Vertex],
    ) {
        let buffer = |data: &[u8], usage| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: data,
                usage,
            })
        };
        // one vertex buffer for the opaque and the see-through triangles
        let shared = buffer(bytemuck::cast_slice(vertices), wgpu::BufferUsages::VERTEX);
        let tri_mesh = |indices: &[u32]| {
            (!indices.is_empty()).then(|| Mesh {
                vertices: shared.clone(),
                indices: Some(buffer(
                    bytemuck::cast_slice(indices),
                    wgpu::BufferUsages::INDEX,
                )),
                count: indices.len() as u32,
            })
        };
        self.triangles = tri_mesh(indices);
        self.glass_mesh = tri_mesh(see_through);
        // the model's box, for the light maps and the ground
        self.bounds = None;
        if !indices.is_empty() {
            let (mut lo, mut hi) = (glam::Vec3::splat(f32::MAX), glam::Vec3::splat(f32::MIN));
            for &i in indices {
                let p = glam::Vec3::from(vertices[i as usize].pos);
                lo = lo.min(p);
                hi = hi.max(p);
            }
            let centre = (lo + hi) / 2.0;
            let radius = ((hi - lo).length() / 2.0).max(1.0);
            self.bounds = Some((centre, radius));
            // the ground, where the model stands (the level at 0 when it
            // passes through it), reaching well past it
            let z = if lo.z < 0.0 && hi.z > 0.0 { 0.0 } else { lo.z } - 0.002;
            let r = radius * 3.0;
            let v = |x: f32, y: f32| Vertex {
                pos: [centre.x + x * r, centre.y + y * r, z],
                normal: [0.0, 0.0, 1.0],
                color: [1.0; 3],
                surface: PLAIN,
            };
            let quad = [
                v(-1.0, -1.0),
                v(1.0, -1.0),
                v(1.0, 1.0),
                v(-1.0, -1.0),
                v(1.0, 1.0),
                v(-1.0, 1.0),
            ];
            self.ground_mesh = Some(Mesh {
                vertices: buffer(bytemuck::cast_slice(&quad), wgpu::BufferUsages::VERTEX),
                indices: None,
                count: 6,
            });
        } else {
            self.ground_mesh = None;
        }
        self.sun_made = None;
        self.sky_made = false;
        let line_mesh = |v: &[Vertex]| {
            (!v.is_empty()).then(|| Mesh {
                vertices: buffer(bytemuck::cast_slice(v), wgpu::BufferUsages::VERTEX),
                indices: None,
                count: v.len() as u32,
            })
        };
        self.line_mesh = line_mesh(lines);
        self.overlay_mesh = line_mesh(overlay);
    }

    /// Replace the selection's highlight: its triangles (three vertices
    /// each) tinted, its edges (depth-tested) and what shows through
    /// everything (a selected cutter's outline). Small: the selection only.
    pub fn set_selection(
        &mut self,
        device: &wgpu::Device,
        triangles: &[Vertex],
        lines: &[Vertex],
        on_top: &[Vertex],
    ) {
        let mesh = |v: &[Vertex]| {
            (!v.is_empty()).then(|| Mesh {
                vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: None,
                    contents: bytemuck::cast_slice(v),
                    usage: wgpu::BufferUsages::VERTEX,
                }),
                indices: None,
                count: v.len() as u32,
            })
        };
        self.sel_mesh = mesh(triangles);
        self.sel_lines = mesh(lines);
        self.sel_top = mesh(on_top);
    }

    /// Replace the triangles (three vertices each) drawn see-through where
    /// what is in front hides them: elements inside walls and slabs.
    pub fn set_behind(&mut self, device: &wgpu::Device, vertices: &[Vertex]) {
        self.behind_mesh = (!vertices.is_empty()).then(|| Mesh {
            vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            indices: None,
            count: vertices.len() as u32,
        });
    }

    fn ensure_targets(&mut self, rs: &egui_wgpu::RenderState, size: [u32; 2]) {
        if self.targets.as_ref().is_some_and(|t| t.size == size) {
            return;
        }
        let device = &rs.device;
        let texture = |samples, format, usage| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: None,
                    size: wgpu::Extent3d {
                        width: size[0],
                        height: size[1],
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let t = Targets {
            size,
            msaa: texture(
                SAMPLES,
                COLOR_FORMAT,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
            ),
            resolve: texture(
                1,
                COLOR_FORMAT,
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            ),
            depth: texture(
                SAMPLES,
                DEPTH_FORMAT,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
            ),
        };
        let mut renderer = rs.renderer.write();
        match self.texture_id {
            Some(id) => renderer.update_egui_texture_from_wgpu_texture(
                device,
                &t.resolve,
                wgpu::FilterMode::Linear,
                id,
            ),
            None => {
                self.texture_id = Some(renderer.register_native_texture(
                    device,
                    &t.resolve,
                    wgpu::FilterMode::Linear,
                ))
            }
        }
        self.targets = Some(t);
    }

    /// Render a frame and return the egui texture holding it.
    pub fn render(
        &mut self,
        rs: &egui_wgpu::RenderState,
        size: [u32; 2],
        view_proj: Mat4,
        eye: [f32; 3],
        background: [f64; 3],
        lighting: Lighting,
    ) -> egui::TextureId {
        let size = [size[0].max(1), size[1].max(1)];
        self.ensure_targets(rs, size);
        let mut encoder = rs.device.create_command_encoder(&Default::default());
        self.prepare(rs, &mut encoder, view_proj, eye, background, lighting);
        let t = self.targets.take().expect("ensured above");
        self.draw(&mut encoder, &t, background, lighting);
        self.targets = Some(t);
        rs.queue.submit([encoder.finish()]);
        self.texture_id.expect("registered in ensure_targets")
    }

    /// The view at `size` pixels, as an image (a high-resolution export):
    /// drawn as the window draws it, into targets of its own.
    pub fn render_image(
        &mut self,
        rs: &egui_wgpu::RenderState,
        size: [u32; 2],
        view_proj: Mat4,
        eye: [f32; 3],
        background: [f64; 3],
        lighting: Lighting,
    ) -> Option<image::RgbaImage> {
        let max = rs.device.limits().max_texture_dimension_2d;
        let size = [size[0].clamp(1, max), size[1].clamp(1, max)];
        let t = make_targets(&rs.device, size, wgpu::TextureUsages::COPY_SRC);
        let mut encoder = rs.device.create_command_encoder(&Default::default());
        self.prepare(rs, &mut encoder, view_proj, eye, background, lighting);
        self.draw(&mut encoder, &t.0, background, lighting);
        // rows padded to 256 bytes, as copies want them
        let row = size[0] * 4;
        let padded = row.div_ceil(256) * 256;
        let buf = rs.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("creabim_export"),
            size: (padded * size[1]) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &t.1,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(size[1]),
                },
            },
            wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
        );
        rs.queue.submit([encoder.finish()]);
        let slice = buf.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r.is_ok());
        });
        let _ = rs.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_secs(60)),
        });
        if !rx.recv().unwrap_or(false) {
            return None;
        }
        let Ok(data) = slice.get_mapped_range() else {
            return None;
        };
        let mut out = Vec::with_capacity((row * size[1]) as usize);
        for y in 0..size[1] {
            let start = (y * padded) as usize;
            out.extend_from_slice(&data[start..start + row as usize]);
        }
        drop(data);
        buf.unmap();
        image::RgbaImage::from_raw(size[0], size[1], out)
    }

    /// The uniforms, and the light maps when the model or the sun changed.
    fn prepare(
        &mut self,
        rs: &egui_wgpu::RenderState,
        encoder: &mut wgpu::CommandEncoder,
        view_proj: Mat4,
        eye: [f32; 3],
        background: [f64; 3],
        lighting: Lighting,
    ) {
        let (centre, radius) = self.bounds.unwrap_or((glam::Vec3::ZERO, 10.0));
        let sun = glam::Vec3::from(lighting.sun).normalize_or(glam::Vec3::Z);
        let sun_vp = light_matrix(sun, centre, radius);
        let sky_vp: [Mat4; SKY] = sky_dirs().map(|d| light_matrix(d, centre, radius));
        let u = Uniforms {
            view_proj: view_proj.to_cols_array_2d(),
            light: [sun.x, sun.y, sun.z, 0.0],
            eye: [eye[0], eye[1], eye[2], 1.0],
            sun_vp: sun_vp.to_cols_array_2d(),
            sky_vp: sky_vp.map(|m| m.to_cols_array_2d()),
            flags: [
                if lighting.shadows { 1.0 } else { 0.0 },
                lighting.ambient_occlusion.clamp(0.0, 1.0),
                if lighting.white { 1.0 } else { 0.0 },
                // a light map's depth, in metres (its far plane at 4 radii)
                radius * 4.0,
            ],
            ground: [
                background[0] as f32,
                background[1] as f32,
                background[2] as f32,
                1.0,
            ],
        };
        rs.queue
            .write_buffer(&self.uniforms, 0, bytemuck::bytes_of(&u));
        let Some(tris) = &self.triangles else { return };
        if !self.lit {
            return;
        }
        let mut maps: Vec<(usize, Mat4)> = vec![];
        if lighting.shadows && self.sun_made != Some(lighting.sun) {
            maps.push((0, sun_vp));
            self.sun_made = Some(lighting.sun);
        }
        if lighting.ambient_occlusion > 0.0 && !self.sky_made {
            for (k, m) in sky_vp.iter().enumerate() {
                maps.push((k + 1, *m));
            }
            self.sky_made = true;
        }
        for (k, m) in maps {
            rs.queue.write_buffer(
                &self.light_bgs[k].0,
                0,
                bytemuck::bytes_of(&m.to_cols_array_2d()),
            );
            let mut pass = if k == 0 {
                encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("creabim_sun_pass"),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.sun_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                })
            } else {
                encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("creabim_sky_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.sky_views[k - 1],
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.sky_depth,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Discard,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                })
            };
            pass.set_pipeline(if k == 0 { &self.shadow } else { &self.sky });
            pass.set_bind_group(0, &self.light_bgs[k].1, &[]);
            pass.set_vertex_buffer(0, tris.vertices.slice(..));
            if let Some(ib) = &tris.indices {
                pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..tris.count, 0, 0..1);
            }
        }
    }

    fn draw(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        t: &Targets,
        background: [f64; 3],
        lighting: Lighting,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("creabim_3d"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &t.msaa,
                resolve_target: Some(&t.resolve),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: background[0],
                        g: background[1],
                        b: background[2],
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Discard,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &t.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_bind_group(0, &self.bind_group, &[]);
        // the ground catches shadows only when there are any to catch
        let ground = if self.lit && (lighting.shadows || lighting.ambient_occlusion > 0.0) {
            &self.ground_mesh
        } else {
            &None
        };
        for (pipeline, mesh) in [
            (&self.ground, ground),
            (&self.shaded, &self.triangles),
            (&self.highlight, &self.sel_mesh),
            (&self.glass, &self.glass_mesh),
            (&self.behind, &self.behind_mesh),
            (&self.lines, &self.line_mesh),
            (&self.lines, &self.sel_lines),
            (&self.overlay_lines, &self.overlay_mesh),
            (&self.overlay_lines, &self.sel_top),
        ] {
            let Some(m) = mesh else { continue };
            pass.set_pipeline(pipeline);
            pass.set_vertex_buffer(0, m.vertices.slice(..));
            match &m.indices {
                Some(ib) => {
                    pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..m.count, 0, 0..1);
                }
                None => pass.draw(0..m.count, 0..1),
            }
        }
    }
}

/// Targets of `size` for a frame: multisampled colour and depth, and the
/// resolved image (returned again as a texture, to copy from).
fn make_targets(
    device: &wgpu::Device,
    size: [u32; 2],
    extra: wgpu::TextureUsages,
) -> (Targets, wgpu::Texture) {
    let texture = |samples, format, usage| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: samples,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
    };
    let resolve = texture(
        1,
        COLOR_FORMAT,
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | extra,
    );
    (
        Targets {
            size,
            msaa: texture(
                SAMPLES,
                COLOR_FORMAT,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
            )
            .create_view(&Default::default()),
            resolve: resolve.create_view(&Default::default()),
            depth: texture(
                SAMPLES,
                DEPTH_FORMAT,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
            )
            .create_view(&Default::default()),
        },
        resolve,
    )
}

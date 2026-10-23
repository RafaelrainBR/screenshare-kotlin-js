//! A video tile drawn from GPU textures of its own, written only while the window draws.
//!
//! iced's image cache finishes an upload when a layout measures the image, but frees replaced
//! uploads only when the window draws. A minimized, hidden or covered window kept laying out every
//! new frame, so each one stayed in memory (gigabytes in seconds). Here a frame reaches the GPU in
//! `prepare`, which runs only inside a draw, and replaces the tile's textures in place. The frame
//! goes up as its I420 planes and the shader turns it into RGB, so the CPU never converts it.
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use desktop_native::VideoFrameSnapshot;
use iced::{
    Rectangle, mouse,
    wgpu::{self, util::DeviceExt as _},
    widget::shader::{self, Viewport},
};

use crate::TileKey;

/// The newest frame of one tile; `version` grows with every new frame.
pub struct Video {
    pub key: TileKey,
    pub frame: Arc<VideoFrameSnapshot>,
    pub version: u64,
}

impl<Message> shader::Program<Message> for Video {
    type State = ();
    type Primitive = Primitive;

    fn draw(&self, _: &(), _: mouse::Cursor, _: Rectangle) -> Primitive {
        Primitive { key: self.key.clone(), frame: self.frame.clone(), version: self.version, viewports: Mutex::default() }
    }
}

pub struct Primitive {
    key: TileKey,
    frame: Arc<VideoFrameSnapshot>,
    version: u64,
    /// Set in `prepare`: (the whole tile, the frame letterboxed in it), in physical pixels.
    viewports: Mutex<Option<([f32; 4], [f32; 4])>>,
}

impl std::fmt::Debug for Primitive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} {}x{} v{}", self.key, self.frame.width, self.frame.height, self.version)
    }
}

pub struct Pipeline {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    textures: HashMap<TileKey, Planes>,
    /// Tiles drawn since the last trim; the others give their textures back.
    used: HashSet<TileKey>,
}

/// Y, U and V of one tile.
struct Planes {
    planes: [wgpu::Texture; 3],
    bind_group: wgpu::BindGroup,
    size: (u32, u32),
    version: u64,
}

const SHADER: &str = r"
struct Out { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> };
var<private> CORNERS: array<vec2<f32>, 6> = array(
    vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0), vec2(0.0, 1.0), vec2(1.0, 0.0), vec2(1.0, 1.0),
);
@vertex fn vs(@builtin(vertex_index) index: u32) -> Out {
    let corner = CORNERS[index];
    return Out(vec4(corner.x * 2.0 - 1.0, 1.0 - corner.y * 2.0, 0.0, 1.0), corner);
}
@group(0) @binding(0) var luma: texture_2d<f32>;
@group(0) @binding(1) var blue: texture_2d<f32>;
@group(0) @binding(2) var red: texture_2d<f32>;
@group(0) @binding(3) var linear: sampler;
@fragment fn fs(input: Out) -> @location(0) vec4<f32> {
    // BT.601 limited range, as libyuv's I420ToABGR did on the CPU before.
    let y = 1.164 * (textureSample(luma, linear, input.uv).r - 0.0625);
    let u = textureSample(blue, linear, input.uv).r - 0.5;
    let v = textureSample(red, linear, input.uv).r - 0.5;
    let rgb = clamp(vec3(y + 1.596 * v, y - 0.392 * u - 0.813 * v, y + 2.017 * u), vec3(0.0), vec3(1.0));
    return vec4(OUTPUT, 1.0);
}
";

impl shader::Pipeline for Pipeline {
    fn new(device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        // An sRGB target encodes what it is given, so the already encoded colour goes in linear.
        let output = if format.is_srgb() { "select(pow((rgb + 0.055) / 1.055, vec3(2.4)), rgb / 12.92, rgb <= vec3(0.04045))" } else { "rgb" };
        let source = SHADER.replace("OUTPUT", output);
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("video"), source: wgpu::ShaderSource::Wgsl(source.into()) });
        let plane = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("video"),
            entries: &[
                plane(0),
                plane(1),
                plane(2),
                wgpu::BindGroupLayoutEntry { binding: 3, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("video"), bind_group_layouts: &[&layout], push_constant_ranges: &[] });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("video"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs"), buffers: &[], compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor { mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear, ..Default::default() });
        Self { pipeline, layout, sampler, textures: HashMap::new(), used: HashSet::new() }
    }

    fn trim(&mut self) {
        let used = std::mem::take(&mut self.used);
        self.textures.retain(|key, _| used.contains(key));
    }
}

impl shader::Primitive for Primitive {
    type Pipeline = Pipeline;

    fn prepare(&self, pipeline: &mut Pipeline, device: &wgpu::Device, queue: &wgpu::Queue, bounds: &Rectangle, viewport: &Viewport) {
        let frame = &self.frame;
        let size = (frame.width, frame.height);
        pipeline.used.insert(self.key.clone());
        let (layout, sampler) = (&pipeline.layout, &pipeline.sampler);
        let planes = pipeline.textures.entry(self.key.clone()).or_insert_with(|| new_planes(device, queue, layout, sampler, frame));
        if planes.size != size {
            *planes = new_planes(device, queue, layout, sampler, frame);
        } else if planes.version != self.version {
            for (texture, data) in planes.planes.iter().zip([&frame.y, &frame.u, &frame.v]) {
                let extent = texture.size();
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo { texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                    data,
                    wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(extent.width), rows_per_image: Some(extent.height) },
                    extent,
                );
            }
        }
        planes.version = self.version;
        let tile = *bounds * viewport.scale_factor();
        let fit = contain(size, (tile.width, tile.height));
        let letterboxed = [tile.x + (tile.width - fit.0) / 2.0, tile.y + (tile.height - fit.1) / 2.0, fit.0, fit.1];
        *self.viewports.lock().unwrap() = Some(([tile.x, tile.y, tile.width, tile.height], letterboxed));
    }

    fn draw(&self, pipeline: &Pipeline, pass: &mut wgpu::RenderPass<'_>) -> bool {
        let (Some(planes), Some((tile, [x, y, width, height]))) = (pipeline.textures.get(&self.key), *self.viewports.lock().unwrap()) else {
            return true;
        };
        if width >= 1.0 && height >= 1.0 {
            pass.set_viewport(x, y, width, height, 0.0, 1.0);
            pass.set_pipeline(&pipeline.pipeline);
            pass.set_bind_group(0, &planes.bind_group, &[]);
            pass.draw(0..6, 0..1);
        }
        // What iced set before this draw; the images and text after it in this pass rely on it.
        pass.set_viewport(tile[0], tile[1], tile[2], tile[3], 0.0, 1.0);
        true
    }
}

fn new_planes(device: &wgpu::Device, queue: &wgpu::Queue, layout: &wgpu::BindGroupLayout, sampler: &wgpu::Sampler, frame: &VideoFrameSnapshot) -> Planes {
    let (chroma_width, chroma_height) = (frame.width.div_ceil(2), frame.height.div_ceil(2));
    let plane = |(width, height), data: &[u8]| {
        device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("video plane"),
                size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            data,
        )
    };
    let planes = [
        plane((frame.width, frame.height), &frame.y),
        plane((chroma_width, chroma_height), &frame.u),
        plane((chroma_width, chroma_height), &frame.v),
    ];
    let views = planes.each_ref().map(|plane| plane.create_view(&wgpu::TextureViewDescriptor::default()));
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("video planes"),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&views[0]) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&views[1]) },
            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&views[2]) },
            wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(sampler) },
        ],
    });
    Planes { planes, bind_group, size: (frame.width, frame.height), version: 0 }
}

/// The largest size with the frame's aspect that fits in `area`, like the image ContentFit::Contain.
fn contain((width, height): (u32, u32), (area_width, area_height): (f32, f32)) -> (f32, f32) {
    let scale = (area_width / width as f32).min(area_height / height as f32);
    (width as f32 * scale, height as f32 * scale)
}

#[test]
fn a_frame_fits_inside_its_tile_keeping_its_aspect() {
    assert_eq!(contain((2560, 1080), (1280.0, 1080.0)), (1280.0, 540.0));
    assert_eq!(contain((1920, 1080), (168.0, 189.0)), (168.0, 94.5));
    assert_eq!(contain((640, 480), (1280.0, 480.0)), (640.0, 480.0));
}

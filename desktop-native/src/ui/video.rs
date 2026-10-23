//! A video tile drawn from one GPU texture per tile, written only while the window draws.
//!
//! iced's image cache finishes an upload when a layout measures the image, but frees replaced
//! uploads only when the window draws. A minimized, hidden or covered window kept laying out every
//! new frame, so each one stayed in memory (gigabytes in seconds). Here a frame reaches the GPU in
//! `prepare`, which runs only inside a draw, and replaces the tile's texture in place.
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
    texture_format: wgpu::TextureFormat,
    textures: HashMap<TileKey, Texture>,
    /// Tiles drawn since the last trim; the others give their texture back.
    used: HashSet<TileKey>,
}

struct Texture {
    texture: wgpu::Texture,
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
@group(0) @binding(0) var frame: texture_2d<f32>;
@group(0) @binding(1) var linear: sampler;
@fragment fn fs(input: Out) -> @location(0) vec4<f32> {
    return textureSample(frame, linear, input.uv);
}
";

impl shader::Pipeline for Pipeline {
    fn new(device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("video"), source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("video"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
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
        // The same bytes iced's own images would show: sRGB texture only for an sRGB target.
        let texture_format = if format.is_srgb() { wgpu::TextureFormat::Rgba8UnormSrgb } else { wgpu::TextureFormat::Rgba8Unorm };
        Self { pipeline, layout, sampler, texture_format, textures: HashMap::new(), used: HashSet::new() }
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
        let (layout, sampler, format) = (&pipeline.layout, &pipeline.sampler, pipeline.texture_format);
        let texture = pipeline.textures.entry(self.key.clone()).or_insert_with(|| new_texture(device, queue, layout, sampler, format, frame));
        if texture.size != size {
            *texture = new_texture(device, queue, layout, sampler, format, frame);
        } else if texture.version != self.version {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo { texture: &texture.texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                &frame.rgba,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * frame.width), rows_per_image: Some(frame.height) },
                wgpu::Extent3d { width: frame.width, height: frame.height, depth_or_array_layers: 1 },
            );
        }
        texture.version = self.version;
        let tile = *bounds * viewport.scale_factor();
        let fit = contain(size, (tile.width, tile.height));
        let letterboxed = [tile.x + (tile.width - fit.0) / 2.0, tile.y + (tile.height - fit.1) / 2.0, fit.0, fit.1];
        *self.viewports.lock().unwrap() = Some(([tile.x, tile.y, tile.width, tile.height], letterboxed));
    }

    fn draw(&self, pipeline: &Pipeline, pass: &mut wgpu::RenderPass<'_>) -> bool {
        let (Some(texture), Some((tile, [x, y, width, height]))) = (pipeline.textures.get(&self.key), *self.viewports.lock().unwrap()) else {
            return true;
        };
        if width >= 1.0 && height >= 1.0 {
            pass.set_viewport(x, y, width, height, 0.0, 1.0);
            pass.set_pipeline(&pipeline.pipeline);
            pass.set_bind_group(0, &texture.bind_group, &[]);
            pass.draw(0..6, 0..1);
        }
        // What iced set before this draw; the images and text after it in this pass rely on it.
        pass.set_viewport(tile[0], tile[1], tile[2], tile[3], 0.0, 1.0);
        true
    }
}

fn new_texture(device: &wgpu::Device, queue: &wgpu::Queue, layout: &wgpu::BindGroupLayout, sampler: &wgpu::Sampler, format: wgpu::TextureFormat, frame: &VideoFrameSnapshot) -> Texture {
    let texture = device.create_texture_with_data(
        queue,
        &wgpu::TextureDescriptor {
            label: Some("video frame"),
            size: wgpu::Extent3d { width: frame.width, height: frame.height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::LayerMajor,
        &frame.rgba,
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("video frame"),
        layout,
        entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) }],
    });
    Texture { texture, bind_group, size: (frame.width, frame.height), version: 0 }
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

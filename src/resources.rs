use std::{fmt::Debug, path::Path};

use glam::{Vec2, Vec3};
use image::flat::SampleLayout;
use image::imageops::thumbnail;
use image::{DynamicImage, FlatSamples, Rgba};
use pollster::FutureExt;
use tracing::{error, info};
use wgpu::Extent3d;

use crate::application::buffer::Buffer;

fn get_max_mip_level_count(width: u32, height: u32) -> u32 {
    bit_width(u32::max(width, height))
}

const fn bit_width(x: u32) -> u32 {
    if x == 0 {
        0
    } else {
        1 + x.ilog2()
    }
}
pub fn load_texture(
    path: impl AsRef<Path>,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> image::ImageResult<(wgpu::Texture, wgpu::TextureView)> {
    let image = image::open(&path)?;
    let label = path.as_ref().to_str();
    let texture_label = label.map(|s| format!("{s} Texture"));
    let mip_level_count = get_max_mip_level_count(image.width(), image.height());
    let texture_descriptor = wgpu::TextureDescriptor {
        label: texture_label.as_deref(),
        size: wgpu::Extent3d {
            width: image.width(),
            height: image.height(),
            depth_or_array_layers: 1,
        },
        mip_level_count,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    };
    let texture = device.create_texture(&texture_descriptor);
    // Write texture mip level 0
    let destination = wgpu::ImageCopyTextureBase {
        texture: &texture,
        mip_level: 0,
        origin: wgpu::Origin3d::ZERO,
        aspect: wgpu::TextureAspect::All,
    };
    let source = wgpu::ImageDataLayout {
        offset: 0,
        bytes_per_row: Some(4 * texture.size().width),
        rows_per_image: Some(texture.size().height),
    };
    let data = image.into_rgba8().into_raw();
    queue.write_texture(destination, &data, source, texture.size());

    let view_label = label.map(|s| format!("{s} Texture View"));
    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        label: view_label.as_deref(),
        format: Some(texture.format()),
        dimension: Some(wgpu::TextureViewDimension::D2),
        aspect: wgpu::TextureAspect::All,
        base_mip_level: 0,
        mip_level_count: Some(mip_level_count),
        base_array_layer: 0,
        array_layer_count: Some(1),
    });
    Ok((texture, view))
}

pub fn save_texture(
    path: impl AsRef<Path>,
    texture: &wgpu::Texture,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    mip_level: u32,
) {
    let width = texture.width() / (1 << mip_level); // pow(mip_level,2)
    let height = texture.height() / (1 << mip_level);
    let channels = 4;
    let component_byte_size = 1;
    let bytes_per_row = width * channels * component_byte_size;
    // Special case: WebGPU spec forbids texture-to-buffer copy with a
    // bytesPerRow lower than 256 so we first copy to a temporary texture.
    let padded_bytes_per_row = bytes_per_row.max(256);
    let pixel_buffer = Buffer::new(
        device,
        u64::from(padded_bytes_per_row * height),
        wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
    );
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    let source = wgpu::ImageCopyTextureBase {
        texture,
        mip_level,
        origin: wgpu::Origin3d::ZERO,
        aspect: wgpu::TextureAspect::All,
    };
    let destination = wgpu::ImageCopyBuffer {
        buffer: &pixel_buffer.buffer,
        layout: wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(padded_bytes_per_row),
            rows_per_image: Some(height),
        },
    };
    encoder.copy_texture_to_buffer(
        source,
        destination,
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    let command = encoder.finish();
    queue.submit([command]);

    let (sender, receiver) = futures_channel::oneshot::channel();
    pixel_buffer
        .buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, |result| {
            let _ = sender.send(result);
        });
    device.poll(wgpu::Maintain::Wait);
    receiver
        .block_on()
        .expect("communication failed")
        .expect("buffer reading failed");
    let pixels: &[u8] = &pixel_buffer.buffer.slice(..).get_mapped_range();

    let layout = SampleLayout::row_major_packed(4, width, height);
    let buffer = FlatSamples {
        samples: pixels,
        layout,
        color_hint: None,
    };
    info!("{width}x{height} padded: {padded_bytes_per_row} ");

    let view = match buffer.as_view::<Rgba<u8>>() {
        Err(e) => {
            error!("{e}");
            return;
        } // Invalid layout.
        Ok(view) => view,
    };
    thumbnail(&view, width, height)
        .save(path)
        .expect("Unable to save");
}

#[allow(clippy::similar_names)]
pub fn write_mipmaps(queue: &wgpu::Queue, texture: &wgpu::Texture, image: DynamicImage) {
    let data = image.into_rgba8().into_raw();
    let mut mip_level_size = texture.size();
    let mut previous_level_pixels = vec![];
    for level in 0..texture.mip_level_count() {
        let pixels = if level == 0 {
            data.clone()
        } else {
            let mut pixels =
                Vec::with_capacity((4 * mip_level_size.width * mip_level_size.height) as usize);
            for i in 0..mip_level_size.width {
                for j in 0..mip_level_size.height {
                    // Get the corresponding 4 pixels from the previous level

                    let width = mip_level_size.width as usize;
                    let mip_level_index = 2 * width;
                    let height_index = 2 * j as usize;
                    let width_index = 2 * i as usize;
                    let i00 = 4 * (mip_level_index * height_index + width_index);
                    let i01 = 4 * (mip_level_index * height_index + (width_index + 1));
                    let i10 = 4 * (mip_level_index * (height_index + 1) + width_index);
                    let i11 = 4 * (mip_level_index * (height_index + 1) + (width_index + 1));

                    let p00: &[u8] = &previous_level_pixels[i00..(i00 + 4)];
                    let p01 = &previous_level_pixels[i01..(i01 + 4)];
                    let p10 = &previous_level_pixels[i10..(i10 + 4)];
                    let p11 = &previous_level_pixels[i11..(i11 + 4)];
                    // Average
                    let r = (u32::from(p00[0])
                        + u32::from(p01[0])
                        + u32::from(p10[0])
                        + u32::from(p11[0]))
                        / 4;
                    let g = (u32::from(p00[1])
                        + u32::from(p01[1])
                        + u32::from(p10[1])
                        + u32::from(p11[1]))
                        / 4;
                    let b = (u32::from(p00[2])
                        + u32::from(p01[2])
                        + u32::from(p10[2])
                        + u32::from(p11[2]))
                        / 4;
                    let a = (u32::from(p00[3])
                        + u32::from(p01[3])
                        + u32::from(p10[3])
                        + u32::from(p11[3]))
                        / 4;
                    pixels.extend([r as u8, g as u8, b as u8, a as u8]);
                }
            }
            pixels
        };
        let destination = wgpu::ImageCopyTextureBase {
            texture,
            mip_level: level,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        };
        let source = wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(4 * mip_level_size.width),
            rows_per_image: Some(mip_level_size.height),
        };
        queue.write_texture(destination, &pixels, source, mip_level_size);
        mip_level_size.width /= 2;
        mip_level_size.height /= 2;
        previous_level_pixels = pixels;
    }
}
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct VertexAttribute {
    pub position: Vec3,
    pub tangent: Vec3,
    pub bitangent: Vec3,
    pub normal: Vec3,
    pub color: Vec3,
    pub uv: Vec2,
}

fn compute_tangent_frame(face: [VertexAttribute; 3], expected_normal: Vec3) -> (Vec3, Vec3) {
    let e1_pos = face[1].position - face[0].position;
    let e2_pos = face[2].position - face[0].position;

    let e1_uv = face[1].uv - face[0].uv;
    let e2_uv = face[2].uv - face[0].uv;

    let mut tangent = (e1_pos * e2_uv.y - e2_pos * e1_uv.y).normalize();
    let mut bitangent = (e2_pos * e1_uv.x - e1_pos * e2_uv.x).normalize();
    let mut normal = tangent.cross(bitangent);

    if normal.dot(expected_normal) < 0.0 {
        tangent = -tangent;
    }

    normal = expected_normal;
    tangent = (tangent - tangent.dot(normal) * normal).normalize();
    bitangent = normal.cross(tangent);

    (tangent, bitangent)
}

impl VertexAttributeLayout for VertexAttribute {
    fn layout() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        const ATTRIBUTES: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3,2=>Float32x3,3=>Float32x3,4=>Float32x3,5=>Float32x2];
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRIBUTES,
        }
    }
}

pub trait VertexAttributeLayout {
    fn layout() -> wgpu::VertexBufferLayout<'static>;
}

pub fn load_geometry(path: impl AsRef<Path> + Debug) -> (Vec<VertexAttribute>, Vec<u32>) {
    let (models, _) = tobj::load_obj(
        path,
        &tobj::LoadOptions {
            single_index: true,
            triangulate: true,
            ignore_points: true,
            ignore_lines: true,
        },
    )
    .expect("Failed to OBJ load file");
    let mut vertices = vec![];
    let mut indices: Vec<u32> = vec![];
    for model in &models {
        let mesh = &model.mesh;
        indices.extend(&mesh.indices);
        let mut positions = Vec::with_capacity(mesh.positions.len() / 3);
        for p in mesh.positions.chunks_exact(3) {
            positions.push(Vec3::new(p[0], p[1], p[2]));
        }

        let normals = if mesh.normals.is_empty() {
            vec![Vec3::ZERO; positions.len()]
        } else {
            let mut normals = Vec::with_capacity(positions.len());
            for n in mesh.normals.chunks_exact(3) {
                normals.push(Vec3::new(n[0], n[1], n[2]));
            }
            normals
        };
        let colors = if mesh.vertex_color.is_empty() {
            vec![Vec3::ZERO; positions.len()]
        } else {
            let mut colors = Vec::with_capacity(positions.len());
            for c in mesh.vertex_color.chunks_exact(3) {
                colors.push(Vec3::new(c[0], c[1], c[2]));
            }
            colors
        };

        let uvs = if mesh.texcoords.is_empty() {
            vec![Vec2::ZERO; positions.len()]
        } else {
            let mut uvs = Vec::with_capacity(mesh.texcoords.len());
            for uv in mesh.texcoords.chunks_exact(2) {
                uvs.push(Vec2::new(uv[0], 1.0 - uv[1]));
            }
            uvs
        };

        vertices.extend(positions.into_iter().zip(normals).zip(colors).zip(uvs).map(
            |(((p, n), c), t)| VertexAttribute {
                position: p,
                tangent: Vec3::Y,
                bitangent: Vec3::Z,
                normal: n,
                color: c,
                uv: t,
            },
        ));
    }

    for i in indices.chunks_exact(3) {
        let v1 = vertices[i[0] as usize];
        let v2 = vertices[i[1] as usize];
        let v3 = vertices[i[2] as usize];
        for j in 0..3 {
            let v = &mut vertices[i[j] as usize];
            let (tangent, bitangent) = compute_tangent_frame([v1, v2, v3], v.normal);
            v.tangent = tangent;
            v.bitangent = bitangent;
        }
    }

    (vertices, indices)
}

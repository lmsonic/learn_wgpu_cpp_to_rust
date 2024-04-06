use std::{fmt::Debug, path::Path};

use glam::{Vec2, Vec3};
use image::DynamicImage;

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
    let mip_level_count = bit_width(u32::max(image.width(), image.height()));
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
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    };
    let view_label = label.map(|s| format!("{s} Texture View"));
    let texture = device.create_texture(&texture_descriptor);
    write_mipmaps(queue, &texture, image);
    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        label: view_label.as_deref(),
        format: Some(texture.format()),
        dimension: Some(wgpu::TextureViewDimension::D2),
        aspect: wgpu::TextureAspect::All,
        base_mip_level: 0,
        mip_level_count: Some(1),
        base_array_layer: 0,
        array_layer_count: Some(1),
    });
    Ok((texture, view))
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
    pub normal: Vec3,
    pub color: Vec3,
    pub uv: Vec2,
}

impl VertexAttributeLayout for VertexAttribute {
    fn layout() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        const ATTRIBUTES: [wgpu::VertexAttribute; 4] =
            wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3,2=>Float32x3,3=>Float32x2];
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

pub fn load_geometry(path: impl AsRef<Path> + Debug) -> Vec<VertexAttribute> {
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
    for model in &models {
        let mesh = &model.mesh;
        vertices.reserve(mesh.indices.len());
        for index in &mesh.indices {
            let i = *index as usize;
            vertices.push(VertexAttribute {
                position: Vec3::new(
                    mesh.positions[i * 3],
                    // Z is up
                    -mesh.positions[i * 3 + 2],
                    mesh.positions[i * 3 + 1],
                ),
                normal: if mesh.normals.is_empty() {
                    Vec3::ZERO
                } else {
                    Vec3::new(
                        mesh.normals[i * 3],
                        // Z is up
                        -mesh.normals[i * 3 + 2],
                        mesh.normals[i * 3 + 1],
                    )
                },
                color: if mesh.vertex_color.is_empty() {
                    Vec3::ONE
                } else {
                    Vec3::new(
                        mesh.vertex_color[i * 3],
                        -mesh.vertex_color[i * 3 + 2],
                        mesh.vertex_color[i * 3 + 1],
                    )
                },
                uv: if mesh.texcoords.is_empty() {
                    Vec2::ZERO
                } else {
                    Vec2::new(
                        mesh.texcoords[i * 2],
                        // Modern graphics APIs use a different UV coordinate system than the OBJ file format.
                        1.0 - mesh.texcoords[i * 2 + 1],
                    )
                },
            });
        }
    }
    vertices
}

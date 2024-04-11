use std::{fmt::Debug, mem};

use wgpu::util::DeviceExt;

pub struct VertexBuffer<A> {
    pub(crate) vertices: Vec<A>,
    // indices: Vec<u32>,
    pub(crate) buffer: wgpu::Buffer,
}

impl<A> VertexBuffer<A>
where
    A: Debug + Clone + Copy + bytemuck::Pod + bytemuck::Zeroable,
{
    pub(crate) fn new(vertices: Vec<A>, device: &wgpu::Device) -> Self {
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::VERTEX,
        });
        // let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        //     label: Some("Index Buffer"),
        //     contents: bytemuck::cast_slice(&indices),
        //     usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::INDEX,
        // });
        Self {
            vertices,
            // indices,
            buffer,
        }
    }
}

pub struct UniformBuffer<T> {
    pub(crate) data: T,
    pub(crate) buffer: wgpu::Buffer,
}

impl<T> UniformBuffer<T>
where
    T: Debug + Clone + Copy + bytemuck::Pod + bytemuck::Zeroable,
{
    pub(crate) fn new(data: T, device: &wgpu::Device) -> Self {
        assert!(
            mem::align_of::<T>() % 4 == 0,
            "Uniform alignment needs to be multiple of 4"
        );
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&[data]),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
        });
        Self { data, buffer }
    }

    pub(crate) fn update(&self, queue: &wgpu::Queue) {
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[self.data]));
    }
}

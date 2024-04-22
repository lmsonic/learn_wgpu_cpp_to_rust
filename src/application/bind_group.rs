use super::texture::Texture;

pub struct BindGroup {
    pub(crate) bind_group_layout: wgpu::BindGroupLayout,
    pub(crate) bind_group: wgpu::BindGroup,
}

impl BindGroup {
    pub(crate) fn new_compute(
        device: &wgpu::Device,
        input_buffers: &[&wgpu::Buffer],
        output_buffers: &[&wgpu::Buffer],
    ) -> Self {
        let mut layout_entries = vec![];
        let mut binding = 0;
        for _ in input_buffers {
            layout_entries.push(wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            });
            binding += 1;
        }

        for _ in output_buffers {
            layout_entries.push(wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            });
            binding += 1;
        }

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &layout_entries,
        });
        binding = 0;
        let mut bind_group_entries = vec![];

        for input_buffer in input_buffers {
            bind_group_entries.push(wgpu::BindGroupEntry {
                binding,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: input_buffer,
                    offset: 0,
                    size: None,
                }),
            });
            binding += 1;
        }

        for output_buffer in output_buffers {
            bind_group_entries.push(wgpu::BindGroupEntry {
                binding,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: output_buffer,
                    offset: 0,
                    size: None,
                }),
            });
            binding += 1;
        }

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,

            layout: &bind_group_layout,
            entries: &bind_group_entries,
        });
        Self {
            bind_group_layout,
            bind_group,
        }
    }
    pub(crate) fn new(
        device: &wgpu::Device,
        uniform_buffers: &[&wgpu::Buffer],
        textures: &[&Texture],
    ) -> Self {
        let mut layout_entries = vec![];
        let mut binding = 0;
        for _ in uniform_buffers {
            layout_entries.push(wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            });
            binding += 1;
        }

        for _ in textures {
            layout_entries.extend([
                wgpu::BindGroupLayoutEntry {
                    binding,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: binding + 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ]);
            binding += 2;
        }

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Uniform Bind Group Layout"),
            entries: &layout_entries,
        });
        binding = 0;
        let mut bind_group_entries = vec![];

        for uniforms in uniform_buffers {
            bind_group_entries.push(wgpu::BindGroupEntry {
                binding,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: uniforms,
                    offset: 0,
                    size: None,
                }),
            });
            binding += 1;
        }

        for texture in textures {
            bind_group_entries.extend([
                wgpu::BindGroupEntry {
                    binding,
                    resource: wgpu::BindingResource::TextureView(&texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: binding + 1,
                    resource: wgpu::BindingResource::Sampler(&texture.sampler),
                },
            ]);
            binding += 2;
        }

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Uniform Bind Group Layout"),

            layout: &bind_group_layout,
            entries: &bind_group_entries,
        });
        Self {
            bind_group_layout,
            bind_group,
        }
    }
}

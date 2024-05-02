use crate::{
    application::{buffer::DataBuffer, texture::Texture, ComputeUniforms},
    resources::save_texture,
};

pub fn generate_mipmaps(texture: &wgpu::Texture, device: &wgpu::Device, queue: &wgpu::Queue) {
    // Create mip views and sizes
    let mut mip_sizes = vec![texture.size()];
    let mut mip_views = vec![];
    let mip_level_count = texture.mip_level_count();
    for level in 0..mip_level_count {
        mip_views.push(texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some(&format!("mip view: {level}")),
            format: Some(texture.format()),
            dimension: Some(wgpu::TextureViewDimension::D2),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: level,
            mip_level_count: Some(1),
            base_array_layer: 0,
            array_layer_count: Some(1),
        }));
        if level > 0 {
            let previous_size = mip_sizes[level as usize - 1];
            mip_sizes.push(wgpu::Extent3d {
                width: previous_size.width / 2,
                height: previous_size.height / 2,
                depth_or_array_layers: previous_size.depth_or_array_layers / 2,
            });
        }
    }

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                count: None,
            },
        ],
    });

    // Create bind groups in advance because of rust borrow rules
    let mut bind_groups = vec![];
    for level in 1..mip_level_count {
        bind_groups.push(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&mip_views[level as usize - 1]),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&mip_views[level as usize]),
                },
            ],
        }));
    }

    let compute_shader = device.create_shader_module(wgpu::include_wgsl!("mipmap_generation.wgsl"));

    let compute_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Compute Pipeline Layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("Compute Pipeline"),
        layout: Some(&compute_pipeline_layout),
        module: &compute_shader,
        entry_point: "compute_mip_map",
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

    let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some("Compute Pass"),
        timestamp_writes: None,
    });
    compute_pass.set_pipeline(&compute_pipeline);

    for level in 1..mip_level_count {
        // We write to each mip level using the previous level
        compute_pass.set_bind_group(0, &bind_groups[level as usize - 1], &[]);
        let invocation_count_x = texture.width();
        let invocation_count_y = texture.height();
        let workgroup_size_per_dim = 8;
        // This ceils invocation_count / workgroup_size
        let workgroup_count_x =
            (invocation_count_x + workgroup_size_per_dim - 1) / workgroup_size_per_dim;
        let workgroup_count_y =
            (invocation_count_y + workgroup_size_per_dim - 1) / workgroup_size_per_dim;
        compute_pass.dispatch_workgroups(workgroup_count_x, workgroup_count_y, 1);
    }

    drop(compute_pass);

    let command = encoder.finish();

    queue.submit([command]);

    // for level in 1..mip_level_count {
    //     save_texture(
    //         format!(
    //             "{}_mip{level}.png",
    //             path.as_ref().with_extension("").display()
    //         ),
    //         &texture,
    //         device,
    //         queue,
    //         level,
    //     );
    // }
}
#[allow(clippy::too_many_lines)]
pub fn compute_filter(
    texture: &Texture,
    compute_uniforms: &DataBuffer<ComputeUniforms>,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) {
    let output_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: texture.texture.width(),
            height: texture.texture.height(),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let output_view = output_texture.create_view(&wgpu::TextureViewDescriptor {
        label: None,
        format: Some(output_texture.format()),
        dimension: Some(wgpu::TextureViewDimension::D2),
        aspect: wgpu::TextureAspect::All,
        base_mip_level: 0,
        mip_level_count: Some(1),
        base_array_layer: 0,
        array_layer_count: Some(1),
    });
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&texture.view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&output_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &compute_uniforms.buffer,
                    offset: 0,
                    size: None,
                }),
            },
        ],
    });
    let compute_shader = device.create_shader_module(wgpu::include_wgsl!("sobel.wgsl"));

    let compute_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Compute Pipeline Layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("Compute Pipeline"),
        layout: Some(&compute_pipeline_layout),
        module: &compute_shader,
        entry_point: "compute_filter",
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

    let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some("Compute Pass"),
        timestamp_writes: None,
    });
    compute_pass.set_pipeline(&compute_pipeline);
    compute_pass.set_bind_group(0, &bind_group, &[]);
    let invocation_count_x = texture.texture.width();
    let invocation_count_y = texture.texture.height();
    let workgroup_size_per_dim = 8;
    // This ceils invocation_count / workgroup_size
    let workgroup_count_x =
        (invocation_count_x + workgroup_size_per_dim - 1) / workgroup_size_per_dim;
    let workgroup_count_y =
        (invocation_count_y + workgroup_size_per_dim - 1) / workgroup_size_per_dim;
    compute_pass.dispatch_workgroups(workgroup_count_x, workgroup_count_y, 1);

    drop(compute_pass);

    let command = encoder.finish();

    queue.submit([command]);

    save_texture("resources/sobel.png", &output_texture, device, queue, 0);
}

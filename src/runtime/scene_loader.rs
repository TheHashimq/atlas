use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;

use crate::engine::math::transform::Transform;
use crate::engine::render::mesh::{Mesh, Vertex};
use crate::engine::render::material::Material;

use crate::runtime::scene::RenderObject;

pub fn load_gltf_from_bytes(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    bytes: &[u8]
) -> Vec<RenderObject> {
    let (gltf, buffers, images) = match gltf::import_slice(bytes) {
        Ok(r)  => r,
        Err(e) => {
            log::error!("gltf::import_slice failed: {:?}", e);
            return Vec::new();
        }
    };

    // 1. Load all textures
    // We'll load them all as Rgba8Unorm (Linear) and use sRGB views for base color/emissive in some cases,
    // OR we can just load them as sRGB and handle linear conversion in shader.
    // The most compliant way is Rgba8UnormSrgb for Color/Emissive and Rgba8Unorm for Normal/MR.
    let mut loaded_textures = HashMap::new();
    println!("Found {} glTF textures", gltf.textures().count());
    for texture in gltf.textures() {
        let img = &images[texture.source().index()];
        
        // We handle format selection during material assignment if possible, 
        // but for now let's load as Rgba8Unorm (linear) and sRGB versions.
        // Or simpler: load two versions of each texture if it's used as both (rare).
        // For atlas, we'll load as Rgba8UnormSrgb and handle linear in shader for non-color textures,
        // OR load as Rgba8Unorm and handle sRGB in shader.
        // BUT the user specifically asked for correct formats:
        // TextureFormat::Rgba8UnormSrgb for base color.
        // Metallic-roughness, normal, and emissive should use linear formats.
        
        // Load as Rgba8Unorm (Linear) by default.
        let format = wgpu::TextureFormat::Rgba8Unorm;
        
        let size = wgpu::Extent3d {
            width: img.width,
            height: img.height,
            depth_or_array_layers: 1,
        };
        let gpu_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("gltf_tex_{}", texture.index())),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[wgpu::TextureFormat::Rgba8UnormSrgb], // Allow sRGB view
        });

        // Convert to RGBA8 if needed
        let (data, bpr) = match img.format {
            gltf::image::Format::R8G8B8 => {
                let mut rgba = Vec::with_capacity(img.pixels.len() / 3 * 4);
                for i in 0..img.pixels.len() / 3 {
                    rgba.push(img.pixels[i * 3]);
                    rgba.push(img.pixels[i * 3 + 1]);
                    rgba.push(img.pixels[i * 3 + 2]);
                    rgba.push(255);
                }
                (rgba, 4 * img.width)
            }
            gltf::image::Format::R8G8B8A8 => {
                (img.pixels.clone(), 4 * img.width)
            }
            _ => {
                log::warn!("Unsupported texture format: {:?}", img.format);
                (img.pixels.clone(), 4 * img.width) // Fallback
            }
        };

        if data.len() >= (bpr * img.height) as usize {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &gpu_tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bpr),
                    rows_per_image: Some(img.height),
                },
                size,
            );
        }

        let linear_view = Rc::new(gpu_tex.create_view(&wgpu::TextureViewDescriptor {
            label: Some(&format!("gltf_tex_{}_linear", texture.index())),
            format: Some(wgpu::TextureFormat::Rgba8Unorm),
            ..Default::default()
        }));
        let srgb_view = Rc::new(gpu_tex.create_view(&wgpu::TextureViewDescriptor {
            label: Some(&format!("gltf_tex_{}_srgb", texture.index())),
            format: Some(wgpu::TextureFormat::Rgba8UnormSrgb),
            ..Default::default()
        }));
        
        // Create sampler
        let sampler_def = texture.sampler();
        let wrap_u = match sampler_def.wrap_s() {
            gltf::texture::WrappingMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
            gltf::texture::WrappingMode::MirroredRepeat => wgpu::AddressMode::MirrorRepeat,
            _ => wgpu::AddressMode::Repeat,
        };
        let wrap_v = match sampler_def.wrap_t() {
            gltf::texture::WrappingMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
            gltf::texture::WrappingMode::MirroredRepeat => wgpu::AddressMode::MirrorRepeat,
            _ => wgpu::AddressMode::Repeat,
        };
        let sampler = Rc::new(device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wrap_u,
            address_mode_v: wrap_v,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        }));

        loaded_textures.insert(texture.index(), (linear_view, srgb_view, sampler));
    }

    let mut scene_objects = Vec::new();

    // 2. Traverse nodes and create objects
    for scene in gltf.scenes() {
        for node in scene.nodes() {
            load_node_recursive(
                device,
                queue,
                &buffers,
                &node,
                glam::Mat4::IDENTITY,
                &loaded_textures,
                &mut scene_objects,
            );
        }
    }

    scene_objects
}

fn load_node_recursive(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    buffers: &[gltf::buffer::Data],
    node: &gltf::Node,
    parent_transform: glam::Mat4,
    loaded_textures: &HashMap<usize, (Rc<wgpu::TextureView>, Rc<wgpu::TextureView>, Rc<wgpu::Sampler>)>,
    scene_objects: &mut Vec<RenderObject>,
) {
    let local_matrix = glam::Mat4::from_cols_array_2d(&node.transform().matrix());
    let world_matrix = parent_transform * local_matrix;

    if let Some(mesh_def) = node.mesh() {
        for primitive in mesh_def.primitives() {
            if primitive.mode() != gltf::mesh::Mode::Triangles {
                continue;
            }

            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

            let positions: Vec<[f32; 3]> = reader.read_positions()
                .map(|it| it.collect())
                .unwrap_or_default();
            if positions.is_empty() { continue; }

            let normals: Vec<[f32; 3]> = reader.read_normals()
                .map(|it| it.collect())
                .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; positions.len()]);

            let uvs: Vec<[f32; 2]> = reader.read_tex_coords(0)
                .map(|it| it.into_f32().collect())
                .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);

            let tangents: Vec<[f32; 4]> = reader.read_tangents()
                .map(|it| it.collect())
                .unwrap_or_else(|| vec![[1.0, 0.0, 0.0, 1.0]; positions.len()]);

            let indices: Vec<u32> = reader.read_indices()
                .map(|it| it.into_u32().collect())
                .unwrap_or_else(|| (0..positions.len() as u32).collect());

            let mut vertices = Vec::with_capacity(positions.len());
            let mut bounds_min = glam::Vec3::splat(f32::MAX);
            let mut bounds_max = glam::Vec3::splat(f32::MIN);

            for i in 0..positions.len() {
                let p = positions[i];
                let pos_vec = glam::Vec3::from(p);
                bounds_min = bounds_min.min(pos_vec);
                bounds_max = bounds_max.max(pos_vec);

                vertices.push(Vertex {
                    position: p,
                    normal: normals[i],
                    uv: uvs[i],
                    tangent: tangents[i],
                });
            }

            let mesh = Rc::new(Mesh::upload(device, &vertices, &indices, mesh_def.name().unwrap_or("Primitive")));
            
            let gltf_mat = primitive.material();
            let pbr = gltf_mat.pbr_metallic_roughness();
            
            let mut base_color_tex = None;
            let mut mr_tex         = None;
            let mut normal_tex     = None;
            let mut emissive_tex   = None;
            let mut occlusion_tex  = None;
            let mut sampler        = None;
            
            // Base Color (sRGB)
            if let Some(tex_info) = pbr.base_color_texture() {
                if let Some((_, srgb, s)) = loaded_textures.get(&tex_info.texture().index()) {
                    base_color_tex = Some(srgb.clone());
                    sampler        = Some(s.clone());
                }
            }

            // Metallic-Roughness (Linear)
            if let Some(tex_info) = pbr.metallic_roughness_texture() {
                if let Some((linear, _, s)) = loaded_textures.get(&tex_info.texture().index()) {
                    mr_tex = Some(linear.clone());
                    if sampler.is_none() { sampler = Some(s.clone()); }
                }
            }

            // Normal Map (Linear)
            if let Some(tex_info) = gltf_mat.normal_texture() {
                if let Some((linear, _, s)) = loaded_textures.get(&tex_info.texture().index()) {
                    normal_tex = Some(linear.clone());
                    if sampler.is_none() { sampler = Some(s.clone()); }
                }
            }

            // Emissive (sRGB)
            if let Some(tex_info) = gltf_mat.emissive_texture() {
                if let Some((_, srgb, s)) = loaded_textures.get(&tex_info.texture().index()) {
                    emissive_tex = Some(srgb.clone());
                    if sampler.is_none() { sampler = Some(s.clone()); }
                }
            }

            // Occlusion Map (Linear)
            if let Some(tex_info) = gltf_mat.occlusion_texture() {
                if let Some((linear, _, s)) = loaded_textures.get(&tex_info.texture().index()) {
                    occlusion_tex = Some(linear.clone());
                    if sampler.is_none() { sampler = Some(s.clone()); }
                }
            }

            let material = Material {
                base_color_factor : pbr.base_color_factor(),
                emissive_factor   : gltf_mat.emissive_factor(),
                roughness_factor  : pbr.roughness_factor(),
                metallic_factor   : pbr.metallic_factor(),
                occlusion_factor  : gltf_mat.occlusion_texture().map(|t| t.strength()).unwrap_or(1.0),
                is_light          : 0.0,
                _pad              : [0.0; 1],
            };

            let mut transform = Transform::identity();
            transform.set_from_matrix(world_matrix);

            scene_objects.push(RenderObject {
                mesh,
                transform: Rc::new(RefCell::new(transform)),
                material,
                base_color_tex,
                metallic_rough_tex: mr_tex,
                normal_tex,
                emissive_tex,
                occlusion_tex,
                sampler,
                bounds_min,
                bounds_max,
            });
        }
    }

    for child in node.children() {
        load_node_recursive(device, queue, buffers, &child, world_matrix, loaded_textures, scene_objects);
    }
}
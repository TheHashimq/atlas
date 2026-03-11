use std::rc::Rc;
use std::cell::RefCell;

use crate::engine::math::transform::Transform;
use crate::engine::render::mesh::{Mesh, Vertex};
use crate::engine::render::material::Material;
use crate::runtime::scene::RenderObject;

pub fn load_gltf_from_bytes(device: &wgpu::Device, bytes: &[u8]) -> Option<RenderObject> {
    let (gltf, buffers, _) = match gltf::import_slice(bytes) {
        Ok(r)  => r,
        Err(e) => {
            log::error!("gltf::import_slice failed: {:?}", e);
            return None;
        }
    };

    // Collect ALL primitive data and merge into one draw call.
    // This handles models split across multiple meshes/primitives.
    let mut all_vertices: Vec<Vertex> = Vec::new();
    let mut all_indices:  Vec<u16>    = Vec::new();
    let mut first_material: Option<Material> = None;

    for mesh in gltf.meshes() {
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

            // ---- Positions (required) ----
            let positions: Vec<[f32; 3]> = match reader.read_positions() {
                Some(it) => it.collect(),
                None     => { log::warn!("Primitive has no positions, skipping."); continue; }
            };
            if positions.is_empty() { continue; }

            // ---- Normals ----
            let normals: Vec<[f32; 3]> = reader.read_normals()
                .map(|it| it.collect())
                .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; positions.len()]);

            // ---- UVs ----
            let uvs: Vec<[f32; 2]> = reader.read_tex_coords(0)
                .map(|it| it.into_f32().collect())
                .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);

            // ---- Tangents ----
            let tangents: Vec<[f32; 3]> = reader.read_tangents()
                .map(|it| it.map(|t| [t[0], t[1], t[2]]).collect())
                .unwrap_or_else(|| vec![[1.0, 0.0, 0.0]; positions.len()]);

            // ---- Indices ----
            let raw_indices: Vec<u32> = match reader.read_indices() {
                Some(it) => it.into_u32().collect(),
                None     => {
                    // Generate sequential strip indices
                    (0..positions.len() as u32).collect()
                }
            };
            if raw_indices.is_empty() { continue; }

            // ---- Check for u16 overflow — split if needed ----
            let base_vertex = all_vertices.len() as u32;
            if base_vertex + positions.len() as u32 > 65535 {
                log::warn!("Mesh vertex count exceeds u16 range — some geometry may be skipped.");
                // Skip overflow primitives; a production loader would use u32 index format
                continue;
            }

            // ---- Append vertices ----
            for i in 0..positions.len() {
                all_vertices.push(Vertex {
                    position: positions[i],
                    normal:   normals.get(i).copied().unwrap_or([0.0, 1.0, 0.0]),
                    tangent:  tangents.get(i).copied().unwrap_or([1.0, 0.0, 0.0]),
                    uv:       uvs.get(i).copied().unwrap_or([0.0, 0.0]),
                    _pad:     [0.0],
                });
            }

            // ---- Append remapped indices ----
            if raw_indices.is_empty() {
                for i in 0..positions.len() as u32 {
                    all_indices.push((base_vertex + i) as u16);
                }
            } else {
                for idx in raw_indices {
                    all_indices.push((base_vertex + idx) as u16);
                }
            }

            // ---- Material — capture once from first primitive ----
            if first_material.is_none() {
                let gltf_mat  = primitive.material();
                let pbr       = gltf_mat.pbr_metallic_roughness();
                let base_color = pbr.base_color_factor();

                // If the model has an essentially white/default albedo, give it a
                // cinematic gunmetal look that photographs well in the space scene.
                let albedo = if base_color[0] > 0.9 && base_color[1] > 0.9 && base_color[2] > 0.9 {
                    [0.55f32, 0.55, 0.60, 1.0]
                } else {
                    base_color
                };

                first_material = Some(Material {
                    albedo,
                    roughness: pbr.roughness_factor().max(0.25), // clamp for PBR plausibility
                    metallic:  pbr.metallic_factor(),
                    emissive:  gltf_mat.emissive_factor().iter().cloned().fold(0.0f32, f32::max),
                    is_light:  0.0,
                });
            }
        }
    }

    if all_vertices.is_empty() || all_indices.is_empty() {
        log::error!("hovercar.glb had no usable geometry after parsing.");
        return None;
    }

    log::info!(
        "Loaded hovercar: {} vertices, {} triangles",
        all_vertices.len(),
        all_indices.len() / 3,
    );

    let mesh      = Rc::new(Mesh::upload(device, &all_vertices, &all_indices, "Hovercar"));
    let material  = first_material.unwrap_or_else(Material::default_blue);
    let transform = Rc::new(RefCell::new(Transform::identity()));

    Some(RenderObject { mesh, transform, material })
}
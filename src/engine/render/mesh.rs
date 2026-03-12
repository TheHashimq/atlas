use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position : [f32; 3],
    pub normal   : [f32; 3],
    pub uv       : [f32; 2],
    pub tangent  : [f32; 4], // XYZ + sign
}

impl Vertex {
    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        use std::mem::size_of;
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute { offset: 0,  shader_location: 0, format: wgpu::VertexFormat::Float32x3 }, // position
                wgpu::VertexAttribute { offset: 12, shader_location: 1, format: wgpu::VertexFormat::Float32x3 }, // normal
                wgpu::VertexAttribute { offset: 24, shader_location: 2, format: wgpu::VertexFormat::Float32x2 }, // uv
                wgpu::VertexAttribute { offset: 32, shader_location: 3, format: wgpu::VertexFormat::Float32x4 }, // tangent
            ],
        }
    }

    fn new(position: [f32; 3], normal: [f32; 3], uv: [f32; 2], tangent: [f32; 4]) -> Self {
        Self { position, normal, uv, tangent }
    }
}

pub struct Mesh {
    pub vertex_buffer : wgpu::Buffer,
    pub index_buffer  : wgpu::Buffer,
    pub index_count   : u32,
}

impl Mesh {
    pub fn upload(device: &wgpu::Device, vertices: &[Vertex], indices: &[u32], label: &str) -> Self {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label:    Some(&format!("{} VB", label)),
            contents: bytemuck::cast_slice(vertices),
            usage:    wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label:    Some(&format!("{} IB", label)),
            contents: bytemuck::cast_slice(indices),
            usage:    wgpu::BufferUsages::INDEX,
        });
        Self { vertex_buffer, index_buffer, index_count: indices.len() as u32 }
    }

    pub fn pyramid(device: &wgpu::Device) -> Self {
        let v = |p, n, uv| Vertex::new(p, n, uv, [1.0, 0.0, 0.0, 1.0]);
        let vertices = [
            // Front
            v([ 0.0, 1.0,  0.0], [0.0, 0.707,  0.707], [0.5, 0.0]),
            v([-1.0,-1.0,  1.0], [0.0, 0.707,  0.707], [0.0, 1.0]),
            v([ 1.0,-1.0,  1.0], [0.0, 0.707,  0.707], [1.0, 1.0]),
            // Right
            v([ 0.0, 1.0,  0.0], [0.707, 0.707, 0.0], [0.5, 0.0]),
            v([ 1.0,-1.0,  1.0], [0.707, 0.707, 0.0], [0.0, 1.0]),
            v([ 1.0,-1.0, -1.0], [0.707, 0.707, 0.0], [1.0, 1.0]),
            // Back
            v([ 0.0, 1.0,  0.0], [0.0, 0.707, -0.707], [0.5, 0.0]),
            v([ 1.0,-1.0, -1.0], [0.0, 0.707, -0.707], [0.0, 1.0]),
            v([-1.0,-1.0, -1.0], [0.0, 0.707, -0.707], [1.0, 1.0]),
            // Left
            v([ 0.0, 1.0,  0.0], [-0.707, 0.707, 0.0], [0.5, 0.0]),
            v([-1.0,-1.0, -1.0], [-0.707, 0.707, 0.0], [0.0, 1.0]),
            v([-1.0,-1.0,  1.0], [-0.707, 0.707, 0.0], [1.0, 1.0]),
            // Bottom
            v([-1.0,-1.0,  1.0], [0.0,-1.0, 0.0], [0.0, 0.0]),
            v([-1.0,-1.0, -1.0], [0.0,-1.0, 0.0], [0.0, 1.0]),
            v([ 1.0,-1.0, -1.0], [0.0,-1.0, 0.0], [1.0, 1.0]),
            v([ 1.0,-1.0,  1.0], [0.0,-1.0, 0.0], [1.0, 0.0]),
        ];
        #[rustfmt::skip]
        let indices: &[u32] = &[
             0, 1, 2,  3, 4, 5,  6, 7, 8,  9,10,11,
            12,13,14, 12,14,15,
        ];
        Self::upload(device, &vertices, indices, "Pyramid")
    }

    pub fn sphere(device: &wgpu::Device) -> Self {
        let stacks = 32u32;
        let slices = 32u32;
        let mut vertices = Vec::new();
        let mut indices  = Vec::new();

        for i in 0..=stacks {
            let v   = i as f32 / stacks as f32;
            let phi = v * std::f32::consts::PI;

            for j in 0..=slices {
                let u     = j as f32 / slices as f32;
                let theta = u * std::f32::consts::TAU;

                let x = theta.sin() * phi.sin();
                let y = phi.cos();
                let z = theta.cos() * phi.sin();

                vertices.push(Vertex::new(
                    [x, y, z],
                    [x, y, z],
                    [u, v],
                    [1.0, 0.0, 0.0, 1.0],
                ));
            }
        }

        for i in 0..stacks {
            for j in 0..slices {
                let first  = i * (slices + 1) + j;
                let second = first + slices + 1;
                indices.extend_from_slice(&[
                    first as u32, second as u32, (first + 1) as u32,
                    second as u32, (second + 1) as u32, (first + 1) as u32,
                ]);
            }
        }

        Self::upload(device, &vertices, &indices, "Sphere")
    }

    /// Flat ground plane, N subdivisions
    pub fn ground_plane(device: &wgpu::Device, size: f32, subdivisions: u32) -> Self {
        let n = subdivisions + 1;
        let mut vertices = Vec::new();
        let mut indices  = Vec::new();

        for z in 0..=subdivisions {
            for x in 0..=subdivisions {
                let fx = (x as f32 / subdivisions as f32 - 0.5) * size;
                let fz = (z as f32 / subdivisions as f32 - 0.5) * size;
                let u  = x as f32 / subdivisions as f32;
                let v  = z as f32 / subdivisions as f32;
                vertices.push(Vertex::new(
                    [fx, 0.0, fz],
                    [0.0, 1.0, 0.0],
                    [u * 8.0, v * 8.0],  // tile UVs
                    [1.0, 0.0, 0.0, 1.0],
                ));
            }
        }

        for z in 0..subdivisions {
            for x in 0..subdivisions {
                let tl = (z * n + x) as u32;
                let tr = tl + 1;
                let bl = ((z + 1) * n + x) as u32;
                let br = bl + 1;
                indices.extend_from_slice(&[tl, bl, tr, tr, bl, br]);
            }
        }

        Self::upload(device, &vertices, &indices, "Ground")
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vertex_layout() {
        let layout = Vertex::layout();
        assert_eq!(layout.array_stride, 48); // 3*4 + 3*4 + 2*4 + 4*4 = 12 + 12 + 8 + 16 = 48
        assert_eq!(layout.attributes.len(), 4);
        
        // Position
        assert_eq!(layout.attributes[0].shader_location, 0);
        assert_eq!(layout.attributes[0].offset, 0);
        
        // Normal
        assert_eq!(layout.attributes[1].shader_location, 1);
        assert_eq!(layout.attributes[1].offset, 12);
        
        // UV
        assert_eq!(layout.attributes[2].shader_location, 2);
        assert_eq!(layout.attributes[2].offset, 24);
    }
}

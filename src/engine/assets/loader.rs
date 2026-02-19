//! Asset Loader (Owned Data)

use super::gltf::GltfAsset;

pub struct AssetLoader;

impl AssetLoader {
    pub fn load_gltf(path: &str) -> GltfAsset {
        let document = gltf::Gltf::open(path)
            .expect("Failed to load glTF");

        let mesh_count = document.meshes().count();

        GltfAsset { mesh_count }
    }
}


use std::num::NonZeroUsize;

use fontdue::Font;
use wgpu::{
    Device,
    Extent3d,
    ImageCopyTexture,
    ImageDataLayout,
    Origin3d,
    Queue,
    Texture,
    TextureAspect,
    TextureDescriptor,
    TextureDimension,
    TextureFormat,
    TextureUsages,
    TextureView,
    TextureViewDescriptor,
};
use guillotiere::{ size2, AtlasAllocator as BucketedAtlasAllocator };
use lru::LruCache;

pub struct InnerAtlas {
    pub texture: Texture,
    pub texture_view: TextureView,
    packer: BucketedAtlasAllocator,
    pub size: u32,
    glyph_cache: LruCache<CacheKey, GlyphDetails>,
}

impl InnerAtlas {
    const INITIAL_SIZE: u32 = 256;

    pub fn new(device: &Device) -> Self {
        let size = Self::INITIAL_SIZE;

        // Initialize the packer for allocating space in the atlas
        let packer = BucketedAtlasAllocator::new(size2(size as i32, size as i32));

        // Create the texture for the atlas
        let texture = device.create_texture(
            &(TextureDescriptor {
                label: Some("Glyph Texture"),
                size: Extent3d {
                    width: size,
                    height: size,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::R8Unorm, // Single channel texture
                usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                view_formats: &[],
            })
        );

        let texture_view = texture.create_view(&TextureViewDescriptor::default());

        Self {
            texture,
            texture_view,
            packer,
            size,
            glyph_cache: LruCache::new(NonZeroUsize::new(1000).unwrap()), // Adjust the cache size as needed
        }
    }

    fn upload_glyph_to_atlas(
        &mut self,
        queue: &Queue,
        glyph_data: &[u8],
        glyph_width: u32,
        glyph_height: u32
    ) -> Option<(u32, u32)> {
        let allocation = self.packer.allocate(size2(glyph_width as i32, glyph_height as i32))?;

        let bytes_per_pixel = 1; // Fontdue typically returns grayscale bitmaps (single channel)
        let bytes_per_row = glyph_width * bytes_per_pixel;

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.texture,
                mip_level: 0,
                origin: Origin3d {
                    x: allocation.rectangle.min.x as u32,
                    y: allocation.rectangle.min.y as u32,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            glyph_data,
            ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: None,
            },
            Extent3d {
                width: glyph_width,
                height: glyph_height,
                depth_or_array_layers: 1,
            }
        );

        Some((allocation.rectangle.min.x as u32, allocation.rectangle.min.y as u32))
    }

    pub fn get_or_create_glyph(
        &mut self,
        character: char,
        font_size: u32,
        font: &Font,
        queue: &Queue
    ) -> Option<GlyphDetails> {
        let key = CacheKey {
            character,
            font_size,
        };

        // Check if the glyph is already in the cache
        if let Some(details) = self.glyph_cache.get(&key) {
            return Some(details.clone());
        }

        // Rasterize the glyph using Fontdue
        let (metrics, bitmap) = font.rasterize(character, font_size as f32);

        if metrics.width == 0 || metrics.height == 0 {
            return None; // Handle empty glyphs (like spaces)
        }

        // Upload the glyph to the texture atlas
        if
            let Some((x, y)) = self.upload_glyph_to_atlas(
                queue,
                &bitmap,
                metrics.width as u32,
                metrics.height as u32
            )
        {
            // Store the glyph details in the cache
            let glyph_details = GlyphDetails {
                x,
                y,
                width: metrics.width as u32,
                height: metrics.height as u32,
            };

            self.glyph_cache.put(key, glyph_details.clone());

            return Some(glyph_details);
        }

        None
    }
}

#[derive(Hash, PartialEq, Eq, Clone)]
struct CacheKey {
    character: char,
    font_size: u32,
}

#[derive(Clone)]
pub struct GlyphDetails {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

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

pub struct InnerAtlas {
    pub texture: Texture,
    pub texture_view: TextureView,
    packer: BucketedAtlasAllocator,
    pub size: u32,
    glyph_cache: LruCache<CacheKey, GlyphDetails>,
    font: Font,
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

        let font = fontdue::Font
            ::from_bytes(
                include_bytes!("../Inter-Bold.ttf") as &[u8],
                fontdue::FontSettings::default()
            )
            .unwrap();

        Self {
            texture,
            texture_view,
            packer,
            size,
            glyph_cache: LruCache::new(NonZeroUsize::new(1000).unwrap()), // Adjust the cache size as needed
            font,
        }
    }

    pub fn get_or_create_glyph(
        &mut self,
        character: char,
        font_size: u32,
        queue: &Queue,
        device: &Device
    ) -> Option<GlyphDetails> {
        let key = CacheKey { character, font_size };

        // Check if the glyph is already in the cache
        if let Some(details) = self.glyph_cache.get(&key) {
            return Some(details.clone());
        }

        // Rasterize the glyph using Fontdue
        let (metrics, bitmap) = self.font.rasterize(character, font_size as f32);

        if metrics.width == 0 || metrics.height == 0 {
            return None; // Handle empty glyphs (like spaces)
        }

        // Attempt to upload the glyph to the atlas
        if
            let Some((x, y)) = self.upload_glyph_to_atlas(
                queue,
                &bitmap,
                metrics.width as u32,
                metrics.height as u32,
                device
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

    fn upload_glyph_to_atlas(
        &mut self,
        queue: &Queue,
        glyph_data: &[u8],
        glyph_width: u32,
        glyph_height: u32,
        device: &Device
    ) -> Option<(u32, u32)> {
        let allocation = self.packer.allocate(size2(glyph_width as i32, glyph_height as i32));

        // If the allocation fails, grow the atlas and try again
        if allocation.is_none() {
            self.grow(device, queue);
            return self.upload_glyph_to_atlas(queue, glyph_data, glyph_width, glyph_height, device);
        }

        let allocation = allocation.unwrap();

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
                aspect: TextureAspect::All,
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

    fn grow(&mut self, device: &Device, queue: &Queue) {
        // Double the size of the atlas
        let new_size = self.size * 2;

        // Create a new texture with the doubled size
        let new_texture = device.create_texture(
            &(TextureDescriptor {
                label: Some("Resized Glyph Texture"),
                size: Extent3d {
                    width: new_size,
                    height: new_size,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::R8Unorm,
                usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                view_formats: &[],
            })
        );

        let new_texture_view = new_texture.create_view(&TextureViewDescriptor::default());

        // Create a new packer with the new size
        let mut new_packer = BucketedAtlasAllocator::new(size2(new_size as i32, new_size as i32));

        // Collect all items from the cache into a vector to avoid borrowing issues
        let cache_items: Vec<(CacheKey, GlyphDetails)> = self.glyph_cache
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        // Re-rasterize and copy all existing glyphs from the old texture to the new texture
        for (key, details) in cache_items {
            // Re-rasterize the glyph
            let (metrics, bitmap) = self.font.rasterize(key.character, key.font_size as f32); // Replace with actual font

            let new_allocation = new_packer
                .allocate(size2(metrics.width as i32, metrics.height as i32))
                .expect("Unable to allocate space in new atlas");

            // Copy the newly rasterized bitmap data into the new texture
            queue.write_texture(
                ImageCopyTexture {
                    texture: &new_texture,
                    mip_level: 0,
                    origin: Origin3d {
                        x: new_allocation.rectangle.min.x as u32,
                        y: new_allocation.rectangle.min.y as u32,
                        z: 0,
                    },
                    aspect: TextureAspect::All,
                },
                &bitmap,
                ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(metrics.width as u32),
                    rows_per_image: None,
                },
                Extent3d {
                    width: metrics.width as u32,
                    height: metrics.height as u32,
                    depth_or_array_layers: 1,
                }
            );

            // Update the cache with the new coordinates
            let updated_details = GlyphDetails {
                x: new_allocation.rectangle.min.x as u32,
                y: new_allocation.rectangle.min.y as u32,
                width: metrics.width as u32,
                height: metrics.height as u32,
            };

            self.glyph_cache.put(key, updated_details);
        }

        // Update the atlas with the new texture, texture view, and packer
        self.texture = new_texture;
        self.texture_view = new_texture_view;
        self.packer = new_packer;
        self.size = new_size;
    }
}

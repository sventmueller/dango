//! Drawing a terminal frame with Metal: the quads `sprites` collects, put together in one draw call into the view's
//! `CAMetalLayer` and shown with the window's Core Animation transaction.

use std::cell::OnceCell;
use std::ptr::NonNull;
use std::rc::Rc;

use ide_terminal::Rgb;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_core_foundation::{CGRect, CGSize};
use objc2_core_graphics::{CGColorSpace, kCGColorSpaceSRGB};
use objc2_foundation::NSString;
use objc2_metal::{
    MTLBlendFactor, MTLClearColor, MTLCommandBuffer, MTLCommandEncoder, MTLCommandQueue, MTLCreateSystemDefaultDevice,
    MTLDevice, MTLDrawable, MTLLibrary, MTLLoadAction, MTLPixelFormat, MTLPrimitiveType, MTLRenderCommandEncoder,
    MTLRenderPassDescriptor, MTLRenderPipelineDescriptor, MTLRenderPipelineState, MTLResourceOptions, MTLStoreAction,
    MTLTexture,
};
use objc2_quartz_core::{CAMetalDrawable, CAMetalLayer, CATransaction};

use crate::render::Fonts;
use crate::sprites::{Quad, QuadCanvas, Sprites};

const SHADER: &str = include_str!("terminal.metal");

/// Set to anything, terminals draw with Core Graphics instead.
const CPU_ONLY: &str = "IDE_TERMINAL_CPU";

/// The Metal device, queue and pipeline all terminal views share.
pub struct Gpu {
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    pipeline: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
}

thread_local! {
    static SHARED: OnceCell<Option<Rc<Gpu>>> = const { OnceCell::new() };
}

/// The GPU the terminal views draw with, set up on first use; none without Metal, when the shader does not build, or
/// with `IDE_TERMINAL_CPU` set.
pub fn shared() -> Option<Rc<Gpu>> {
    SHARED.with(|cell| {
        cell.get_or_init(|| {
            if std::env::var_os(CPU_ONLY).is_some() {
                return None;
            }
            Gpu::new()
                .map_err(|error| ide_ui_common::diag::report("drawing terminals with Metal", error))
                .ok()
                .map(Rc::new)
        })
        .clone()
    })
}

impl Gpu {
    fn new() -> Result<Gpu, String> {
        let device = MTLCreateSystemDefaultDevice().ok_or("no Metal device")?;
        let queue = device.newCommandQueue().ok_or("no command queue")?;
        let library = device
            .newLibraryWithSource_options_error(&NSString::from_str(SHADER), None)
            .map_err(|error| error.localizedDescription().to_string())?;
        let function = |name: &str| {
            library
                .newFunctionWithName(&NSString::from_str(name))
                .ok_or_else(|| format!("the shader lacks {name}"))
        };
        let descriptor = MTLRenderPipelineDescriptor::new();
        descriptor.setVertexFunction(Some(&*function("quad_vertex")?));
        descriptor.setFragmentFunction(Some(&*function("quad_fragment")?));
        // SAFETY: Every pipeline descriptor has colour attachment 0.
        let attachment = unsafe { descriptor.colorAttachments().objectAtIndexedSubscript(0) };
        attachment.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
        attachment.setBlendingEnabled(true);
        attachment.setSourceRGBBlendFactor(MTLBlendFactor::One);
        attachment.setDestinationRGBBlendFactor(MTLBlendFactor::OneMinusSourceAlpha);
        attachment.setSourceAlphaBlendFactor(MTLBlendFactor::One);
        attachment.setDestinationAlphaBlendFactor(MTLBlendFactor::OneMinusSourceAlpha);
        let pipeline = device
            .newRenderPipelineStateWithDescriptor_error(&descriptor)
            .map_err(|error| error.localizedDescription().to_string())?;
        Ok(Gpu {
            device,
            queue,
            pipeline,
        })
    }

    /// A command buffer that draws `quads` over `atlas` into `target`, `size` pixels large, cleared to `clear`.
    fn encode(
        &self,
        target: &ProtocolObject<dyn MTLTexture>,
        size: CGSize,
        quads: &[Quad],
        atlas: &ProtocolObject<dyn MTLTexture>,
        clear: Rgb,
    ) -> Option<Retained<ProtocolObject<dyn MTLCommandBuffer>>> {
        let pass = MTLRenderPassDescriptor::new();
        // SAFETY: Every render pass descriptor has colour attachment 0.
        let attachment = unsafe { pass.colorAttachments().objectAtIndexedSubscript(0) };
        attachment.setTexture(Some(target));
        attachment.setLoadAction(MTLLoadAction::Clear);
        attachment.setStoreAction(MTLStoreAction::Store);
        let channel = |v: u8| f64::from(v) / 255.0;
        attachment.setClearColor(MTLClearColor {
            red: channel(clear.r),
            green: channel(clear.g),
            blue: channel(clear.b),
            alpha: 1.0,
        });
        let buffer = self.queue.commandBuffer()?;
        let encoder = buffer.renderCommandEncoderWithDescriptor(&pass)?;
        if !quads.is_empty() {
            let bytes = NonNull::from(quads).cast();
            // SAFETY: The pointer covers the quads' bytes, which Metal copies into the new buffer.
            let vertices = unsafe {
                self.device.newBufferWithBytes_length_options(
                    bytes,
                    size_of_val(quads),
                    MTLResourceOptions::StorageModeShared,
                )
            };
            #[expect(
                clippy::cast_possible_truncation,
                reason = "a drawable is far smaller than f32's range"
            )]
            let viewport = [size.width as f32, size.height as f32];
            encoder.setRenderPipelineState(&self.pipeline);
            // SAFETY: The vertex function reads the quads from buffer 0 and the viewport's two floats from index 1, the
            // fragment function the atlas from texture 0; each instance draws its quad as a strip of four corners.
            unsafe {
                encoder.setVertexBuffer_offset_atIndex(vertices.as_deref(), 0, 0);
                encoder.setVertexBytes_length_atIndex(NonNull::from(&viewport).cast(), size_of_val(&viewport), 1);
                encoder.setFragmentTexture_atIndex(Some(atlas), 0);
                encoder.drawPrimitives_vertexStart_vertexCount_instanceCount(
                    MTLPrimitiveType::TriangleStrip,
                    0,
                    4,
                    quads.len(),
                );
            }
        }
        encoder.endEncoding();
        Some(buffer)
    }
}

fn metal_layer(device: &ProtocolObject<dyn MTLDevice>) -> Retained<CAMetalLayer> {
    let layer = CAMetalLayer::new();
    layer.setDevice(Some(device));
    layer.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
    layer.setFramebufferOnly(true);
    // Frames go out with the window's own drawing, so the grid and the view keep step while the window resizes.
    layer.setPresentsWithTransaction(true);
    layer.setOpaque(true);
    // SAFETY: The name is one of Core Graphics' own colour spaces.
    let srgb = CGColorSpace::with_name(Some(unsafe { kCGColorSpaceSRGB }));
    layer.setColorspace(srgb.as_deref());
    layer
}

/// A view's drawing on the GPU: its sprites, and the frames it presents to the view's layer.
pub struct Surface {
    gpu: Rc<Gpu>,
    sprites: Sprites,
    layer: Retained<CAMetalLayer>,
}

impl Surface {
    /// A surface on the shared GPU; none where terminals draw with Core Graphics.
    pub fn new() -> Option<Surface> {
        let gpu = shared()?;
        let sprites = Sprites::new(&gpu.device)?;
        let layer = metal_layer(&gpu.device);
        Some(Surface { gpu, sprites, layer })
    }

    /// The layer `present` draws into, for the view to hold.
    pub fn layer(&self) -> &CAMetalLayer {
        &self.layer
    }

    /// The quads `paint` makes over sprites drawn with `fonts`; a frame whose sprites do not fit is painted again into
    /// a larger atlas.
    fn quads(&mut self, fonts: &Fonts, paint: &dyn Fn(&mut QuadCanvas)) -> Vec<Quad> {
        loop {
            self.sprites.prepare(fonts);
            let mut canvas = QuadCanvas::new(&mut self.sprites, fonts);
            paint(&mut canvas);
            let quads = canvas.quads;
            if !self.sprites.full() || !self.sprites.grow() {
                return quads;
            }
        }
    }

    /// Draws the frame `paint` makes into the layer, laid over `frame` of the view, and presents it with the current
    /// transaction.
    pub fn present(&mut self, fonts: &Fonts, frame: CGRect, clear: Rgb, paint: &dyn Fn(&mut QuadCanvas)) {
        let scale = fonts.metrics.scale;
        let pixels = CGSize::new((frame.size.width * scale).round(), (frame.size.height * scale).round());
        if pixels.width < 1.0 || pixels.height < 1.0 {
            return;
        }
        let layer = self.layer.clone();
        CATransaction::begin();
        CATransaction::setDisableActions(true);
        layer.setFrame(frame);
        CATransaction::commit();
        layer.setContentsScale(scale);
        if layer.drawableSize() != pixels {
            layer.setDrawableSize(pixels);
        }
        let quads = self.quads(fonts, paint);
        let Some(drawable) = layer.nextDrawable() else {
            return;
        };
        let target = drawable.texture();
        let Some(buffer) = self.gpu.encode(&target, pixels, &quads, self.sprites.texture(), clear) else {
            return;
        };
        buffer.commit();
        // With `presentsWithTransaction`, the drawable may only be presented once its commands are scheduled.
        buffer.waitUntilScheduled();
        drawable.present();
    }

    /// The BGRA pixels of the frame `paint` makes, `width` × `height` pixels large; none on a GPU whose memory the
    /// processor cannot read directly.
    #[cfg(test)]
    pub fn picture(
        &mut self,
        fonts: &Fonts,
        (width, height): (usize, usize),
        clear: Rgb,
        paint: &dyn Fn(&mut QuadCanvas),
    ) -> Option<Vec<u8>> {
        use objc2_metal::{MTLOrigin, MTLRegion, MTLSize, MTLStorageMode, MTLTextureDescriptor, MTLTextureUsage};
        if !self.gpu.device.hasUnifiedMemory() {
            return None;
        }
        // SAFETY: A plain two-dimensional texture without mipmaps.
        let descriptor = unsafe {
            MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(
                MTLPixelFormat::BGRA8Unorm,
                width,
                height,
                false,
            )
        };
        descriptor.setUsage(MTLTextureUsage::RenderTarget);
        descriptor.setStorageMode(MTLStorageMode::Shared);
        let target = self.gpu.device.newTextureWithDescriptor(&descriptor)?;
        let quads = self.quads(fonts, paint);
        let size = CGSize::new(width as f64, height as f64);
        let buffer = self.gpu.encode(&target, size, &quads, self.sprites.texture(), clear)?;
        buffer.commit();
        buffer.waitUntilCompleted();
        let mut pixels = vec![0u8; width * height * 4];
        let region = MTLRegion {
            origin: MTLOrigin { x: 0, y: 0, z: 0 },
            size: MTLSize {
                width,
                height,
                depth: 1,
            },
        };
        // SAFETY: The buffer holds the region's rows of 4 bytes per pixel, and the GPU has finished with the texture.
        unsafe {
            target.getBytes_bytesPerRow_fromRegion_mipmapLevel(
                NonNull::from(pixels.as_mut_slice()).cast(),
                width * 4,
                region,
                0,
            );
        }
        Some(pixels)
    }
}

#[cfg(test)]
mod tests {
    use ide_terminal::{Cell, CellFlags, Cursor, CursorShape, Damage, Highlight, InputModes, Point, Snapshot};

    use super::*;
    use crate::palette;
    use crate::render::{self, Overlay, rect};
    use crate::sprites;

    const LINES: [&str; 6] = [
        "plain text, then BOLD and italic and both",
        "underlined double curly dotted dashed struck",
        "漢字 and 😀 and e\u{301} beside ╭──┬──╮ ║ ░▒▓█",
        "red green blue on yellow, selected words",
        "$ cargo build --release   ",
        "",
    ];

    fn cell(ch: char, fg: Rgb, bg: Rgb, flags: CellFlags) -> Cell {
        Cell {
            ch,
            zero_width: vec![],
            fg,
            bg,
            underline_color: None,
            flags,
            hyperlink: None,
        }
    }

    /// A screen that holds each kind of thing the view draws.
    fn screen(fg: Rgb, bg: Rgb, columns: usize) -> Snapshot {
        let styles = [
            CellFlags::UNDERLINE,
            CellFlags::DOUBLE_UNDERLINE,
            CellFlags::UNDERCURL,
            CellFlags::DOTTED_UNDERLINE,
            CellFlags::DASHED_UNDERLINE,
            CellFlags::STRIKETHROUGH,
        ];
        let colours = [
            Rgb::new(240, 113, 107),
            Rgb::new(139, 203, 110),
            Rgb::new(106, 167, 245),
        ];
        let rows = LINES
            .iter()
            .enumerate()
            .map(|(line, text)| {
                let mut row: Vec<Cell> = Vec::new();
                for (word, part) in text.split(' ').enumerate() {
                    let flags = match line {
                        0 if word >= 3 => [CellFlags::BOLD, CellFlags::ITALIC][word % 2],
                        1 => styles[word % styles.len()],
                        _ => CellFlags::default(),
                    };
                    let (cfg, cbg) = match (line, word) {
                        (3, w) if w < 3 => (colours[w], bg),
                        (3, 4) => (bg, Rgb::new(227, 194, 106)),
                        _ => (fg, bg),
                    };
                    for ch in part.chars() {
                        if ch == '\u{301}' {
                            if let Some(last) = row.last_mut() {
                                last.zero_width.push(ch);
                            }
                            continue;
                        }
                        let wide = matches!(ch, '漢' | '字' | '😀');
                        let flags = if wide { CellFlags::WIDE } else { flags };
                        row.push(cell(ch, cfg, cbg, flags));
                        if wide {
                            row.push(cell(' ', cfg, cbg, CellFlags::WIDE_SPACER));
                        }
                    }
                    row.push(cell(' ', fg, bg, CellFlags::default()));
                }
                row.resize(columns, cell(' ', fg, bg, CellFlags::default()));
                row.truncate(columns);
                row
            })
            .collect();
        Snapshot {
            columns,
            lines: LINES.len(),
            rows,
            cursor: Cursor {
                point: Point { line: 4, column: 24 },
                shape: CursorShape::Block,
                visible: true,
                blinking: false,
                wide: false,
            },
            selection: Some(Highlight {
                start: Point { line: 3, column: 30 },
                end: Point { line: 3, column: 43 },
                block: false,
            }),
            display_offset: 0,
            history: 0,
            damage: Damage::Full,
            background: bg,
            modes: InputModes::default(),
        }
    }

    fn save(path: &std::path::Path, bgra: &[u8], width: usize, height: usize) {
        let rgba: Vec<u8> = bgra
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|p| [p[2], p[1], p[0], p[3]])
            .collect();
        let (w, h) = (u32::try_from(width).unwrap(), u32::try_from(height).unwrap());
        let _ = image::save_buffer(path, &rgba, w, h, image::ExtendedColorType::Rgba8);
    }

    #[test]
    fn a_frame_takes_the_layer_to_its_size_in_pixels_and_one_without_pixels_is_left_out() {
        let Some(mut surface) = Surface::new() else {
            return;
        };
        let fonts = Fonts::new(13.0, 2.0);
        let theme = palette::theme(false, false);
        let snapshot = screen(theme.palette.foreground, theme.palette.background, 20);
        let overlay = Overlay {
            focused: false,
            cursor_on: true,
            marked: "",
            final_line: None,
            flash: false,
        };
        let present = |surface: &mut Surface, bounds: CGRect| {
            let paint = |canvas: &mut QuadCanvas| render::paint(canvas, &fonts, &theme, &snapshot, &overlay, bounds);
            surface.present(&fonts, bounds, theme.palette.background, &paint);
        };
        let before = surface.layer().drawableSize();
        present(&mut surface, rect(0.0, 0.0, 0.2, 0.2));
        assert_eq!(
            surface.layer().drawableSize(),
            before,
            "less than a pixel draws nothing"
        );
        let bounds = rect(0.0, 0.0, 300.0, 120.0);
        present(&mut surface, bounds);
        assert_eq!(surface.layer().drawableSize(), CGSize::new(600.0, 240.0));
        assert_eq!(surface.layer().frame(), bounds);
        assert!((surface.layer().contentsScale() - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn the_gpu_draws_what_core_graphics_draws() {
        let Some(mut surface) = Surface::new() else {
            return;
        };
        for dark in [true, false] {
            let fonts = Fonts::new(13.0, 2.0);
            let m = fonts.metrics;
            let theme = palette::theme(dark, false);
            let snapshot = screen(theme.palette.foreground, theme.palette.background, 48);
            let overlay = Overlay {
                focused: true,
                cursor_on: true,
                marked: "",
                final_line: None,
                flash: false,
            };
            let (width, height) = (
                2.0 * render::INSET_X + 48.0 * m.cell_width,
                2.0 * render::INSET_Y + LINES.len() as f64 * m.cell_height,
            );
            let bounds = rect(0.0, 0.0, width, height);
            let pixels = (
                render::clamp_to(width * 2.0, 4096),
                render::clamp_to(height * 2.0, 4096),
            );
            let paint = |canvas: &mut QuadCanvas| render::paint(canvas, &fonts, &theme, &snapshot, &overlay, bounds);
            let Some(gpu) = surface.picture(&fonts, pixels, theme.palette.background, &paint) else {
                return;
            };
            let cpu = sprites::bitmap(pixels.0, pixels.1, 2.0, (0.0, 0.0), &|ctx| {
                render::draw(ctx, &fonts, &theme, &snapshot, &overlay, bounds);
            })
            .unwrap();
            let differing = |limit: u8| {
                cpu.as_chunks::<4>()
                    .0
                    .iter()
                    .zip(gpu.as_chunks::<4>().0)
                    .filter(|(a, b)| (0..3).any(|i| a[i].abs_diff(b[i]) > limit))
                    .count()
            };
            let total = pixels.0 * pixels.1;
            let (some, clear) = (differing(48), differing(128));
            let target = std::env::temp_dir().join("ide-ui-terminal-gpu-parity");
            let name = if dark { "dark" } else { "light" };
            let fits = some * 2000 <= total && clear == 0;
            if std::env::var_os("IDE_GPU_PICTURES").is_some() || !fits {
                let _ = std::fs::create_dir_all(&target);
                save(
                    &target.join(format!("gpu-parity-{name}-cpu.png")),
                    &cpu,
                    pixels.0,
                    pixels.1,
                );
                save(
                    &target.join(format!("gpu-parity-{name}-gpu.png")),
                    &gpu,
                    pixels.0,
                    pixels.1,
                );
            }
            assert!(
                fits,
                "{name}: of {total} pixels, {some} differ by more than 48 and {clear} by more than 128; the pictures are \
                 in {}/gpu-parity-{name}-*.png",
                target.display()
            );
        }
    }
}

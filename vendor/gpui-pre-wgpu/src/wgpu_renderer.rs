use crate::{CompositorGpuHint, DeviceErrorState, WgpuAtlas, WgpuContext};
use anyhow::{Context as _, Result};
use bytemuck::{Pod, Zeroable};
use collections::FxHashMap;
use gpui::{
    AtlasTextureId, Background, BorderStyle, Bounds, ContentMask, Corners, DevicePixels, Edges,
    GpuSpecs, MonochromeSprite, Path, Point, PolychromeSprite, PrimitiveBatch, Quad, ScaledPixels,
    Scene, Shadow, Size, SubpixelSprite, TransformationMatrix, Underline,
    get_gamma_correction_ratios,
};
use log::warn;
#[cfg(not(target_family = "wasm"))]
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use smallvec::SmallVec;
use std::cell::RefCell;
use std::num::NonZeroU64;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

const MAX_INSTANCE_BUFFER_SIZE: u64 = 256 * 1024 * 1024;

const INSTANCE_TEXTURE_TEXEL_SIZE: u64 = 16;

/// Shader variant for backends with storage buffer support: the shared shader
/// logic plus the storage-buffer instance transport.
const STORAGE_BUFFER_SHADERS: &str = concat!(
    include_str!("shaders.wgsl"),
    include_str!("shaders_storage.wgsl"),
);

/// Shader variant for WebGL2, which has no storage buffers: the shared shader
/// logic plus the texture-based instance transport.
const WEBGL_SHADERS: &str = concat!(
    include_str!("shaders.wgsl"),
    include_str!("shaders_webgl.wgsl"),
);

/// Subpixel text rendering requires dual-source blending, which WebGL2 lacks, so
/// this variant only ever runs with the storage-buffer transport. The `enable`
/// directive must precede all declarations.
const SUBPIXEL_SHADERS: &str = concat!(
    "enable dual_source_blending;\n",
    include_str!("shaders.wgsl"),
    include_str!("shaders_storage.wgsl"),
    include_str!("shaders_subpixel.wgsl"),
);

fn least_common_multiple(left: u64, right: u64) -> u64 {
    let mut first = left;
    let mut second = right;
    while second != 0 {
        let remainder = first % second;
        first = second;
        second = remainder;
    }
    left / first * right
}

/// Corvene patch: the damage scissor's helper shaders (`shaders_damage.wgsl`).
const DAMAGE_SHADERS: &str = include_str!("shaders_damage.wgsl");

/// Corvene patch: see [`set_opaque_depth_pass`].
static OPAQUE_DEPTH_PASS: AtomicBool = AtomicBool::new(false);

/// Corvene patch: see [`set_damage_scissor`].
static DAMAGE_SCISSOR: AtomicBool = AtomicBool::new(false);

/// Corvene patch: turns the opaque depth pass on or off, for every window
/// from its next frame on (off by default).
///
/// GPUI paints back to front with blending and no depth test, so every
/// panel, row and diff line is shaded over the one before it: on a phone's
/// tile-based GPU that overdraw is most of a frame's cost. With this on,
/// each primitive gets a depth from its place in the paint order (later is
/// nearer). The quads that are opaque over their interior (a solid colour of
/// alpha 1, inside their borders, rounded corners and content mask, snapped
/// to whole pixels) are drawn first, front to back, writing depth with
/// blending off; then everything is drawn in the usual order with a depth
/// test and no depth writes, so what a later opaque quad hides is rejected
/// before it is shaded (and so is each opaque quad's own interior, which the
/// first pass already drew). The image stays the same.
pub fn set_opaque_depth_pass(enabled: bool) {
    OPAQUE_DEPTH_PASS.store(enabled, Ordering::Relaxed);
}

/// Corvene patch: whether the opaque depth pass is on, see
/// [`set_opaque_depth_pass`].
pub fn opaque_depth_pass() -> bool {
    OPAQUE_DEPTH_PASS.load(Ordering::Relaxed)
}

/// Corvene patch: turns the damage scissor on or off, for every window from
/// its next frame on (off by default).
///
/// With this on, a window draws into a texture that keeps the last frame and
/// then copies it to the swapchain image (which does not keep its content).
/// The new scene is compared with the last one, primitive by primitive; when
/// only a small part changed (a caret, a hovered row, a badge), only the
/// rectangle around the changed primitives is cleared and redrawn, with a
/// scissor. A frame that changed in more than three quarters of its area,
/// or whose size, colours or sprite atlas changed, is drawn whole, as is the
/// first. The extra copy of the whole frame costs a frame's worth of memory
/// bandwidth, far less than shading it on a phone.
pub fn set_damage_scissor(enabled: bool) {
    DAMAGE_SCISSOR.store(enabled, Ordering::Relaxed);
}

/// Corvene patch: whether the damage scissor is on, see
/// [`set_damage_scissor`].
pub fn damage_scissor() -> bool {
    DAMAGE_SCISSOR.load(Ordering::Relaxed)
}

/// Corvene patch: the switches as one frame reads them, so that a frame
/// sees one value of each (and tests can set them without the globals).
#[derive(Clone, Copy, Debug, Default)]
struct FrameOptions {
    opaque_depth_pass: bool,
    damage_scissor: bool,
}

impl FrameOptions {
    fn current() -> Self {
        Self {
            opaque_depth_pass: opaque_depth_pass(),
            damage_scissor: damage_scissor(),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GlobalParams {
    viewport_size: [f32; 2],
    premultiplied_alpha: u32,
    /// Corvene patch: see `with_order_depth` in `shaders.wgsl` (was padding)
    order_is_depth: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PodBounds {
    origin: [f32; 2],
    size: [f32; 2],
}

impl From<Bounds<ScaledPixels>> for PodBounds {
    fn from(bounds: Bounds<ScaledPixels>) -> Self {
        Self {
            origin: [bounds.origin.x.0, bounds.origin.y.0],
            size: [bounds.size.width.0, bounds.size.height.0],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SurfaceParams {
    bounds: PodBounds,
    content_mask: PodBounds,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GammaParams {
    gamma_ratios: [f32; 4],
    grayscale_enhanced_contrast: f32,
    subpixel_enhanced_contrast: f32,
    is_bgr: u32,
    _pad: u32,
}

#[derive(Clone, Debug)]
#[repr(C)]
struct PathSprite {
    bounds: Bounds<ScaledPixels>,
}

#[derive(Clone, Debug)]
#[repr(C)]
struct PathRasterizationVertex {
    xy_position: Point<ScaledPixels>,
    st_position: Point<f32>,
    color: Background,
    bounds: Bounds<ScaledPixels>,
}

pub struct WgpuSurfaceConfig {
    pub size: Size<DevicePixels>,
    pub transparent: bool,
    /// Preferred presentation mode. When `Some`, the renderer will use this
    /// mode if supported by the surface, falling back to `Fifo`.
    /// When `None`, defaults to `Fifo` (VSync).
    ///
    /// Mobile platforms may prefer `Mailbox` (triple-buffering) to avoid
    /// blocking in `get_current_texture()` during lifecycle transitions.
    pub preferred_present_mode: Option<wgpu::PresentMode>,
}

struct WgpuPipelines {
    quads: wgpu::RenderPipeline,
    shadows: wgpu::RenderPipeline,
    path_rasterization: wgpu::RenderPipeline,
    paths: wgpu::RenderPipeline,
    underlines: wgpu::RenderPipeline,
    mono_sprites: wgpu::RenderPipeline,
    subpixel_sprites: Option<wgpu::RenderPipeline>,
    poly_sprites: wgpu::RenderPipeline,
    #[allow(dead_code)]
    surfaces: wgpu::RenderPipeline,
    /// Corvene patch: the opaque quads, front to back, writing depth with
    /// blending off; only in the set built with a depth format (see
    /// `set_opaque_depth_pass`)
    opaque_quads: Option<wgpu::RenderPipeline>,
}

/// One frame allocation of instance data, ready to bind.
struct InstanceBinding {
    bind_group: wgpu::BindGroup,
    /// Index of the allocation's first instance within the bound data. Always
    /// zero on the storage-buffer path, where the binding offset already
    /// positions the array; on the WebGL texture path the shader indexes the
    /// shared instance texture absolutely, so draws must offset their
    /// instance (or vertex) ranges by this value.
    first_instance: u32,
}

struct InstanceBindings {
    quads: InstanceBinding,
    shadows: InstanceBinding,
    underlines: InstanceBinding,
    monochrome_sprites: InstanceBinding,
    subpixel_sprites: InstanceBinding,
    polychrome_sprites: InstanceBinding,
}

struct WgpuBindGroupLayouts {
    globals: wgpu::BindGroupLayout,
    instances: wgpu::BindGroupLayout,
    texture: wgpu::BindGroupLayout,
    surfaces: wgpu::BindGroupLayout,
}

/// Shared GPU context reference, used to coordinate device recovery across multiple windows.
pub type GpuContext = Rc<RefCell<Option<WgpuContext>>>;

enum InstanceData {
    Storage(wgpu::Buffer),
    // WebGL2 has no storage buffers. A uint texture keeps the records available to both shader
    // stages while preserving integer and floating-point bit patterns exactly.
    Texture {
        texture: wgpu::Texture,
        view: wgpu::TextureView,
        width: u32,
        height: u32,
    },
}

/// GPU resources that must be dropped together during device recovery.
struct WgpuResources {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    pipelines: WgpuPipelines,
    bind_group_layouts: WgpuBindGroupLayouts,
    atlas_sampler: wgpu::Sampler,
    atlas_texture_bind_groups: FxHashMap<AtlasTextureId, CachedTextureBindGroup>,
    globals_buffer: wgpu::Buffer,
    globals_bind_group: wgpu::BindGroup,
    path_globals_bind_group: wgpu::BindGroup,
    instance_data: InstanceData,
    path_intermediate_texture: Option<wgpu::Texture>,
    path_intermediate_view: Option<wgpu::TextureView>,
    path_msaa_texture: Option<wgpu::Texture>,
    path_msaa_view: Option<wgpu::TextureView>,
    /// Corvene patch: `pipelines` with a depth test, built when the opaque
    /// depth pass is first used (see `set_opaque_depth_pass`)
    depth_pipelines: Option<WgpuPipelines>,
    /// Corvene patch: the opaque depth pass's depth buffer, the size of the
    /// frame, rebuilt like the path textures
    depth_texture: Option<wgpu::Texture>,
    depth_view: Option<wgpu::TextureView>,
    /// Corvene patch: see `set_damage_scissor`
    damage_pipelines: Option<DamagePipelines>,
    damage: Option<DamageState>,
}

struct CachedTextureBindGroup {
    texture_generation: u64,
    bind_group: wgpu::BindGroup,
}

impl WgpuResources {
    fn invalidate_intermediate_textures(&mut self) {
        self.path_intermediate_texture = None;
        self.path_intermediate_view = None;
        self.path_msaa_texture = None;
        self.path_msaa_view = None;
        // Corvene patch: these are the size of the frame too
        self.depth_texture = None;
        self.depth_view = None;
        self.damage = None;
    }
}

struct WgpuRendererCore {
    resources: WgpuResources,
    atlas: Arc<WgpuAtlas>,
    path_globals_offset: u64,
    gamma_offset: u64,
    instance_data_capacity: u64,
    max_instance_data_size: u64,
    instance_data_alignment: u64,
    uses_webgl_instance_data: bool,
    rendering_params: RenderingParameters,
    is_bgr: bool,
    dual_source_blending: bool,
    adapter_info: wgpu::AdapterInfo,
    target_format: wgpu::TextureFormat,
    max_texture_size: u32,
    /// Corvene patch: the format of the opaque depth pass's depth buffer
    depth_format: wgpu::TextureFormat,
    /// Corvene patch: what `resources.pipelines` blend for, to build the
    /// opaque depth pass's set alike
    alpha_mode: wgpu::CompositeAlphaMode,
    /// Corvene patch: the opaque depth pass's per-frame data, kept to reuse
    /// its memory
    depth_scratch: DepthScratch,
    depth_bytes: Vec<u8>,
}

/// GPU resources of a windowed renderer. A surface is only ever configured against the
/// device that owns `core`, so it cannot outlive it: there is no surface-only state.
enum RendererState {
    /// Frames can be drawn.
    Ready {
        surface: wgpu::Surface<'static>,
        core: WgpuRendererCore,
    },
    /// The native surface is gone (Android `TerminateWindow`, browser context loss) but
    /// the device, pipelines, and atlas remain so `replace_surface` can resume without
    /// re-uploading cached textures.
    Unconfigured { core: WgpuRendererCore },
    /// Released by `destroy`, or dropped for a device recovery that has not succeeded yet.
    Released,
}

pub struct WgpuRenderer {
    /// Shared GPU context for device recovery coordination (unused on WASM).
    #[allow(dead_code)]
    context: Option<GpuContext>,
    /// Compositor GPU hint for adapter selection (unused on WASM).
    #[allow(dead_code)]
    compositor_gpu: Option<CompositorGpuHint>,
    state: RendererState,
    surface_config: wgpu::SurfaceConfiguration,
    atlas: Arc<WgpuAtlas>,
    transparent_alpha_mode: wgpu::CompositeAlphaMode,
    opaque_alpha_mode: wgpu::CompositeAlphaMode,
    max_texture_size: u32,
    is_bgr: bool,
    failed_frame_count: u32,
    device_errors: Arc<DeviceErrorState>,
    observed_error_generation: u64,
    last_surface_error: Option<String>,
    needs_redraw: bool,
    /// Corvene patch: whether the surface's images can be copied to, for
    /// the damage scissor (see `set_damage_scissor`)
    surface_copy_dst: bool,
    /// Corvene patch: the retained frame could not be shown on this surface
    /// (see `set_damage_scissor`); frames are drawn directly from then on.
    damage_unusable: bool,
}

impl WgpuRenderer {
    fn core(&self) -> Option<&WgpuRendererCore> {
        match &self.state {
            RendererState::Ready { core, .. } | RendererState::Unconfigured { core } => Some(core),
            RendererState::Released => None,
        }
    }

    fn core_mut(&mut self) -> Option<&mut WgpuRendererCore> {
        match &mut self.state {
            RendererState::Ready { core, .. } | RendererState::Unconfigured { core } => Some(core),
            RendererState::Released => None,
        }
    }

    /// Creates a new WgpuRenderer from raw window handles.
    ///
    /// The `gpu_context` is a shared reference that coordinates GPU context across
    /// multiple windows. The first window to create a renderer will initialize the
    /// context; subsequent windows will share it.
    ///
    /// # Safety
    /// The caller must ensure that the window handle remains valid for the lifetime
    /// of the returned renderer.
    #[cfg(not(target_family = "wasm"))]
    pub fn new<W>(
        gpu_context: GpuContext,
        window: &W,
        config: WgpuSurfaceConfig,
        compositor_gpu: Option<CompositorGpuHint>,
    ) -> anyhow::Result<Self>
    where
        W: HasWindowHandle + HasDisplayHandle + std::fmt::Debug + Send + Sync + Clone + 'static,
    {
        let window_handle = window
            .window_handle()
            .map_err(|e| anyhow::anyhow!("Failed to get window handle: {e}"))?;

        let target = wgpu::SurfaceTargetUnsafe::RawHandle {
            // Fall back to the display handle already provided via InstanceDescriptor::display.
            raw_display_handle: None,
            raw_window_handle: window_handle.as_raw(),
        };

        // Use the existing context's instance if available, otherwise create a new one.
        // The surface must be created with the same instance that will be used for
        // adapter selection, otherwise wgpu will panic.
        let instance = gpu_context
            .borrow()
            .as_ref()
            .map(|ctx| ctx.instance.clone())
            .unwrap_or_else(|| WgpuContext::instance(Some(Box::new(window.clone()))));

        // Safety: The caller guarantees that the window handle is valid for the
        // lifetime of this renderer. In practice, the RawWindow struct is created
        // from the native window handles and the surface is dropped before the window.
        let surface = unsafe {
            instance
                .create_surface_unsafe(target)
                .map_err(|e| anyhow::anyhow!("Failed to create surface: {e}"))?
        };

        let mut ctx_ref = gpu_context.borrow_mut();
        let context = match ctx_ref.as_mut() {
            Some(context) => {
                context.check_compatible_with_surface(&surface)?;
                context
            }
            None => ctx_ref.insert(WgpuContext::new(instance, &surface, compositor_gpu)?),
        };

        let atlas = Arc::new(WgpuAtlas::from_context(context));

        Self::new_internal(
            Some(Rc::clone(&gpu_context)),
            context,
            surface,
            config,
            compositor_gpu,
            atlas,
        )
    }

    #[cfg(target_family = "wasm")]
    pub fn new_from_canvas(
        context: &WgpuContext,
        canvas: &web_sys::HtmlCanvasElement,
        config: WgpuSurfaceConfig,
    ) -> anyhow::Result<Self> {
        let surface = context
            .instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
            .map_err(|e| anyhow::anyhow!("Failed to create surface: {e}"))?;
        Self::new_from_surface(context, surface, config)
    }

    #[cfg(target_family = "wasm")]
    #[allow(clippy::arc_with_non_send_sync)]
    pub fn new_from_surface(
        context: &WgpuContext,
        surface: wgpu::Surface<'static>,
        config: WgpuSurfaceConfig,
    ) -> anyhow::Result<Self> {
        let atlas = Arc::new(WgpuAtlas::from_context(context));
        Self::new_internal(None, context, surface, config, None, atlas)
    }

    fn new_internal(
        gpu_context: Option<GpuContext>,
        context: &WgpuContext,
        surface: wgpu::Surface<'static>,
        config: WgpuSurfaceConfig,
        compositor_gpu: Option<CompositorGpuHint>,
        atlas: Arc<WgpuAtlas>,
    ) -> anyhow::Result<Self> {
        let surface_caps = surface.get_capabilities(&context.adapter);
        let preferred_formats = [
            wgpu::TextureFormat::Bgra8Unorm,
            wgpu::TextureFormat::Rgba8Unorm,
        ];
        let surface_format = preferred_formats
            .iter()
            .find(|f| surface_caps.formats.contains(f))
            .copied()
            .or_else(|| surface_caps.formats.iter().find(|f| !f.is_srgb()).copied())
            .or_else(|| surface_caps.formats.first().copied())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Surface reports no supported texture formats for adapter {:?}",
                    context.adapter.get_info().name
                )
            })?;

        let pick_alpha_mode =
            |preferences: &[wgpu::CompositeAlphaMode]| -> anyhow::Result<wgpu::CompositeAlphaMode> {
                preferences
                    .iter()
                    .find(|p| surface_caps.alpha_modes.contains(p))
                    .copied()
                    .or_else(|| surface_caps.alpha_modes.first().copied())
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "Surface reports no supported alpha modes for adapter {:?}",
                            context.adapter.get_info().name
                        )
                    })
            };

        let transparent_alpha_mode = pick_alpha_mode(&[
            wgpu::CompositeAlphaMode::PreMultiplied,
            wgpu::CompositeAlphaMode::Inherit,
        ])?;

        let opaque_alpha_mode = pick_alpha_mode(&[
            wgpu::CompositeAlphaMode::Opaque,
            wgpu::CompositeAlphaMode::Inherit,
        ])?;

        let alpha_mode = if config.transparent {
            transparent_alpha_mode
        } else {
            opaque_alpha_mode
        };

        let device = Arc::clone(&context.device);
        let max_texture_size = device.limits().max_texture_dimension_2d;

        let requested_width = config.size.width.0 as u32;
        let requested_height = config.size.height.0 as u32;
        let clamped_width = requested_width.min(max_texture_size);
        let clamped_height = requested_height.min(max_texture_size);

        if clamped_width != requested_width || clamped_height != requested_height {
            warn!(
                "Requested surface size ({}, {}) exceeds maximum texture dimension {}. \
                 Clamping to ({}, {}). Window content may not fill the entire window.",
                requested_width, requested_height, max_texture_size, clamped_width, clamped_height
            );
        }

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: clamped_width.max(1),
            height: clamped_height.max(1),
            present_mode: config
                .preferred_present_mode
                .filter(|mode| surface_caps.present_modes.contains(mode))
                .unwrap_or(wgpu::PresentMode::Fifo),
            desired_maximum_frame_latency: 2,
            alpha_mode,
            view_formats: vec![],
        };
        // Configure the surface immediately. The adapter selection process already validated
        // that this adapter can successfully configure this surface.
        surface.configure(&context.device, &surface_config);

        let core = WgpuRendererCore::new(context, atlas.clone(), surface_format, alpha_mode);

        Ok(Self {
            context: gpu_context,
            compositor_gpu,
            state: RendererState::Ready { surface, core },
            surface_config,
            atlas,
            transparent_alpha_mode,
            opaque_alpha_mode,
            max_texture_size,
            is_bgr: false,
            failed_frame_count: 0,
            device_errors: Arc::clone(context.errors()),
            observed_error_generation: 0,
            last_surface_error: None,
            needs_redraw: false,
            surface_copy_dst: surface_caps.usages.contains(wgpu::TextureUsages::COPY_DST),
            damage_unusable: false,
        })
    }
}

impl WgpuRendererCore {
    fn create_bind_group_layouts(
        device: &wgpu::Device,
        uses_webgl_instance_data: bool,
    ) -> WgpuBindGroupLayouts {
        let globals =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("globals_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: NonZeroU64::new(
                                std::mem::size_of::<GlobalParams>() as u64
                            ),
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: NonZeroU64::new(
                                std::mem::size_of::<GammaParams>() as u64
                            ),
                        },
                        count: None,
                    },
                ],
            });

        let instance_data_entry = wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: if uses_webgl_instance_data {
                wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Uint,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                }
            } else {
                wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                }
            },
            count: None,
        };

        let instances = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("instances_layout"),
            entries: &[instance_data_entry],
        });

        let texture = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("texture_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let surfaces = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("surfaces_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: NonZeroU64::new(
                            std::mem::size_of::<SurfaceParams>() as u64
                        ),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        WgpuBindGroupLayouts {
            globals,
            instances,
            texture,
            surfaces,
        }
    }

    fn create_instance_texture(
        device: &wgpu::Device,
        requested_capacity: u64,
        max_texture_dimension: u32,
    ) -> (InstanceData, u64) {
        let texel_count = requested_capacity.div_ceil(INSTANCE_TEXTURE_TEXEL_SIZE);
        let width = texel_count.min(u64::from(max_texture_dimension)).max(1) as u32;
        let height = texel_count
            .div_ceil(u64::from(width))
            .min(u64::from(max_texture_dimension))
            .max(1) as u32;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("instance_texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Uint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let capacity = u64::from(width) * u64::from(height) * INSTANCE_TEXTURE_TEXEL_SIZE;
        (
            InstanceData::Texture {
                texture,
                view,
                width,
                height,
            },
            capacity,
        )
    }

    fn create_pipelines(
        device: &wgpu::Device,
        layouts: &WgpuBindGroupLayouts,
        surface_format: wgpu::TextureFormat,
        alpha_mode: wgpu::CompositeAlphaMode,
        path_sample_count: u32,
        dual_source_blending: bool,
        uses_webgl_instance_data: bool,
        // Corvene patch: `Some` builds the set for the opaque depth pass (see
        // `set_opaque_depth_pass`): every pipeline that draws to the frame
        // tests depth (without writing it), and `opaque_quads` exists
        depth_format: Option<wgpu::TextureFormat>,
    ) -> WgpuPipelines {
        // Diagnostic guard: verify the device actually has
        // DUAL_SOURCE_BLENDING. We have a crash report (ZED-5G1) where a
        // feature mismatch caused a wgpu-hal abort, but we haven't
        // identified the code path that produces the mismatch. This
        // guard prevents the crash and logs more evidence.
        // Remove this check once:
        // a) We find and fix the root cause, or
        // b) There are no reports of this warning appearing for some time.
        let device_has_feature = device
            .features()
            .contains(wgpu::Features::DUAL_SOURCE_BLENDING);
        if dual_source_blending && !device_has_feature {
            log::error!(
                "BUG: dual_source_blending flag is true but device does not \
                 have DUAL_SOURCE_BLENDING enabled (device features: {:?}). \
                 Falling back to mono text rendering. Please report this at \
                 https://github.com/zed-industries/zed/issues",
                device.features(),
            );
        }
        let dual_source_blending =
            dual_source_blending && device_has_feature && !uses_webgl_instance_data;

        let shader_source = if uses_webgl_instance_data {
            WEBGL_SHADERS
        } else {
            STORAGE_BUFFER_SHADERS
        };
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("gpui_shaders"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        let subpixel_shader_module = if dual_source_blending {
            Some(device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("gpui_subpixel_shaders"),
                source: wgpu::ShaderSource::Wgsl(SUBPIXEL_SHADERS.into()),
            }))
        } else {
            None
        };

        let blend_mode = match alpha_mode {
            wgpu::CompositeAlphaMode::PreMultiplied => {
                wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING
            }
            _ => wgpu::BlendState::ALPHA_BLENDING,
        };

        let color_target = wgpu::ColorTargetState {
            format: surface_format,
            blend: Some(blend_mode),
            write_mask: wgpu::ColorWrites::ALL,
        };

        let create_pipeline =
            |name: &str,
             vs_entry: &str,
             fs_entry: &str,
             globals_layout: &wgpu::BindGroupLayout,
             data_layout: &wgpu::BindGroupLayout,
             texture_layout: Option<&wgpu::BindGroupLayout>,
             topology: wgpu::PrimitiveTopology,
             color_targets: &[Option<wgpu::ColorTargetState>],
             sample_count: u32,
             module: &wgpu::ShaderModule,
             depth_stencil: Option<wgpu::DepthStencilState>| {
                let mut bind_group_layouts = vec![Some(globals_layout), Some(data_layout)];
                bind_group_layouts.extend(texture_layout.map(Some));
                let pipeline_layout =
                    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some(&format!("{name}_layout")),
                        bind_group_layouts: &bind_group_layouts,
                        immediate_size: 0,
                    });

                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(name),
                    layout: Some(&pipeline_layout),
                    vertex: wgpu::VertexState {
                        module,
                        entry_point: Some(vs_entry),
                        buffers: &[],
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                    },
                    fragment: Some(wgpu::FragmentState {
                        module,
                        entry_point: Some(fs_entry),
                        targets: color_targets,
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology,
                        strip_index_format: None,
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: None,
                        polygon_mode: wgpu::PolygonMode::Fill,
                        unclipped_depth: false,
                        conservative: false,
                    },
                    depth_stencil,
                    multisample: wgpu::MultisampleState {
                        count: sample_count,
                        mask: !0,
                        alpha_to_coverage_enabled: false,
                    },
                    multiview_mask: None,
                    cache: None,
                })
            };

        // Corvene patch: see `set_opaque_depth_pass`
        let depth_tested = depth_format.map(|format| wgpu::DepthStencilState {
            format,
            depth_write_enabled: Some(false),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        });
        let opaque_quads = depth_format.map(|format| {
            create_pipeline(
                "opaque_quads",
                "vs_quad",
                "fs_quad",
                &layouts.globals,
                &layouts.instances,
                None,
                wgpu::PrimitiveTopology::TriangleStrip,
                &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                1,
                &shader_module,
                Some(wgpu::DepthStencilState {
                    format,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::Less),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
            )
        });

        let quads = create_pipeline(
            "quads",
            "vs_quad",
            "fs_quad",
            &layouts.globals,
            &layouts.instances,
            None,
            wgpu::PrimitiveTopology::TriangleStrip,
            &[Some(color_target.clone())],
            1,
            &shader_module,
            depth_tested.clone(),
        );

        let shadows = create_pipeline(
            "shadows",
            "vs_shadow",
            "fs_shadow",
            &layouts.globals,
            &layouts.instances,
            None,
            wgpu::PrimitiveTopology::TriangleStrip,
            &[Some(color_target.clone())],
            1,
            &shader_module,
            depth_tested.clone(),
        );

        let path_rasterization = create_pipeline(
            "path_rasterization",
            "vs_path_rasterization",
            "fs_path_rasterization",
            &layouts.globals,
            &layouts.instances,
            None,
            wgpu::PrimitiveTopology::TriangleList,
            &[Some(wgpu::ColorTargetState {
                format: surface_format,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            path_sample_count,
            &shader_module,
            // drawn into the path textures, in a pass of its own
            None,
        );

        let paths_blend = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
        };

        let paths = create_pipeline(
            "paths",
            "vs_path",
            "fs_path",
            &layouts.globals,
            &layouts.instances,
            Some(&layouts.texture),
            wgpu::PrimitiveTopology::TriangleStrip,
            &[Some(wgpu::ColorTargetState {
                format: surface_format,
                blend: Some(paths_blend),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            1,
            &shader_module,
            depth_tested.clone(),
        );

        let underlines = create_pipeline(
            "underlines",
            "vs_underline",
            "fs_underline",
            &layouts.globals,
            &layouts.instances,
            None,
            wgpu::PrimitiveTopology::TriangleStrip,
            &[Some(color_target.clone())],
            1,
            &shader_module,
            depth_tested.clone(),
        );

        let mono_sprites = create_pipeline(
            "mono_sprites",
            "vs_mono_sprite",
            "fs_mono_sprite",
            &layouts.globals,
            &layouts.instances,
            Some(&layouts.texture),
            wgpu::PrimitiveTopology::TriangleStrip,
            &[Some(color_target.clone())],
            1,
            &shader_module,
            depth_tested.clone(),
        );

        let subpixel_sprites = if let Some(subpixel_module) = &subpixel_shader_module {
            let subpixel_blend = wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::Src1,
                    dst_factor: wgpu::BlendFactor::OneMinusSrc1,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
            };

            Some(create_pipeline(
                "subpixel_sprites",
                "vs_subpixel_sprite",
                "fs_subpixel_sprite",
                &layouts.globals,
                &layouts.instances,
                Some(&layouts.texture),
                wgpu::PrimitiveTopology::TriangleStrip,
                &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(subpixel_blend),
                    write_mask: wgpu::ColorWrites::COLOR,
                })],
                1,
                subpixel_module,
                depth_tested.clone(),
            ))
        } else {
            None
        };

        let poly_sprites = create_pipeline(
            "poly_sprites",
            "vs_poly_sprite",
            "fs_poly_sprite",
            &layouts.globals,
            &layouts.instances,
            Some(&layouts.texture),
            wgpu::PrimitiveTopology::TriangleStrip,
            &[Some(color_target.clone())],
            1,
            &shader_module,
            depth_tested.clone(),
        );

        let surfaces = create_pipeline(
            "surfaces",
            "vs_surface",
            "fs_surface",
            &layouts.globals,
            &layouts.surfaces,
            None,
            wgpu::PrimitiveTopology::TriangleStrip,
            &[Some(color_target)],
            1,
            &shader_module,
            depth_tested,
        );

        WgpuPipelines {
            quads,
            shadows,
            path_rasterization,
            paths,
            underlines,
            mono_sprites,
            subpixel_sprites,
            poly_sprites,
            surfaces,
            opaque_quads,
        }
    }

    fn create_path_intermediate(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("path_intermediate"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    fn create_msaa_if_needed(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        sample_count: u32,
    ) -> Option<(wgpu::Texture, wgpu::TextureView)> {
        if sample_count <= 1 {
            return None;
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("path_msaa"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Some((texture, view))
    }
}

impl WgpuRenderer {
    /// Records the new drawable size and reconfigures the surface when one is present.
    /// The size is kept even without GPU resources so that recovery restores it.
    pub fn update_drawable_size(&mut self, size: Size<DevicePixels>) {
        let width = size.width.0 as u32;
        let height = size.height.0 as u32;

        if width == self.surface_config.width && height == self.surface_config.height {
            return;
        }

        let clamped_width = width.min(self.max_texture_size);
        let clamped_height = height.min(self.max_texture_size);
        if clamped_width != width || clamped_height != height {
            warn!(
                "Requested surface size ({}, {}) exceeds maximum texture dimension {}. \
                 Clamping to ({}, {}). Window content may not fill the entire window.",
                width, height, self.max_texture_size, clamped_width, clamped_height
            );
        }
        self.surface_config.width = clamped_width.max(1);
        self.surface_config.height = clamped_height.max(1);

        let Some(core) = self.core_mut() else {
            return;
        };
        let resources = &mut core.resources;

        // Wait for any in-flight GPU work to complete before destroying textures
        if let Err(e) = resources.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        }) {
            warn!("Failed to poll device during resize: {e:?}");
        }

        // Destroy old textures before allocating new ones to avoid GPU memory spikes
        if let Some(ref texture) = resources.path_intermediate_texture {
            texture.destroy();
        }
        if let Some(ref texture) = resources.path_msaa_texture {
            texture.destroy();
        }
        // Corvene patch: the opaque depth pass's and the damage scissor's
        if let Some(ref texture) = resources.depth_texture {
            texture.destroy();
        }
        if let Some(ref damage) = resources.damage {
            damage.texture.destroy();
        }

        // Invalidate intermediate textures - they will be lazily recreated
        // in draw() after we confirm the surface is healthy. This avoids
        // panics when the device/surface is in an invalid state during resize.
        resources.invalidate_intermediate_textures();

        if let RendererState::Ready { surface, core } = &self.state {
            surface.configure(&core.resources.device, &self.surface_config);
        }
    }

    pub fn set_subpixel_layout(&mut self, is_bgr: bool) {
        self.is_bgr = is_bgr;
        if let Some(core) = self.core_mut() {
            core.is_bgr = is_bgr;
        }
    }

    pub fn update_transparency(&mut self, transparent: bool) {
        let new_alpha_mode = if transparent {
            self.transparent_alpha_mode
        } else {
            self.opaque_alpha_mode
        };
        if new_alpha_mode == self.surface_config.alpha_mode {
            return;
        }
        self.surface_config.alpha_mode = new_alpha_mode;
        let format = self.surface_config.format;

        let Some(core) = self.core_mut() else {
            return;
        };
        let resources = &mut core.resources;
        resources.pipelines = WgpuRendererCore::create_pipelines(
            &resources.device,
            &resources.bind_group_layouts,
            format,
            new_alpha_mode,
            core.rendering_params.path_sample_count,
            core.dual_source_blending,
            core.uses_webgl_instance_data,
            None,
        );
        // Corvene patch: built again with the new blending when next used
        resources.depth_pipelines = None;
        core.alpha_mode = new_alpha_mode;

        if let RendererState::Ready { surface, core } = &self.state {
            surface.configure(&core.resources.device, &self.surface_config);
        }
    }

    #[allow(dead_code)]
    pub fn viewport_size(&self) -> Size<DevicePixels> {
        Size {
            width: DevicePixels(self.surface_config.width as i32),
            height: DevicePixels(self.surface_config.height as i32),
        }
    }

    pub fn sprite_atlas(&self) -> &Arc<WgpuAtlas> {
        &self.atlas
    }

    pub fn supports_dual_source_blending(&self) -> bool {
        self.core().is_some_and(|core| core.dual_source_blending)
    }

    /// Returns `None` once GPU resources have been released by `destroy` or a pending
    /// device recovery.
    pub fn gpu_specs(&self) -> Option<GpuSpecs> {
        let adapter_info = &self.core()?.adapter_info;
        Some(GpuSpecs {
            is_software_emulated: adapter_info.device_type == wgpu::DeviceType::Cpu,
            device_name: adapter_info.name.clone(),
            driver_name: adapter_info.driver.clone(),
            driver_info: adapter_info.driver_info.clone(),
        })
    }

    pub fn max_texture_size(&self) -> u32 {
        self.max_texture_size
    }

    /// Corvene patch: draw `scene` into an offscreen texture with this
    /// window's own core (its sprite atlas holds the scene's glyphs and
    /// images) and read it back, for `PlatformWindow::render_to_image`.
    #[cfg(all(not(target_family = "wasm"), any(test, feature = "test-support")))]
    pub fn render_to_image(&mut self, scene: &Scene) -> anyhow::Result<image::RgbaImage> {
        let size = Size {
            width: DevicePixels(self.surface_config.width as i32),
            height: DevicePixels(self.surface_config.height as i32),
        };
        let core = match &mut self.state {
            RendererState::Ready { core, .. } | RendererState::Unconfigured { core } => core,
            RendererState::Released => anyhow::bail!("the renderer was released"),
        };
        let texture = core
            .resources
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("offscreen_render_target"),
                size: wgpu::Extent3d {
                    width: size.width.0 as u32,
                    height: size.height.0 as u32,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: core.target_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let options = FrameOptions {
            damage_scissor: false,
            ..FrameOptions::current()
        };
        core.render_frame(
            scene,
            FrameTarget::View(&view),
            size,
            false,
            wgpu::Color::BLACK,
            options,
        )?;
        read_texture(core, &texture)
    }

    pub fn draw(&mut self, scene: &Scene) -> bool {
        #[cfg(target_family = "wasm")]
        if self.device_lost() {
            if matches!(self.state, RendererState::Ready { .. }) {
                log::error!(
                    "Browser graphics context was lost; rendering has stopped. Reload the page to recover."
                );
                self.unconfigure_surface();
            }
            return false;
        }

        // Bail out early if the surface has been unconfigured (e.g. during
        // Android background/rotation transitions).  Attempting to acquire
        // a texture from an unconfigured surface can block indefinitely on
        // some drivers (Adreno).
        let RendererState::Ready { surface, core } = &mut self.state else {
            return false;
        };

        if let Some(error) = self.last_surface_error.take().or_else(|| {
            self.device_errors
                .observe_error(&mut self.observed_error_generation)
        }) {
            self.failed_frame_count += 1;
            log::error!(
                "GPU error during frame (failure {} of 10): {error}",
                self.failed_frame_count
            );

            // TBD. Does retrying more actually help?
            if self.failed_frame_count > 10 {
                panic!("Too many consecutive GPU errors. Last error: {error}");
            } else if self.failed_frame_count > 5 {
                core.resources.invalidate_intermediate_textures();
                self.atlas.clear();
                self.needs_redraw = true;
                self.failed_frame_count = 0;
                return false;
            }
        } else {
            self.failed_frame_count = 0;
        }

        // Corvene patch: see `set_damage_scissor`. The swapchain images are
        // copied to when it is on (where they can be), and the retained
        // frame is let go of when it is off.
        let mut options = FrameOptions::current();
        options.damage_scissor &= !self.damage_unusable;
        let copy_dst = options.damage_scissor && self.surface_copy_dst;
        if copy_dst
            != self
                .surface_config
                .usage
                .contains(wgpu::TextureUsages::COPY_DST)
        {
            self.surface_config
                .usage
                .set(wgpu::TextureUsages::COPY_DST, copy_dst);
            surface.configure(&core.resources.device, &self.surface_config);
        }
        if !options.damage_scissor {
            core.resources.damage = None;
        }

        let frame = match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => frame,
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                // Textures must be destroyed before the surface can be reconfigured.
                drop(frame);
                surface.configure(&core.resources.device, &self.surface_config);
                return false;
            }
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                surface.configure(&core.resources.device, &self.surface_config);
                return false;
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return false;
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                self.last_surface_error = Some("Surface texture validation error".to_string());
                return false;
            }
        };

        // The acquired texture is the authority on frame dimensions; the surface
        // configuration is only a request.
        let size = Size {
            width: DevicePixels(frame.texture.width() as i32),
            height: DevicePixels(frame.texture.height() as i32),
        };
        let frame_view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let premultiplied_alpha =
            self.surface_config.alpha_mode == wgpu::CompositeAlphaMode::PreMultiplied;
        // Corvene patch: see `set_damage_scissor`
        let target = if options.damage_scissor {
            FrameTarget::Retained(&frame.texture)
        } else {
            FrameTarget::View(&frame_view)
        };
        if let Err(error) = core.render_frame(
            scene,
            target,
            size,
            premultiplied_alpha,
            wgpu::Color::TRANSPARENT,
            options,
        ) {
            log::error!("{error:#}");
            // Corvene patch: a surface the retained frame cannot be shown on
            // must not stop every frame: draw this one (and the rest) directly
            if !options.damage_scissor {
                return false;
            }
            self.damage_unusable = true;
            core.resources.damage = None;
            options.damage_scissor = false;
            if let Err(error) = core.render_frame(
                scene,
                FrameTarget::View(&frame_view),
                size,
                premultiplied_alpha,
                wgpu::Color::TRANSPARENT,
                options,
            ) {
                log::error!("{error:#}");
                return false;
            }
        }

        // Corvene patch: see `ANDROID_LAST_PRESENT_NANOS`
        #[cfg(target_os = "android")]
        let presenting = std::time::Instant::now();
        frame.present();
        #[cfg(target_os = "android")]
        crate::wgpu_context::ANDROID_LAST_PRESENT_NANOS.store(
            presenting.elapsed().as_nanos() as u64,
            std::sync::atomic::Ordering::Relaxed,
        );
        true
    }
}

impl WgpuRendererCore {
    fn new(
        context: &WgpuContext,
        atlas: Arc<WgpuAtlas>,
        target_format: wgpu::TextureFormat,
        alpha_mode: wgpu::CompositeAlphaMode,
    ) -> Self {
        let device = Arc::clone(&context.device);
        let queue = Arc::clone(&context.queue);
        let rendering_params = RenderingParameters::new(&context.adapter, target_format);
        let uses_webgl_instance_data = context.uses_webgl_instance_data();
        let dual_source_blending =
            context.supports_dual_source_blending() && !uses_webgl_instance_data;
        let bind_group_layouts = Self::create_bind_group_layouts(&device, uses_webgl_instance_data);
        let pipelines = Self::create_pipelines(
            &device,
            &bind_group_layouts,
            target_format,
            alpha_mode,
            rendering_params.path_sample_count,
            dual_source_blending,
            uses_webgl_instance_data,
            None,
        );
        // Corvene patch: see `set_opaque_depth_pass`. Both formats can be
        // render attachments everywhere (WebGPU, Vulkan, GLES 3); 32-bit
        // float tells apart millions of primitives.
        let depth_format = [
            wgpu::TextureFormat::Depth32Float,
            wgpu::TextureFormat::Depth24Plus,
        ]
        .into_iter()
        .find(|format| {
            context
                .adapter
                .get_texture_format_features(*format)
                .allowed_usages
                .contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
        })
        .unwrap_or(wgpu::TextureFormat::Depth24Plus);
        let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("atlas_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let uniform_alignment = device.limits().min_uniform_buffer_offset_alignment as u64;
        let globals_size = std::mem::size_of::<GlobalParams>() as u64;
        let gamma_size = std::mem::size_of::<GammaParams>() as u64;
        let path_globals_offset = globals_size.next_multiple_of(uniform_alignment);
        let gamma_offset = (path_globals_offset + globals_size).next_multiple_of(uniform_alignment);
        let globals_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals_buffer"),
            size: gamma_offset + gamma_size,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let (
            instance_data,
            instance_data_capacity,
            max_instance_data_size,
            instance_data_alignment,
        ) = if uses_webgl_instance_data {
            let max_texture_dimension = device.limits().max_texture_dimension_2d;
            let max_instance_data_size = (u64::from(max_texture_dimension).pow(2)
                * INSTANCE_TEXTURE_TEXEL_SIZE)
                .min(MAX_INSTANCE_BUFFER_SIZE);
            let initial_capacity = (2 * 1024 * 1024).min(max_instance_data_size);
            let (instance_data, capacity) =
                Self::create_instance_texture(&device, initial_capacity, max_texture_dimension);
            (
                instance_data,
                capacity,
                max_instance_data_size,
                INSTANCE_TEXTURE_TEXEL_SIZE,
            )
        } else {
            let max_buffer_size = device
                .limits()
                .max_buffer_size
                .min(device.limits().max_storage_buffer_binding_size)
                .min(MAX_INSTANCE_BUFFER_SIZE);
            let initial_capacity = (2 * 1024 * 1024).min(max_buffer_size);
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("instance_buffer"),
                size: initial_capacity,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            (
                InstanceData::Storage(buffer),
                initial_capacity,
                max_buffer_size,
                device.limits().min_storage_buffer_offset_alignment as u64,
            )
        };
        let globals_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals_bind_group"),
            layout: &bind_group_layouts.globals,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &globals_buffer,
                        offset: 0,
                        size: NonZeroU64::new(globals_size),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &globals_buffer,
                        offset: gamma_offset,
                        size: NonZeroU64::new(gamma_size),
                    }),
                },
            ],
        });
        let path_globals_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("path_globals_bind_group"),
            layout: &bind_group_layouts.globals,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &globals_buffer,
                        offset: path_globals_offset,
                        size: NonZeroU64::new(globals_size),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &globals_buffer,
                        offset: gamma_offset,
                        size: NonZeroU64::new(gamma_size),
                    }),
                },
            ],
        });
        let max_texture_size = device.limits().max_texture_dimension_2d;

        Self {
            resources: WgpuResources {
                device,
                queue,
                pipelines,
                bind_group_layouts,
                atlas_sampler,
                atlas_texture_bind_groups: FxHashMap::default(),
                globals_buffer,
                globals_bind_group,
                path_globals_bind_group,
                instance_data,
                path_intermediate_texture: None,
                path_intermediate_view: None,
                path_msaa_texture: None,
                path_msaa_view: None,
                depth_pipelines: None,
                depth_texture: None,
                depth_view: None,
                damage_pipelines: None,
                damage: None,
            },
            atlas,
            path_globals_offset,
            gamma_offset,
            instance_data_capacity,
            max_instance_data_size,
            instance_data_alignment,
            uses_webgl_instance_data,
            rendering_params,
            is_bgr: false,
            dual_source_blending,
            adapter_info: context.adapter.get_info(),
            target_format,
            max_texture_size,
            depth_format,
            alpha_mode,
            depth_scratch: DepthScratch::default(),
            depth_bytes: Vec::new(),
        }
    }

    fn resources(&self) -> &WgpuResources {
        &self.resources
    }

    fn resources_mut(&mut self) -> &mut WgpuResources {
        &mut self.resources
    }

    /// Path rendering goes through an intermediate texture that must match the frame
    /// target's dimensions, so the textures are keyed by size and rebuilt on mismatch.
    fn ensure_intermediate_textures(&mut self, size: Size<DevicePixels>) {
        let width = size.width.0 as u32;
        let height = size.height.0 as u32;
        if self
            .resources
            .path_intermediate_texture
            .as_ref()
            .is_some_and(|texture| texture.width() == width && texture.height() == height)
        {
            return;
        }

        let format = self.target_format;
        let path_sample_count = self.rendering_params.path_sample_count;
        let resources = &mut self.resources;

        let (texture, view) =
            Self::create_path_intermediate(&resources.device, format, width, height);
        resources.path_intermediate_texture = Some(texture);
        resources.path_intermediate_view = Some(view);

        let (path_msaa_texture, path_msaa_view) = Self::create_msaa_if_needed(
            &resources.device,
            format,
            width,
            height,
            path_sample_count,
        )
        .map(|(texture, view)| (Some(texture), Some(view)))
        .unwrap_or((None, None));
        resources.path_msaa_texture = path_msaa_texture;
        resources.path_msaa_view = path_msaa_view;
    }

    fn render_frame(
        &mut self,
        scene: &Scene,
        target: FrameTarget<'_>,
        size: Size<DevicePixels>,
        premultiplied_alpha: bool,
        clear_color: wgpu::Color,
        options: FrameOptions,
    ) -> Result<wgpu::SubmissionIndex> {
        anyhow::ensure!(
            size.width.0 > 0 && size.height.0 > 0,
            "invalid render target size: {size:?}"
        );
        anyhow::ensure!(
            size.width.0 as u32 <= self.max_texture_size
                && size.height.0 as u32 <= self.max_texture_size,
            "render target size {size:?} exceeds maximum texture dimension {}",
            self.max_texture_size
        );

        self.atlas.before_frame();
        self.ensure_intermediate_textures(size);

        let gamma_params = GammaParams {
            gamma_ratios: self.rendering_params.gamma_ratios,
            grayscale_enhanced_contrast: self.rendering_params.grayscale_enhanced_contrast,
            subpixel_enhanced_contrast: self.rendering_params.subpixel_enhanced_contrast,
            is_bgr: self.is_bgr as u32,
            _pad: 0,
        };
        let globals = GlobalParams {
            viewport_size: [size.width.0 as f32, size.height.0 as f32],
            premultiplied_alpha: premultiplied_alpha as u32,
            order_is_depth: 0,
        };
        let path_globals = GlobalParams {
            premultiplied_alpha: 0,
            ..globals
        };
        self.resources.queue.write_buffer(
            &self.resources.globals_buffer,
            0,
            bytemuck::bytes_of(&globals),
        );
        self.resources.queue.write_buffer(
            &self.resources.globals_buffer,
            self.path_globals_offset,
            bytemuck::bytes_of(&path_globals),
        );
        self.resources.queue.write_buffer(
            &self.resources.globals_buffer,
            self.gamma_offset,
            bytemuck::bytes_of(&gamma_params),
        );

        self.record_frame(
            scene,
            target,
            size,
            premultiplied_alpha,
            clear_color,
            options,
        )
        .inspect_err(|_| {
            // Queue writes are staged before encoding; flush them even if the frame fails.
            self.resources.queue.submit(std::iter::empty());
        })
    }

    fn record_frame(
        &mut self,
        scene: &Scene,
        target: FrameTarget<'_>,
        size: Size<DevicePixels>,
        premultiplied_alpha: bool,
        clear_color: wgpu::Color,
        options: FrameOptions,
    ) -> Result<wgpu::SubmissionIndex> {
        // Corvene patch: see `without_hidden_quads` (decided here, so that
        // the opaque depth pass draws the quads the frame draws)
        #[cfg(target_os = "android")]
        let visible = without_hidden_quads(&scene.quads);
        #[cfg(target_os = "android")]
        let quads: &[Quad] = visible.as_deref().unwrap_or(&scene.quads);
        #[cfg(not(target_os = "android"))]
        let quads: &[Quad] = &scene.quads;

        // Corvene patch: see `set_damage_scissor`. The retained frame is
        // drawn into (only where the scene changed) and then shown.
        let (frame_view, presented, damage) = match target {
            FrameTarget::View(view) => (view.clone(), None, None),
            FrameTarget::Retained(texture) => {
                let key = DamageKey {
                    size,
                    clear_color,
                    premultiplied_alpha,
                    is_bgr: self.is_bgr,
                    atlas_version: self.atlas.version(),
                    opaque_depth_pass: options.opaque_depth_pass,
                };
                let (view, plan, shown) = self.plan_damage(scene, key)?;
                (view, Some(texture), Some((plan, key, shown)))
            }
        };
        let plan = damage
            .as_ref()
            .map_or(DamagePlan::Full, |(plan, _, _)| *plan);

        // Corvene patch: see `set_opaque_depth_pass`
        let mut depth = std::mem::take(&mut self.depth_scratch);
        let use_depth = options.opaque_depth_pass
            && plan != DamagePlan::Unchanged
            && self.prepare_depth(&mut depth, scene, quads, size);

        let mut encoder =
            self.resources()
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("main_encoder"),
                });
        let encoded = if plan == DamagePlan::Unchanged {
            Ok(())
        } else {
            self.encode_scene(
                &mut encoder,
                scene,
                quads,
                &frame_view,
                size,
                clear_color,
                plan,
                use_depth.then_some(&depth),
            )
        };
        self.depth_scratch = depth;
        encoded?;
        if let Some(presented) = presented {
            self.present_retained(&mut encoder, presented)?;
        }

        let submission = self
            .resources()
            .queue
            .submit(std::iter::once(encoder.finish()));
        // Corvene patch: the retained frame now shows this scene (which,
        // unchanged, it already knows)
        if let Some((plan, key, shown)) = damage {
            let mut shown = shown.unwrap_or_default();
            if plan != DamagePlan::Unchanged {
                shown.fill(scene, key);
            }
            if let Some(damage) = self.resources.damage.as_mut() {
                damage.shown = Some(shown);
            }
        }
        Ok(submission)
    }

    /// Draws `scene` into `frame_view`: the frame's body before the Corvene
    /// patches for the opaque depth pass (`depth`) and the damage scissor
    /// (`plan`).
    fn encode_scene(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        scene: &Scene,
        quads: &[Quad],
        frame_view: &wgpu::TextureView,
        size: Size<DevicePixels>,
        clear_color: wgpu::Color,
        plan: DamagePlan,
        depth: Option<&DepthScratch>,
    ) -> Result<()> {
        let mut instance_offset = 0;
        let instance_bindings = self
            .write_instances(scene, quads, depth, &mut instance_offset)
            .with_context(|| {
                format!(
                    "scene too large: {} paths, {} shadows, {} quads, {} underlines, {} monochrome sprites, {} subpixel sprites, {} polychrome sprites",
                    scene.paths.len(),
                    scene.shadows.len(),
                    scene.quads.len(),
                    scene.underlines.len(),
                    scene.monochrome_sprites.len(),
                    scene.subpixel_sprites.len(),
                    scene.polychrome_sprites.len(),
                )
            })?;
        self.prepare_texture_bind_groups(scene);

        // Corvene patch: see `set_opaque_depth_pass`
        let opaque_quads = match depth {
            Some(depth) => {
                self.resources.queue.write_buffer(
                    &self.resources.globals_buffer,
                    std::mem::offset_of!(GlobalParams, order_is_depth) as u64,
                    bytemuck::bytes_of(&1u32),
                );
                let binding = self.write_instance_binding(
                    "opaque_quads_bind_group",
                    &mut instance_offset,
                    &depth.opaque_quads,
                )?;
                Some((binding, depth.opaque_quads.len() as u32))
            }
            None => None,
        };
        let depth_view = depth.and(self.resources.depth_view.clone());
        // kept between the passes that paths split the frame into
        let depth_store = if scene.paths.is_empty() {
            wgpu::StoreOp::Discard
        } else {
            wgpu::StoreOp::Store
        };
        let depth_attachment = |load| depth_view.as_ref().map(|view| (view, load, depth_store));
        let depth_tested = depth.is_some();

        // Corvene patch: see `set_damage_scissor`
        let scissor = match plan {
            DamagePlan::Partial(scissor) => Some(scissor),
            DamagePlan::Full | DamagePlan::Unchanged => None,
        };

        {
            let mut pass = begin_main_pass(
                encoder,
                "main_pass",
                frame_view,
                match scissor {
                    Some(_) => wgpu::LoadOp::Load,
                    None => wgpu::LoadOp::Clear(clear_color),
                },
                depth_attachment(wgpu::LoadOp::Clear(1.0)),
            );
            if let Some([x, y, width, height]) = scissor {
                pass.set_scissor_rect(x, y, width, height);
                self.clear_damage(&mut pass, clear_color, depth_tested);
            }
            if let Some((binding, count)) = &opaque_quads
                && let Some(pipeline) = &self.pipelines(depth_tested).opaque_quads
            {
                self.draw_instances(binding, pipeline, 0..*count, &mut pass);
            }

            let mut path_batches = depth.map(|depth| depth.path_batches.iter().copied());
            for batch in scene.batches() {
                match batch {
                    PrimitiveBatch::Quads(range) => self.draw_instances(
                        &instance_bindings.quads,
                        &self.pipelines(depth_tested).quads,
                        instance_range(range),
                        &mut pass,
                    ),
                    PrimitiveBatch::Shadows(range) => self.draw_instances(
                        &instance_bindings.shadows,
                        &self.pipelines(depth_tested).shadows,
                        instance_range(range),
                        &mut pass,
                    ),
                    PrimitiveBatch::Paths(range) => {
                        // Corvene patch: see `set_opaque_depth_pass`
                        let path_depth = path_batches.as_mut().and_then(Iterator::next);
                        let paths = &scene.paths[range];
                        if paths.is_empty() {
                            continue;
                        }

                        drop(pass);
                        let rasterized =
                            self.draw_paths_to_intermediate(encoder, paths, &mut instance_offset)?;

                        pass = begin_main_pass(
                            encoder,
                            "main_pass_continued",
                            frame_view,
                            wgpu::LoadOp::Load,
                            depth_attachment(wgpu::LoadOp::Load),
                        );
                        // Corvene patch: see `set_damage_scissor`
                        if let Some([x, y, width, height]) = scissor {
                            pass.set_scissor_rect(x, y, width, height);
                        }

                        if rasterized {
                            self.draw_paths_from_intermediate(
                                paths,
                                &mut instance_offset,
                                &mut pass,
                                path_depth.map(|depth| (depth, size)),
                            )?;
                        }
                    }
                    PrimitiveBatch::Underlines(range) => self.draw_instances(
                        &instance_bindings.underlines,
                        &self.pipelines(depth_tested).underlines,
                        instance_range(range),
                        &mut pass,
                    ),
                    PrimitiveBatch::MonochromeSprites { texture_id, range } => {
                        self.draw_sprites(
                            &instance_bindings.monochrome_sprites,
                            texture_id,
                            &self.pipelines(depth_tested).mono_sprites,
                            instance_range(range),
                            &mut pass,
                        )?;
                    }
                    PrimitiveBatch::SubpixelSprites { texture_id, range } => {
                        let pipelines = self.pipelines(depth_tested);
                        self.draw_sprites(
                            &instance_bindings.subpixel_sprites,
                            texture_id,
                            pipelines
                                .subpixel_sprites
                                .as_ref()
                                .unwrap_or(&pipelines.mono_sprites),
                            instance_range(range),
                            &mut pass,
                        )?;
                    }
                    PrimitiveBatch::PolychromeSprites { texture_id, range } => {
                        self.draw_sprites(
                            &instance_bindings.polychrome_sprites,
                            texture_id,
                            &self.pipelines(depth_tested).poly_sprites,
                            instance_range(range),
                            &mut pass,
                        )?;
                    }
                    // Surfaces are macOS-only for video playback and are not
                    // implemented by the WGPU renderer.
                    PrimitiveBatch::Surfaces(_surfaces) => {}
                }
            }
        }
        Ok(())
    }

    /// Corvene patch: the pipelines of a frame, with the opaque depth pass's
    /// depth test (see `set_opaque_depth_pass`) or without.
    fn pipelines(&self, depth_tested: bool) -> &WgpuPipelines {
        match &self.resources.depth_pipelines {
            Some(pipelines) if depth_tested => pipelines,
            _ => &self.resources.pipelines,
        }
    }

    fn write_instances(
        &mut self,
        scene: &Scene,
        quads: &[Quad],
        // Corvene patch: see `set_opaque_depth_pass`
        depth: Option<&DepthScratch>,
        instance_offset: &mut u64,
    ) -> Result<InstanceBindings> {
        Ok(InstanceBindings {
            quads: self.write_ordered_binding(
                "quads_bind_group",
                instance_offset,
                quads,
                depth.map(|depth| &depth.quads[..]),
            )?,
            shadows: self.write_ordered_binding(
                "shadows_bind_group",
                instance_offset,
                &scene.shadows,
                depth.map(|depth| &depth.shadows[..]),
            )?,
            underlines: self.write_ordered_binding(
                "underlines_bind_group",
                instance_offset,
                &scene.underlines,
                depth.map(|depth| &depth.underlines[..]),
            )?,
            monochrome_sprites: self.write_ordered_binding(
                "monochrome_sprites_bind_group",
                instance_offset,
                &scene.monochrome_sprites,
                depth.map(|depth| &depth.monochrome_sprites[..]),
            )?,
            subpixel_sprites: self.write_ordered_binding(
                "subpixel_sprites_bind_group",
                instance_offset,
                &scene.subpixel_sprites,
                depth.map(|depth| &depth.subpixel_sprites[..]),
            )?,
            polychrome_sprites: self.write_ordered_binding(
                "polychrome_sprites_bind_group",
                instance_offset,
                &scene.polychrome_sprites,
                depth.map(|depth| &depth.polychrome_sprites[..]),
            )?,
        })
    }

    /// Corvene patch: `write_instance_binding`, with each record's `order`
    /// (its first four bytes) replaced by its depth when `depths` is given
    /// (see `set_opaque_depth_pass` and `with_order_depth` in the shaders).
    fn write_ordered_binding<T>(
        &mut self,
        label: &str,
        instance_offset: &mut u64,
        instances: &[T],
        depths: Option<&[f32]>,
    ) -> Result<InstanceBinding> {
        let Some(depths) = depths else {
            return self.write_instance_binding(label, instance_offset, instances);
        };
        let stride = std::mem::size_of::<T>();
        anyhow::ensure!(
            stride >= 4 && depths.len() == instances.len(),
            "{label}: {} depths for {} instances",
            depths.len(),
            instances.len()
        );
        let mut bytes = std::mem::take(&mut self.depth_bytes);
        bytes.clear();
        bytes.extend_from_slice(unsafe { Self::instance_bytes(instances) });
        for (record, depth) in bytes.chunks_exact_mut(stride).zip(depths) {
            record[..4].copy_from_slice(&depth.to_bits().to_ne_bytes());
        }
        let binding = self.write_instance_bytes(label, instance_offset, &bytes, stride as u64);
        self.depth_bytes = bytes;
        binding
    }

    /// Corvene patch: see `set_opaque_depth_pass`. Fills `depth` for this
    /// frame and makes the pipelines and the depth buffer it needs; false
    /// when the frame is drawn as well without (no opaque quad, or more
    /// primitives than the depth values tell apart).
    fn prepare_depth(
        &mut self,
        depth: &mut DepthScratch,
        scene: &Scene,
        quads: &[Quad],
        size: Size<DevicePixels>,
    ) -> bool {
        if !depth.fill(scene, quads) {
            return false;
        }
        let resources = &mut self.resources;
        if resources.depth_pipelines.is_none() {
            resources.depth_pipelines = Some(Self::create_pipelines(
                &resources.device,
                &resources.bind_group_layouts,
                self.target_format,
                self.alpha_mode,
                self.rendering_params.path_sample_count,
                self.dual_source_blending,
                self.uses_webgl_instance_data,
                Some(self.depth_format),
            ));
        }
        let (width, height) = (size.width.0 as u32, size.height.0 as u32);
        if !resources
            .depth_texture
            .as_ref()
            .is_some_and(|texture| texture.width() == width && texture.height() == height)
        {
            let texture = resources.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("opaque_depth"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.depth_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            resources.depth_view =
                Some(texture.create_view(&wgpu::TextureViewDescriptor::default()));
            resources.depth_texture = Some(texture);
        }
        true
    }

    /// Corvene patch: see `set_damage_scissor`. Makes the retained frame
    /// (and the helper pipelines) for a frame of `key.size` and decides what
    /// of it to draw again; also returns what it showed, for its memory.
    fn plan_damage(
        &mut self,
        scene: &Scene,
        key: DamageKey,
    ) -> Result<(wgpu::TextureView, DamagePlan, Option<DamageSnapshot>)> {
        let resources = &mut self.resources;
        if resources.damage_pipelines.is_none() {
            resources.damage_pipelines = Some(DamagePipelines::new(
                &resources.device,
                self.target_format,
                self.depth_format,
            ));
        }
        let (width, height) = (key.size.width.0 as u32, key.size.height.0 as u32);
        if !resources.damage.as_ref().is_some_and(|damage| {
            damage.texture.width() == width && damage.texture.height() == height
        }) {
            let texture = resources.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("retained_frame"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.target_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            resources.damage = Some(DamageState {
                texture,
                view,
                shown: None,
            });
        }
        let damage = resources
            .damage
            .as_mut()
            .context("the retained frame was not created")?;
        // unknown until this frame is submitted
        let shown = damage.shown.take();
        let plan = match &shown {
            Some(shown) if shown.key == key => shown.plan(scene, key.size),
            _ => DamagePlan::Full,
        };
        Ok((damage.view.clone(), plan, shown))
    }

    /// Corvene patch: see `set_damage_scissor`. Fills the scissor rectangle
    /// of `pass` with the clear colour.
    fn clear_damage(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        clear_color: wgpu::Color,
        depth_tested: bool,
    ) {
        let Some(pipelines) = &self.resources.damage_pipelines else {
            return;
        };
        pass.set_pipeline(if depth_tested {
            &pipelines.clear_with_depth
        } else {
            &pipelines.clear
        });
        pass.set_blend_constant(clear_color);
        pass.draw(0..3, 0..1);
    }

    /// Corvene patch: see `set_damage_scissor`. Copies the retained frame to
    /// `presented` (a swapchain image), with a texture copy when its usage
    /// allows, otherwise with a draw.
    fn present_retained(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        presented: &wgpu::Texture,
    ) -> Result<()> {
        let resources = &self.resources;
        let damage = resources
            .damage
            .as_ref()
            .context("the retained frame was not created")?;
        anyhow::ensure!(
            presented.size() == damage.texture.size(),
            "the retained frame is {:?}, the presented one {:?}",
            damage.texture.size(),
            presented.size()
        );
        if presented.usage().contains(wgpu::TextureUsages::COPY_DST)
            && presented.format() == damage.texture.format()
        {
            encoder.copy_texture_to_texture(
                damage.texture.as_image_copy(),
                presented.as_image_copy(),
                damage.texture.size(),
            );
            return Ok(());
        }
        anyhow::ensure!(
            presented.format() == self.target_format,
            "cannot show the retained frame in {:?}",
            presented.format()
        );
        let pipelines = resources
            .damage_pipelines
            .as_ref()
            .context("the damage pipelines were not created")?;
        let bind_group = resources
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("retained_frame_bind_group"),
                layout: &pipelines.blit_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&damage.view),
                }],
            });
        let view = presented.create_view(&wgpu::TextureViewDescriptor::default());
        let mut pass = begin_main_pass(
            encoder,
            "retained_frame_blit",
            &view,
            // every pixel is written: nothing to load
            wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
            None,
        );
        pass.set_pipeline(&pipelines.blit);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
        Ok(())
    }

    fn create_texture_bind_group(
        &self,
        label: &str,
        texture_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        let resources = self.resources();
        resources
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &resources.bind_group_layouts.texture,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&resources.atlas_sampler),
                    },
                ],
            })
    }

    fn prepare_texture_bind_groups(&mut self, scene: &Scene) {
        let mut texture_ids = SmallVec::<[AtlasTextureId; 8]>::new();
        for batch in scene.batches() {
            let texture_id = match batch {
                PrimitiveBatch::MonochromeSprites { texture_id, .. }
                | PrimitiveBatch::SubpixelSprites { texture_id, .. }
                | PrimitiveBatch::PolychromeSprites { texture_id, .. } => texture_id,
                _ => continue,
            };
            if !texture_ids.contains(&texture_id) {
                texture_ids.push(texture_id);
            }
        }

        self.resources_mut()
            .atlas_texture_bind_groups
            .retain(|texture_id, _| texture_ids.contains(texture_id));

        for texture_id in texture_ids {
            let Some(texture_info) = self.atlas.get_texture_info(texture_id) else {
                self.resources_mut()
                    .atlas_texture_bind_groups
                    .remove(&texture_id);
                continue;
            };
            let is_current = self
                .resources()
                .atlas_texture_bind_groups
                .get(&texture_id)
                .is_some_and(|cached| cached.texture_generation == texture_info.generation);
            if is_current {
                continue;
            }

            let bind_group =
                self.create_texture_bind_group("atlas_texture_bind_group", &texture_info.view);
            self.resources_mut().atlas_texture_bind_groups.insert(
                texture_id,
                CachedTextureBindGroup {
                    texture_generation: texture_info.generation,
                    bind_group,
                },
            );
        }
    }

    fn draw_instances(
        &self,
        instances: &InstanceBinding,
        pipeline: &wgpu::RenderPipeline,
        range: Range<u32>,
        pass: &mut wgpu::RenderPass<'_>,
    ) {
        if range.is_empty() {
            return;
        }
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &self.resources().globals_bind_group, &[]);
        pass.set_bind_group(1, &instances.bind_group, &[]);
        pass.draw(
            0..4,
            instances.first_instance + range.start..instances.first_instance + range.end,
        );
    }

    fn draw_sprites(
        &self,
        sprite_instances: &InstanceBinding,
        texture_id: AtlasTextureId,
        pipeline: &wgpu::RenderPipeline,
        range: Range<u32>,
        pass: &mut wgpu::RenderPass<'_>,
    ) -> Result<()> {
        if range.is_empty() {
            return Ok(());
        }
        let resources = self.resources();
        // The atlas has released this texture; the batch belongs to a stale
        // paint that will be replaced once its view re-renders.
        let Some(texture) = resources.atlas_texture_bind_groups.get(&texture_id) else {
            return Ok(());
        };
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &resources.globals_bind_group, &[]);
        pass.set_bind_group(1, &sprite_instances.bind_group, &[]);
        pass.set_bind_group(2, &texture.bind_group, &[]);
        pass.draw(
            0..4,
            sprite_instances.first_instance + range.start
                ..sprite_instances.first_instance + range.end,
        );
        Ok(())
    }

    unsafe fn instance_bytes<T>(instances: &[T]) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(
                instances.as_ptr() as *const u8,
                std::mem::size_of_val(instances),
            )
        }
    }

    fn draw_paths_from_intermediate(
        &mut self,
        paths: &[Path<ScaledPixels>],
        instance_offset: &mut u64,
        pass: &mut wgpu::RenderPass<'_>,
        // Corvene patch: the batch's depth and the frame's size, with the
        // opaque depth pass (see `set_opaque_depth_pass`)
        depth: Option<(f32, Size<DevicePixels>)>,
    ) -> Result<()> {
        let first_path = &paths[0];
        let sprites: Vec<PathSprite> = if paths.last().map(|p| &p.order) == Some(&first_path.order)
        {
            paths
                .iter()
                .map(|p| PathSprite {
                    bounds: p.clipped_bounds(),
                })
                .collect()
        } else {
            let mut bounds = first_path.clipped_bounds();
            for path in paths.iter().skip(1) {
                bounds = bounds.union(&path.clipped_bounds());
            }
            vec![PathSprite { bounds }]
        };

        let Some(path_intermediate_view) = self.resources().path_intermediate_view.clone() else {
            return Ok(());
        };
        let instances =
            self.write_instance_binding("path_sprites_bind_group", instance_offset, &sprites)?;
        let texture = self.create_texture_bind_group(
            "path_intermediate_texture_bind_group",
            &path_intermediate_view,
        );
        let resources = self.resources();
        pass.set_pipeline(&self.pipelines(depth.is_some()).paths);
        pass.set_bind_group(0, &resources.globals_bind_group, &[]);
        pass.set_bind_group(1, &instances.bind_group, &[]);
        pass.set_bind_group(2, &texture, &[]);
        // Corvene patch: the path sprites carry no order to take a depth
        // from, so the viewport's depth range puts the whole batch (z = 0)
        // at its depth
        if let Some((depth, size)) = depth {
            let (width, height) = (size.width.0 as f32, size.height.0 as f32);
            pass.set_viewport(0.0, 0.0, width, height, depth, depth);
        }
        pass.draw(
            0..4,
            instances.first_instance..instances.first_instance + sprites.len() as u32,
        );
        if let Some((_, size)) = depth {
            let (width, height) = (size.width.0 as f32, size.height.0 as f32);
            pass.set_viewport(0.0, 0.0, width, height, 0.0, 1.0);
        }
        Ok(())
    }

    fn draw_paths_to_intermediate(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        paths: &[Path<ScaledPixels>],
        instance_offset: &mut u64,
    ) -> Result<bool> {
        let mut vertices = Vec::new();
        for path in paths {
            let bounds = path.clipped_bounds();
            vertices.extend(path.vertices.iter().map(|v| PathRasterizationVertex {
                xy_position: v.xy_position,
                st_position: v.st_position,
                color: path.color,
                bounds,
            }));
        }

        if vertices.is_empty() {
            return Ok(false);
        }

        let vertex_binding = self.write_instance_binding(
            "path_rasterization_bind_group",
            instance_offset,
            &vertices,
        )?;

        let resources = self.resources();
        let Some(path_intermediate_view) = resources.path_intermediate_view.as_ref() else {
            return Ok(false);
        };

        let (target_view, resolve_target) = if let Some(ref msaa_view) = resources.path_msaa_view {
            (msaa_view, Some(path_intermediate_view))
        } else {
            (path_intermediate_view, None)
        };

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("path_rasterization_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target_view,
                    resolve_target,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });

            pass.set_pipeline(&resources.pipelines.path_rasterization);
            pass.set_bind_group(0, &resources.path_globals_bind_group, &[]);
            pass.set_bind_group(1, &vertex_binding.bind_group, &[]);
            // The path rasterization shader loads records by vertex index
            // rather than instance index, so the allocation's base shifts the
            // vertex range here.
            pass.draw(
                vertex_binding.first_instance
                    ..vertex_binding.first_instance + vertices.len() as u32,
                0..1,
            );
        }

        Ok(true)
    }

    fn write_instance_binding<T>(
        &mut self,
        label: &str,
        instance_offset: &mut u64,
        instances: &[T],
    ) -> Result<InstanceBinding> {
        let data = unsafe { Self::instance_bytes(instances) };
        self.write_instance_bytes(
            label,
            instance_offset,
            data,
            std::mem::size_of::<T>() as u64,
        )
    }

    /// Corvene patch: the body of `write_instance_binding`, for records of
    /// `stride` bytes given as bytes (see `write_ordered_binding`).
    fn write_instance_bytes(
        &mut self,
        label: &str,
        instance_offset: &mut u64,
        data: &[u8],
        stride: u64,
    ) -> Result<InstanceBinding> {
        // wgpu rejects zero-sized bindings, so empty primitive arrays still
        // reserve the 16-byte minimum.
        let size = (data.len() as u64).max(16);
        let stride = stride.max(1);
        let (alignment, allocation_size) = if self.uses_webgl_instance_data {
            // The texture transport has no binding offset: the shader indexes
            // the instance texture absolutely, so each allocation must start on
            // a whole instance (a stride multiple) and a whole texel, and must
            // end on a texel boundary so the zero padding of its final partial
            // texel cannot overlap the next allocation.
            (
                least_common_multiple(self.instance_data_alignment, stride),
                size.next_multiple_of(INSTANCE_TEXTURE_TEXEL_SIZE),
            )
        } else {
            (self.instance_data_alignment.max(1), size)
        };
        let mut offset = (*instance_offset).next_multiple_of(alignment);
        if offset + allocation_size > self.instance_data_capacity {
            self.grow_instance_data(allocation_size)?;
            offset = 0;
        }
        *instance_offset = offset + allocation_size;

        let first_instance = if self.uses_webgl_instance_data {
            u32::try_from(offset / stride).context("instance index exceeds u32 range")?
        } else {
            0
        };

        let resources = self.resources();
        if !data.is_empty() {
            match &resources.instance_data {
                InstanceData::Storage(buffer) => resources.queue.write_buffer(buffer, offset, data),
                InstanceData::Texture { .. } => {
                    Self::write_instance_texture(resources, offset, data)
                }
            }
        }
        let bind_group = resources
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &resources.bind_group_layouts.instances,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: match &resources.instance_data {
                        InstanceData::Storage(buffer) => {
                            wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                buffer,
                                offset,
                                size: NonZeroU64::new(size),
                            })
                        }
                        InstanceData::Texture { view, .. } => {
                            wgpu::BindingResource::TextureView(view)
                        }
                    },
                }],
            });
        Ok(InstanceBinding {
            bind_group,
            first_instance,
        })
    }

    fn write_instance_texture(resources: &WgpuResources, offset: u64, data: &[u8]) {
        let InstanceData::Texture {
            texture,
            width,
            height,
            ..
        } = &resources.instance_data
        else {
            return;
        };
        let mut byte_offset = 0usize;
        let mut texel_offset = offset / INSTANCE_TEXTURE_TEXEL_SIZE;
        while byte_offset < data.len() {
            let x = (texel_offset % u64::from(*width)) as u32;
            let y = (texel_offset / u64::from(*width)) as u32;
            if y >= *height {
                // The capacity check in write_instance_binding should make this
                // unreachable. Truncating silently would leave stale bytes in the
                // texture and draw garbage for the remaining instances.
                debug_assert!(
                    false,
                    "instance texture write out of bounds: row {y} >= height {}",
                    *height
                );
                log::error!(
                    "instance texture write out of bounds; dropping {} bytes of instance data",
                    data.len() - byte_offset
                );
                return;
            }
            let available_texels = u64::from(*width - x);
            let remaining_bytes = data.len() - byte_offset;
            let complete_texels = remaining_bytes as u64 / INSTANCE_TEXTURE_TEXEL_SIZE;
            let texels = complete_texels.min(available_texels);
            if texels > 0 {
                let byte_count = (texels * INSTANCE_TEXTURE_TEXEL_SIZE) as usize;
                resources.queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d { x, y, z: 0 },
                        aspect: wgpu::TextureAspect::All,
                    },
                    &data[byte_offset..byte_offset + byte_count],
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(byte_count as u32),
                        rows_per_image: None,
                    },
                    wgpu::Extent3d {
                        width: texels as u32,
                        height: 1,
                        depth_or_array_layers: 1,
                    },
                );
                byte_offset += byte_count;
                texel_offset += texels;
                continue;
            }

            let mut final_texel = [0; INSTANCE_TEXTURE_TEXEL_SIZE as usize];
            final_texel[..remaining_bytes].copy_from_slice(&data[byte_offset..]);
            resources.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x, y, z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                &final_texel,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(INSTANCE_TEXTURE_TEXEL_SIZE as u32),
                    rows_per_image: None,
                },
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            );
            break;
        }
    }

    fn grow_instance_data(&mut self, required: u64) -> Result<()> {
        let capacity = (self.instance_data_capacity * 2)
            .max(required.next_power_of_two())
            .min(self.max_instance_data_size);
        anyhow::ensure!(
            capacity >= required,
            "instance data needs {required} bytes, above the maximum of {}",
            self.max_instance_data_size
        );
        anyhow::ensure!(
            capacity > self.instance_data_capacity,
            "frame instance data exceeds the {}-byte maximum",
            self.max_instance_data_size
        );
        log::debug!(
            "instance data grown from {} to {capacity}",
            self.instance_data_capacity
        );
        // Bind groups created earlier in the frame keep the previous buffer or
        // texture alive, so allocations written before the grow remain valid;
        // only subsequent writes land in the new allocation.
        let uses_webgl_instance_data = self.uses_webgl_instance_data;
        let resources = self.resources_mut();
        if uses_webgl_instance_data {
            let max_texture_dimension = resources.device.limits().max_texture_dimension_2d;
            let (instance_data, actual_capacity) =
                Self::create_instance_texture(&resources.device, capacity, max_texture_dimension);
            resources.instance_data = instance_data;
            self.instance_data_capacity = actual_capacity;
        } else {
            resources.instance_data =
                InstanceData::Storage(resources.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("instance_buffer"),
                    size: capacity,
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));
            self.instance_data_capacity = capacity;
        }
        Ok(())
    }
}

impl WgpuRenderer {
    /// Mark the surface as unconfigured so rendering is skipped until a new
    /// surface is provided via [`replace_surface`](Self::replace_surface).
    ///
    /// This does **not** drop the renderer — the device, queue, atlas, and
    /// pipelines stay alive.  Use this when the native window is destroyed
    /// (e.g. Android `TerminateWindow`) but you intend to re-create the
    /// surface later without losing cached atlas textures.
    pub fn unconfigure_surface(&mut self) {
        self.state = match std::mem::replace(&mut self.state, RendererState::Released) {
            RendererState::Ready { core, surface } => {
                drop(surface);
                RendererState::Unconfigured { core }
            }
            state @ (RendererState::Unconfigured { .. } | RendererState::Released) => state,
        };
        // Drop intermediate textures since they reference the old surface size.
        if let Some(core) = self.core_mut() {
            core.resources.invalidate_intermediate_textures();
        }
    }

    /// Replace the wgpu surface with a new one (e.g. after Android destroys
    /// and recreates the native window).  Keeps the device, queue, atlas, and
    /// all pipelines intact so cached `AtlasTextureId`s remain valid.
    ///
    /// The `instance` **must** be the same [`wgpu::Instance`] that was used to
    /// create the adapter and device (i.e. from the [`WgpuContext`]).  Using a
    /// different instance will cause a "Device does not exist" panic because
    /// the wgpu device is bound to its originating instance.
    #[cfg(not(target_family = "wasm"))]
    pub fn replace_surface<W: HasWindowHandle>(
        &mut self,
        window: &W,
        config: WgpuSurfaceConfig,
        instance: &wgpu::Instance,
    ) -> anyhow::Result<()> {
        let window_handle = window
            .window_handle()
            .map_err(|e| anyhow::anyhow!("Failed to get window handle: {e}"))?;

        let surface = create_surface(instance, window_handle.as_raw())?;

        let width = (config.size.width.0 as u32).max(1);
        let height = (config.size.height.0 as u32).max(1);

        let alpha_mode = if config.transparent {
            self.transparent_alpha_mode
        } else {
            self.opaque_alpha_mode
        };

        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface_config.alpha_mode = alpha_mode;
        if let Some(mode) = config.preferred_present_mode {
            self.surface_config.present_mode = mode;
        }

        let mut core = match std::mem::replace(&mut self.state, RendererState::Released) {
            RendererState::Ready {
                core,
                surface: old_surface,
            } => {
                drop(old_surface);
                core
            }
            RendererState::Unconfigured { core } => core,
            RendererState::Released => {
                anyhow::bail!("Cannot replace the surface: GPU resources have been released")
            }
        };
        surface.configure(&core.resources.device, &self.surface_config);
        core.resources.invalidate_intermediate_textures();
        self.state = RendererState::Ready { surface, core };

        Ok(())
    }

    pub fn destroy(&mut self) {
        // Release surface-bound GPU resources eagerly so the underlying native
        // window can be destroyed before the renderer itself is dropped.
        self.state = RendererState::Released;
    }

    /// Returns true if the GPU device was lost and recovery is needed.
    pub fn device_lost(&self) -> bool {
        self.device_errors.device_lost()
    }

    /// Returns true if a redraw is needed because GPU state was cleared.
    /// Calling this method clears the flag.
    pub fn needs_redraw(&mut self) -> bool {
        std::mem::take(&mut self.needs_redraw)
    }

    /// Recovers from a lost GPU device by recreating the renderer with a new context.
    ///
    /// Call this after detecting `device_lost()` returns true.
    ///
    /// This method coordinates recovery across multiple windows:
    /// - The first window to call this will recreate the shared context
    /// - Subsequent windows will adopt the already-recovered context
    #[cfg(not(target_family = "wasm"))]
    pub fn recover<W>(&mut self, window: &W) -> anyhow::Result<()>
    where
        W: HasWindowHandle + HasDisplayHandle + std::fmt::Debug + Send + Sync + Clone + 'static,
    {
        let gpu_context = self.context.as_ref().expect("recover requires gpu_context");

        // Check if another window already recovered the context
        let needs_new_context = gpu_context
            .borrow()
            .as_ref()
            .is_none_or(|ctx| ctx.device_lost());

        let window_handle = window
            .window_handle()
            .map_err(|e| anyhow::anyhow!("Failed to get window handle: {e}"))?;

        let surface = if needs_new_context {
            log::warn!("GPU device lost, recreating context...");

            // Drop old resources to release Arc<Device>/Arc<Queue> and GPU resources
            self.state = RendererState::Released;
            *gpu_context.borrow_mut() = None;

            // Wait briefly for the GPU driver to stabilize, then try to
            // recreate the context without software renderers. If this fails
            // the caller should request another frame and retry — the real GPU
            // may need more time to come back (e.g. after suspend/resume).
            std::thread::sleep(std::time::Duration::from_millis(350));

            let instance = WgpuContext::instance(Some(Box::new(window.clone())));
            let surface = create_surface(&instance, window_handle.as_raw())?;
            let new_context =
                WgpuContext::new_rejecting_software(instance, &surface, self.compositor_gpu)?;
            *gpu_context.borrow_mut() = Some(new_context);
            surface
        } else {
            let ctx_ref = gpu_context.borrow();
            let instance = &ctx_ref.as_ref().unwrap().instance;
            create_surface(instance, window_handle.as_raw())?
        };

        let config = WgpuSurfaceConfig {
            size: gpui::Size {
                width: gpui::DevicePixels(self.surface_config.width as i32),
                height: gpui::DevicePixels(self.surface_config.height as i32),
            },
            transparent: self.surface_config.alpha_mode != wgpu::CompositeAlphaMode::Opaque,
            preferred_present_mode: Some(self.surface_config.present_mode),
        };
        let gpu_context = Rc::clone(gpu_context);
        let ctx_ref = gpu_context.borrow();
        let context = ctx_ref.as_ref().expect("context should exist");

        self.state = RendererState::Released;
        self.atlas.handle_device_lost(context);

        let is_bgr = self.is_bgr;
        *self = Self::new_internal(
            Some(gpu_context.clone()),
            context,
            surface,
            config,
            self.compositor_gpu,
            self.atlas.clone(),
        )?;
        self.set_subpixel_layout(is_bgr);

        log::info!("GPU recovery complete");
        Ok(())
    }
}

#[cfg(all(
    not(target_family = "wasm"),
    any(test, feature = "bench-support", feature = "test-support")
))]
struct HeadlessRenderTarget {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}

#[cfg(all(
    not(target_family = "wasm"),
    any(test, feature = "bench-support", feature = "test-support")
))]
impl HeadlessRenderTarget {
    fn size(&self) -> Size<DevicePixels> {
        Size {
            width: DevicePixels(self.texture.width() as i32),
            height: DevicePixels(self.texture.height() as i32),
        }
    }
}

#[cfg(all(
    not(target_family = "wasm"),
    any(test, feature = "bench-support", feature = "test-support")
))]
pub struct WgpuHeadlessRenderer {
    context: WgpuContext,
    core: WgpuRendererCore,
    render_target: Option<HeadlessRenderTarget>,
    observed_error_generation: u64,
}

#[cfg(all(
    not(target_family = "wasm"),
    any(test, feature = "bench-support", feature = "test-support")
))]
impl WgpuHeadlessRenderer {
    pub fn new() -> anyhow::Result<Self> {
        let (context, target_format) = WgpuContext::new_headless()?;
        let atlas = Arc::new(WgpuAtlas::from_context(&context));
        let core = WgpuRendererCore::new(
            &context,
            atlas,
            target_format,
            wgpu::CompositeAlphaMode::Opaque,
        );

        Ok(Self {
            context,
            core,
            render_target: None,
            observed_error_generation: 0,
        })
    }

    fn ensure_render_target(&mut self, size: Size<DevicePixels>) -> anyhow::Result<()> {
        anyhow::ensure!(
            size.width.0 > 0 && size.height.0 > 0,
            "invalid headless render target size: {size:?}"
        );
        anyhow::ensure!(
            size.width.0 as u32 <= self.core.max_texture_size
                && size.height.0 as u32 <= self.core.max_texture_size,
            "headless render target size {size:?} exceeds maximum texture dimension {}",
            self.core.max_texture_size
        );
        if self
            .render_target
            .as_ref()
            .is_some_and(|target| target.size() == size)
        {
            return Ok(());
        }

        let texture = self
            .core
            .resources
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("headless_render_target"),
                size: wgpu::Extent3d {
                    width: size.width.0 as u32,
                    height: size.height.0 as u32,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.core.target_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.render_target = Some(HeadlessRenderTarget { texture, view });
        Ok(())
    }

    fn check_gpu_errors(&mut self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.context.device_lost(),
            "GPU device was lost during headless rendering"
        );
        if let Some(error) = self
            .context
            .errors()
            .observe_error(&mut self.observed_error_generation)
        {
            anyhow::bail!("GPU error during headless rendering: {error}");
        }
        Ok(())
    }

    fn render(&mut self, scene: &Scene, size: Size<DevicePixels>) -> anyhow::Result<()> {
        self.check_gpu_errors()?;
        self.ensure_render_target(size)?;
        let view = self
            .render_target
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Headless render target was not created"))?
            .view
            .clone();
        // Corvene patch: see `FrameOptions`
        let options = FrameOptions {
            damage_scissor: false,
            ..FrameOptions::current()
        };
        self.core.render_frame(
            scene,
            FrameTarget::View(&view),
            size,
            false,
            wgpu::Color::BLACK,
            options,
        )?;
        Ok(())
    }

    /// Copies the current render target back to the CPU. Dimensions come from the
    /// target texture itself, so the copy can never disagree with what was rendered.
    fn read_image(&mut self) -> anyhow::Result<image::RgbaImage> {
        let target = self
            .render_target
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Headless render target was not created"))?;
        let width = target.texture.width();
        let height = target.texture.height();
        let bytes_per_row = width
            .checked_mul(4)
            .ok_or_else(|| anyhow::anyhow!("Headless render target row size overflowed"))?;
        let padded_bytes_per_row = bytes_per_row
            .checked_next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            .ok_or_else(|| anyhow::anyhow!("Headless padded row size overflowed"))?;
        let buffer_size = u64::from(padded_bytes_per_row)
            .checked_mul(u64::from(height))
            .ok_or_else(|| anyhow::anyhow!("Headless readback buffer size overflowed"))?;
        anyhow::ensure!(
            buffer_size <= self.core.resources.device.limits().max_buffer_size,
            "Headless readback buffer size {buffer_size} exceeds maximum buffer size {}",
            self.core.resources.device.limits().max_buffer_size
        );
        let readback_buffer = self
            .core
            .resources
            .device
            .create_buffer(&wgpu::BufferDescriptor {
                label: Some("headless_readback_buffer"),
                size: buffer_size,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
        let mut encoder =
            self.core
                .resources
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("headless_readback_encoder"),
                });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        let submission = self
            .core
            .resources
            .queue
            .submit(std::iter::once(encoder.finish()));
        let (sender, receiver) = std::sync::mpsc::channel();
        readback_buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                if sender.send(result).is_err() {
                    log::error!("Headless readback receiver was dropped before mapping completed");
                }
            });
        self.core
            .resources
            .device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(std::time::Duration::from_secs(30)),
            })
            .map_err(|error| anyhow::anyhow!("Failed to wait for headless rendering: {error}"))?;
        receiver
            .recv_timeout(std::time::Duration::from_secs(30))
            .map_err(|error| anyhow::anyhow!("Failed to receive headless mapping result: {error}"))?
            .map_err(|error| anyhow::anyhow!("Failed to map headless readback buffer: {error}"))?;
        self.check_gpu_errors()?;

        let mapped_data = readback_buffer.slice(..).get_mapped_range();
        let pixel_capacity = usize::try_from(u64::from(bytes_per_row) * u64::from(height))
            .map_err(|_| anyhow::anyhow!("Headless image size exceeds addressable memory"))?;
        let mut pixels = Vec::with_capacity(pixel_capacity);
        for row in mapped_data
            .chunks_exact(padded_bytes_per_row as usize)
            .take(height as usize)
        {
            pixels.extend_from_slice(&row[..bytes_per_row as usize]);
        }
        drop(mapped_data);
        readback_buffer.unmap();

        if self.core.target_format == wgpu::TextureFormat::Bgra8Unorm {
            for pixel in pixels.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
        }

        image::RgbaImage::from_raw(width, height, pixels)
            .ok_or_else(|| anyhow::anyhow!("Failed to create image from headless pixel data"))
    }
}

#[cfg(all(
    not(target_family = "wasm"),
    any(test, feature = "bench-support", feature = "test-support")
))]
impl gpui::PlatformHeadlessRenderer for WgpuHeadlessRenderer {
    fn render_scene_to_image(
        &mut self,
        scene: &Scene,
        size: Size<DevicePixels>,
    ) -> anyhow::Result<image::RgbaImage> {
        self.render(scene, size)?;
        self.read_image()
    }

    fn render_scene(&mut self, scene: &Scene, size: Size<DevicePixels>) -> anyhow::Result<()> {
        self.render(scene, size)
    }

    fn sprite_atlas(&self) -> Arc<dyn gpui::PlatformAtlas> {
        self.core.atlas.clone()
    }
}

fn instance_range(range: Range<usize>) -> Range<u32> {
    range.start as u32..range.end as u32
}

#[cfg(not(target_family = "wasm"))]
fn create_surface(
    instance: &wgpu::Instance,
    raw_window_handle: raw_window_handle::RawWindowHandle,
) -> anyhow::Result<wgpu::Surface<'static>> {
    unsafe {
        instance
            .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                // Fall back to the display handle already provided via InstanceDescriptor::display.
                raw_display_handle: None,
                raw_window_handle,
            })
            .map_err(|e| anyhow::anyhow!("{e}"))
    }
}

struct RenderingParameters {
    path_sample_count: u32,
    gamma_ratios: [f32; 4],
    grayscale_enhanced_contrast: f32,
    subpixel_enhanced_contrast: f32,
}

impl RenderingParameters {
    fn new(adapter: &wgpu::Adapter, surface_format: wgpu::TextureFormat) -> Self {
        use std::env;

        let format_features = adapter.get_texture_format_features(surface_format);
        let path_sample_count = [4, 2, 1]
            .into_iter()
            .find(|&n| format_features.flags.sample_count_supported(n))
            .unwrap_or(1);

        let gamma = env::var("ZED_FONTS_GAMMA")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1.8_f32)
            .clamp(1.0, 2.2);
        let gamma_ratios = get_gamma_correction_ratios(gamma);

        let grayscale_enhanced_contrast = env::var("ZED_FONTS_GRAYSCALE_ENHANCED_CONTRAST")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1.0_f32)
            .max(0.0);

        let subpixel_enhanced_contrast = env::var("ZED_FONTS_SUBPIXEL_ENHANCED_CONTRAST")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.5_f32)
            .max(0.0);

        Self {
            path_sample_count,
            gamma_ratios,
            grayscale_enhanced_contrast,
            subpixel_enhanced_contrast,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{
        BorderStyle, ColorSpace, ContentMask, Corners, Edges, Hsla, MonochromeSprite,
        PolychromeSprite, Quad, Shadow, Size, SubpixelSprite, Underline, linear_color_stop,
        linear_gradient,
    };
    #[cfg(target_os = "linux")]
    use gpui::{DevicePixels, PlatformHeadlessRenderer, Scene};

    #[cfg(target_os = "linux")]
    fn device_size(width: i32, height: i32) -> Size<DevicePixels> {
        Size {
            width: DevicePixels(width),
            height: DevicePixels(height),
        }
    }

    #[cfg(target_os = "linux")]
    fn solid_quad(x: f32, y: f32, width: f32, height: f32, color: Hsla) -> Quad {
        let bounds = Bounds {
            origin: Point {
                x: x.into(),
                y: y.into(),
            },
            size: Size {
                width: width.into(),
                height: height.into(),
            },
        };
        Quad {
            order: 0,
            border_style: BorderStyle::Solid,
            bounds,
            content_mask: ContentMask { bounds },
            background: color.into(),
            border_color: color,
            corner_radii: Corners::default(),
            border_widths: Edges::default(),
        }
    }

    /// Channels are compared with a small tolerance so the assertions hold across
    /// drivers without pinning exact rasterizer output.
    #[cfg(target_os = "linux")]
    fn assert_pixel(image: &image::RgbaImage, x: u32, y: u32, expected: [u8; 4]) {
        let actual = image.get_pixel(x, y).0;
        assert!(
            actual
                .iter()
                .zip(expected)
                .all(|(actual, expected)| actual.abs_diff(expected) <= 3),
            "pixel ({x}, {y}) was {actual:?}, expected {expected:?}"
        );
    }

    #[cfg(target_os = "linux")]
    const RED: [u8; 4] = [255, 0, 0, 255];
    #[cfg(target_os = "linux")]
    const BLUE: [u8; 4] = [0, 0, 255, 255];
    #[cfg(target_os = "linux")]
    const BLACK: [u8; 4] = [0, 0, 0, 255];

    #[cfg(target_os = "linux")]
    #[test]
    fn headless_renderer_draws_quads_with_distinct_colors() -> anyhow::Result<()> {
        let mut renderer = WgpuHeadlessRenderer::new()?;
        let mut scene = Scene::default();
        scene.insert_primitive(solid_quad(0.0, 0.0, 32.0, 32.0, gpui::red()));
        scene.insert_primitive(solid_quad(32.0, 0.0, 32.0, 32.0, gpui::blue()));
        scene.finish();

        let image = renderer.render_scene_to_image(&scene, device_size(64, 32))?;
        assert_eq!(image.dimensions(), (64, 32));
        assert_pixel(&image, 8, 16, RED);
        assert_pixel(&image, 24, 16, RED);
        assert_pixel(&image, 40, 16, BLUE);
        assert_pixel(&image, 56, 16, BLUE);
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn headless_renderer_captures_each_requested_size() -> anyhow::Result<()> {
        let mut renderer = WgpuHeadlessRenderer::new()?;
        let mut scene = Scene::default();
        scene.insert_primitive(solid_quad(2.0, 2.0, 4.0, 3.0, gpui::red()));
        scene.finish();

        // 13 px rows are 52 bytes, forcing readback to strip copy-row padding.
        let image = renderer.render_scene_to_image(&scene, device_size(13, 7))?;
        assert_eq!(image.dimensions(), (13, 7));
        assert_pixel(&image, 0, 0, BLACK);
        assert_pixel(&image, 3, 3, RED);
        assert_pixel(&image, 12, 6, BLACK);

        let image = renderer.render_scene_to_image(&scene, device_size(17, 9))?;
        assert_eq!(image.dimensions(), (17, 9));
        assert_pixel(&image, 3, 3, RED);
        assert_pixel(&image, 16, 8, BLACK);

        let image = renderer.render_scene_to_image(&Scene::default(), device_size(13, 7))?;
        assert_eq!(image.dimensions(), (13, 7));
        assert!(image.pixels().all(|pixel| pixel.0 == BLACK));
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn headless_renderer_reuses_target_for_same_size() -> anyhow::Result<()> {
        let mut renderer = WgpuHeadlessRenderer::new()?;
        let target_texture = |renderer: &WgpuHeadlessRenderer| {
            renderer
                .render_target
                .as_ref()
                .map(|target| target.texture.clone())
        };

        renderer.render_scene(&Scene::default(), device_size(16, 16))?;
        let first = target_texture(&renderer);
        assert!(first.is_some());

        renderer.render_scene(&Scene::default(), device_size(16, 16))?;
        assert_eq!(target_texture(&renderer), first);

        renderer.render_scene(&Scene::default(), device_size(16, 17))?;
        assert_ne!(target_texture(&renderer), first);
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn headless_renderer_rejects_invalid_sizes() -> anyhow::Result<()> {
        let mut renderer = WgpuHeadlessRenderer::new()?;
        let too_large = renderer.core.max_texture_size as i32 + 1;

        for size in [
            device_size(0, 8),
            device_size(8, 0),
            device_size(-1, 8),
            device_size(too_large, 8),
        ] {
            assert!(
                renderer.render_scene(&Scene::default(), size).is_err(),
                "{size:?} should be rejected"
            );
            assert!(
                renderer
                    .render_scene_to_image(&Scene::default(), size)
                    .is_err(),
                "{size:?} should be rejected"
            );
        }

        // Rejection must leave the renderer usable.
        let image = renderer.render_scene_to_image(&Scene::default(), device_size(4, 4))?;
        assert_eq!(image.dimensions(), (4, 4));
        Ok(())
    }

    #[test]
    fn webgl_shader_is_valid_wgsl_without_storage_buffers() {
        assert!(!WEBGL_SHADERS.contains("var<storage"));
        validate_wgsl(WEBGL_SHADERS, naga::valid::Capabilities::empty());
    }

    #[test]
    fn storage_buffer_shader_is_valid_wgsl() {
        validate_wgsl(STORAGE_BUFFER_SHADERS, naga::valid::Capabilities::empty());
    }

    #[test]
    fn subpixel_shader_is_valid_wgsl() {
        validate_wgsl(
            SUBPIXEL_SHADERS,
            naga::valid::Capabilities::DUAL_SOURCE_BLENDING,
        );
    }

    fn validate_wgsl(source: &str, capabilities: naga::valid::Capabilities) {
        let module = naga::front::wgsl::parse_str(source).expect("shader should parse");
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), capabilities)
            .validate(&module)
            .expect("shader should validate");
    }

    #[test]
    fn webgl_record_sizes_match_shader_word_strides() {
        assert_eq!(std::mem::size_of::<Quad>(), 40 * 4);
        assert_eq!(std::mem::size_of::<Shadow>(), 28 * 4);
        assert_eq!(std::mem::size_of::<PathRasterizationVertex>(), 26 * 4);
        assert_eq!(std::mem::size_of::<PathSprite>(), 4 * 4);
        assert_eq!(std::mem::size_of::<Underline>(), 16 * 4);
        assert_eq!(std::mem::size_of::<MonochromeSprite>(), 28 * 4);
        assert_eq!(std::mem::size_of::<SubpixelSprite>(), 28 * 4);
        assert_eq!(std::mem::size_of::<PolychromeSprite>(), 24 * 4);
    }

    #[test]
    fn webgl_quad_layout_matches_fixed_decoder() {
        let quad = Quad {
            order: 41,
            border_style: BorderStyle::Dashed,
            bounds: Bounds {
                origin: Point {
                    x: 1.0.into(),
                    y: 2.0.into(),
                },
                size: Size {
                    width: 3.0.into(),
                    height: 4.0.into(),
                },
            },
            content_mask: ContentMask {
                bounds: Bounds {
                    origin: Point {
                        x: 5.0.into(),
                        y: 6.0.into(),
                    },
                    size: Size {
                        width: 7.0.into(),
                        height: 8.0.into(),
                    },
                },
            },
            background: linear_gradient(
                11.0,
                linear_color_stop(
                    Hsla {
                        h: 12.0,
                        s: 13.0,
                        l: 14.0,
                        a: 15.0,
                    },
                    16.0,
                ),
                linear_color_stop(
                    Hsla {
                        h: 17.0,
                        s: 18.0,
                        l: 19.0,
                        a: 20.0,
                    },
                    21.0,
                ),
            )
            .color_space(ColorSpace::Oklab),
            border_color: Hsla {
                h: 22.0,
                s: 23.0,
                l: 24.0,
                a: 25.0,
            },
            corner_radii: Corners {
                top_left: 26.0.into(),
                top_right: 27.0.into(),
                bottom_right: 28.0.into(),
                bottom_left: 29.0.into(),
            },
            border_widths: Edges {
                top: 30.0.into(),
                right: 31.0.into(),
                bottom: 32.0.into(),
                left: 33.0.into(),
            },
        };

        let bytes = unsafe { WgpuRendererCore::instance_bytes(std::slice::from_ref(&quad)) };
        let words: &[u32] = bytemuck::cast_slice(bytes);
        assert_eq!(
            words,
            &[
                41,
                1,
                1.0_f32.to_bits(),
                2.0_f32.to_bits(),
                3.0_f32.to_bits(),
                4.0_f32.to_bits(),
                5.0_f32.to_bits(),
                6.0_f32.to_bits(),
                7.0_f32.to_bits(),
                8.0_f32.to_bits(),
                1,
                1,
                0,
                0,
                0,
                0,
                11.0_f32.to_bits(),
                12.0_f32.to_bits(),
                13.0_f32.to_bits(),
                14.0_f32.to_bits(),
                15.0_f32.to_bits(),
                16.0_f32.to_bits(),
                17.0_f32.to_bits(),
                18.0_f32.to_bits(),
                19.0_f32.to_bits(),
                20.0_f32.to_bits(),
                21.0_f32.to_bits(),
                0,
                22.0_f32.to_bits(),
                23.0_f32.to_bits(),
                24.0_f32.to_bits(),
                25.0_f32.to_bits(),
                26.0_f32.to_bits(),
                27.0_f32.to_bits(),
                28.0_f32.to_bits(),
                29.0_f32.to_bits(),
                30.0_f32.to_bits(),
                31.0_f32.to_bits(),
                32.0_f32.to_bits(),
                33.0_f32.to_bits(),
            ]
        );
    }
}

/// Corvene patch: copy `texture` (in `core.target_format`) back to the CPU
/// as RGBA, as `WgpuHeadlessRenderer::read_image` does.
#[cfg(all(not(target_family = "wasm"), any(test, feature = "test-support")))]
fn read_texture(
    core: &WgpuRendererCore,
    texture: &wgpu::Texture,
) -> anyhow::Result<image::RgbaImage> {
    let device = &core.resources.device;
    let (width, height) = (texture.width(), texture.height());
    let bytes_per_row = width * 4;
    let padded_bytes_per_row = bytes_per_row.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("offscreen_readback_buffer"),
        size: u64::from(padded_bytes_per_row) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("offscreen_readback_encoder"),
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    let submission = core
        .resources
        .queue
        .submit(std::iter::once(encoder.finish()));
    let (sender, receiver) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
    device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(std::time::Duration::from_secs(30)),
        })
        .map_err(|error| anyhow::anyhow!("Failed to wait for offscreen rendering: {error}"))?;
    receiver
        .recv_timeout(std::time::Duration::from_secs(30))
        .map_err(|error| anyhow::anyhow!("Failed to receive the readback mapping: {error}"))?
        .map_err(|error| anyhow::anyhow!("Failed to map the readback buffer: {error}"))?;
    let mapped = buffer.slice(..).get_mapped_range();
    let mut pixels = Vec::with_capacity((bytes_per_row * height) as usize);
    for row in mapped
        .chunks_exact(padded_bytes_per_row as usize)
        .take(height as usize)
    {
        pixels.extend_from_slice(&row[..bytes_per_row as usize]);
    }
    drop(mapped);
    buffer.unmap();
    if matches!(
        core.target_format,
        wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
    ) {
        for pixel in pixels.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
    }
    image::RgbaImage::from_raw(width, height, pixels)
        .ok_or_else(|| anyhow::anyhow!("Failed to create an image from the readback"))
}

/// Corvene patch (Android): the quads of a scene with those emptied that a
/// later opaque quad covers completely, or `None` when there is none.
///
/// GPUI paints back to front: the window's background, each panel's over it,
/// each row's over that. On a desktop GPU the hidden layers cost nothing
/// worth saving; a phone's fills the whole window several times a frame for
/// pixels nobody sees. An emptied quad keeps its place (batches refer to
/// quads by index) and rasterizes to nothing.
#[cfg(target_os = "android")]
fn without_hidden_quads(quads: &[gpui::Quad]) -> Option<Vec<gpui::Quad>> {
    // enough to hold the panels of a window; small quads hide little
    const OCCLUDERS: usize = 24;
    let visible = |quad: &gpui::Quad| quad.bounds.intersect(&quad.content_mask.bounds);
    let area = |bounds: &Bounds<ScaledPixels>| bounds.size.width.0 * bounds.size.height.0;
    let square = |quad: &gpui::Quad| {
        let radii = &quad.corner_radii;
        radii.top_left.0 == 0.0
            && radii.top_right.0 == 0.0
            && radii.bottom_left.0 == 0.0
            && radii.bottom_right.0 == 0.0
    };

    let mut occluders: Vec<(usize, Bounds<ScaledPixels>, f32)> = quads
        .iter()
        .enumerate()
        .filter(|(_, quad)| quad.background.is_opaque_solid() && square(quad))
        .map(|(index, quad)| {
            let bounds = visible(quad);
            (index, bounds, area(&bounds))
        })
        .filter(|(_, _, area)| *area > 0.0)
        .collect();
    if occluders.is_empty() {
        return None;
    }
    occluders.sort_by(|a, b| b.2.total_cmp(&a.2));
    occluders.truncate(OCCLUDERS);

    // rectangles as [left, top, right, bottom]
    let rect = |bounds: &Bounds<ScaledPixels>| {
        [
            bounds.origin.x.0,
            bounds.origin.y.0,
            bounds.origin.x.0 + bounds.size.width.0,
            bounds.origin.y.0 + bounds.size.height.0,
        ]
    };
    let covers: Vec<(usize, [f32; 4])> = occluders
        .iter()
        .map(|(index, bounds, _)| (*index, rect(bounds)))
        .collect();
    // What is left of `bounds` once every later occluder is taken away:
    // nothing, when the panels over it cover it between them (a window's
    // background under a sidebar and a content pane).
    let hidden_by_later = |index: usize, quad: &gpui::Quad, bounds: &Bounds<ScaledPixels>| {
        let mut left = vec![rect(bounds)];
        for (occluder, cover) in &covers {
            if *occluder <= index || quads[*occluder].order < quad.order {
                continue;
            }
            let mut next = Vec::with_capacity(left.len());
            for piece in left {
                let [l, t, r, b] = piece;
                let [cl, ct, cr, cb] = *cover;
                if cl >= r || cr <= l || ct >= b || cb <= t {
                    next.push(piece);
                    continue;
                }
                // the parts of the piece outside the cover: above, below,
                // left and right of the overlap
                if ct > t {
                    next.push([l, t, r, ct]);
                }
                if cb < b {
                    next.push([l, cb, r, b]);
                }
                let (top, bottom) = (t.max(ct), b.min(cb));
                if cl > l {
                    next.push([l, top, cl, bottom]);
                }
                if cr < r {
                    next.push([cr, top, r, bottom]);
                }
            }
            left = next;
            if left.is_empty() {
                return true;
            }
            // a pathological scene: give up rather than splinter further
            if left.len() > 16 {
                return false;
            }
        }
        false
    };
    let mut result: Option<Vec<gpui::Quad>> = None;
    for (index, quad) in quads.iter().enumerate() {
        let bounds = visible(quad);
        if area(&bounds) <= 0.0 {
            continue;
        }
        if hidden_by_later(index, quad, &bounds) {
            let emptied = result.get_or_insert_with(|| quads.to_vec());
            emptied[index].bounds.size = Default::default();
        }
    }
    result
}

/// Corvene patch: where a frame is drawn.
enum FrameTarget<'a> {
    /// Straight into this view, cleared first.
    View(&'a wgpu::TextureView),
    /// Into the retained frame, only where the scene changed, then copied
    /// to this texture (see `set_damage_scissor`).
    Retained(&'a wgpu::Texture),
}

/// Corvene patch: the main pass of a frame, with the opaque depth pass's
/// depth buffer when it is given (see `set_opaque_depth_pass`).
fn begin_main_pass<'e>(
    encoder: &'e mut wgpu::CommandEncoder,
    label: &str,
    view: &wgpu::TextureView,
    load: wgpu::LoadOp<wgpu::Color>,
    depth: Option<(&wgpu::TextureView, wgpu::LoadOp<f32>, wgpu::StoreOp)>,
) -> wgpu::RenderPass<'e> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            resolve_target: None,
            ops: wgpu::Operations {
                load,
                store: wgpu::StoreOp::Store,
            },
            depth_slice: None,
        })],
        depth_stencil_attachment: depth.map(|(view, load, store)| {
            wgpu::RenderPassDepthStencilAttachment {
                view,
                depth_ops: Some(wgpu::Operations { load, store }),
                stencil_ops: None,
            }
        }),
        ..Default::default()
    })
}

// Corvene patch: `write_ordered_binding` writes a primitive's depth over its
// first four bytes, which must be its `order`.
const _: () = {
    assert!(std::mem::offset_of!(Quad, order) == 0);
    assert!(std::mem::offset_of!(Shadow, order) == 0);
    assert!(std::mem::offset_of!(Underline, order) == 0);
    assert!(std::mem::offset_of!(MonochromeSprite, order) == 0);
    assert!(std::mem::offset_of!(SubpixelSprite, order) == 0);
    assert!(std::mem::offset_of!(PolychromeSprite, order) == 0);
};

/// Corvene patch: above this many primitives in a frame the depth values
/// (1 / (count + 1) apart) would come too close for 24-bit depth buffers,
/// and the frame is drawn without the opaque depth pass.
const MAX_DEPTH_PRIMITIVES: usize = 1 << 22;

/// Corvene patch: one frame's data for the opaque depth pass (see
/// `set_opaque_depth_pass`), kept between frames to reuse its memory.
#[derive(Default)]
struct DepthScratch {
    /// The depth of each primitive, by kind and index in the scene: the
    /// primitives are numbered in paint order, and later ones are nearer
    /// (smaller, the test passes when less), all strictly between 0 and 1.
    shadows: Vec<f32>,
    quads: Vec<f32>,
    underlines: Vec<f32>,
    monochrome_sprites: Vec<f32>,
    subpixel_sprites: Vec<f32>,
    polychrome_sprites: Vec<f32>,
    /// One depth for each batch of paths (they are drawn together from the
    /// path texture), in paint order.
    path_batches: Vec<f32>,
    /// The opaque part of each quad that has one, front to back, with its
    /// depth as its `order`.
    opaque_quads: Vec<Quad>,
}

impl DepthScratch {
    /// Fills these for `scene`, whose quads are drawn as `quads` (the
    /// scene's, or as many with some emptied). False when the frame is
    /// drawn as well without: no opaque quad, too many primitives, or a
    /// batch that does not fit its primitives.
    fn fill(&mut self, scene: &Scene, quads: &[Quad]) -> bool {
        let count = scene.shadows.len()
            + quads.len()
            + scene.paths.len()
            + scene.underlines.len()
            + scene.monochrome_sprites.len()
            + scene.subpixel_sprites.len()
            + scene.polychrome_sprites.len();
        if count >= MAX_DEPTH_PRIMITIVES {
            return false;
        }
        let step = 1.0 / (count + 1) as f64;
        let depth_of = |rank: usize| (1.0 - (rank + 1) as f64 * step) as f32;
        let reset = |depths: &mut Vec<f32>, len: usize| {
            depths.clear();
            depths.resize(len, 1.0);
        };
        reset(&mut self.shadows, scene.shadows.len());
        reset(&mut self.quads, quads.len());
        reset(&mut self.underlines, scene.underlines.len());
        reset(&mut self.monochrome_sprites, scene.monochrome_sprites.len());
        reset(&mut self.subpixel_sprites, scene.subpixel_sprites.len());
        reset(&mut self.polychrome_sprites, scene.polychrome_sprites.len());
        self.path_batches.clear();
        self.opaque_quads.clear();

        let mut rank = 0;
        for batch in scene.batches() {
            let (depths, range) = match batch {
                PrimitiveBatch::Shadows(range) => (&mut self.shadows, range),
                PrimitiveBatch::Quads(range) => (&mut self.quads, range),
                PrimitiveBatch::Underlines(range) => (&mut self.underlines, range),
                PrimitiveBatch::MonochromeSprites { range, .. } => {
                    (&mut self.monochrome_sprites, range)
                }
                PrimitiveBatch::SubpixelSprites { range, .. } => {
                    (&mut self.subpixel_sprites, range)
                }
                PrimitiveBatch::PolychromeSprites { range, .. } => {
                    (&mut self.polychrome_sprites, range)
                }
                PrimitiveBatch::Paths(range) => {
                    self.path_batches.push(depth_of(rank));
                    rank += range.len();
                    continue;
                }
                PrimitiveBatch::Surfaces(_) => continue,
            };
            let Some(depths) = depths.get_mut(range) else {
                return false;
            };
            for depth in depths {
                *depth = depth_of(rank);
                rank += 1;
            }
        }

        for (quad, depth) in quads.iter().zip(&self.quads).rev() {
            if let Some(bounds) = opaque_interior(quad) {
                self.opaque_quads.push(Quad {
                    order: depth.to_bits(),
                    border_style: BorderStyle::Solid,
                    bounds,
                    content_mask: ContentMask { bounds },
                    background: quad.background,
                    border_color: quad.border_color,
                    corner_radii: Corners::default(),
                    border_widths: Edges::default(),
                });
            }
        }
        !self.opaque_quads.is_empty()
    }
}

/// Corvene patch: the whole pixels of `quad` that `fs_quad` paints with its
/// background colour at alpha 1 and nothing else, if any: a solid opaque
/// background, inside the borders and the rounded corners (the shader's
/// `QuadVarying.interior`) and inside the content mask, shrunk to whole
/// pixels so that every pixel centre is half a pixel inside.
fn opaque_interior(quad: &Quad) -> Option<Bounds<ScaledPixels>> {
    if !quad.background.is_opaque_solid() {
        return None;
    }
    let widths = &quad.border_widths;
    let radii = &quad.corner_radii;
    let edges = [
        widths.top.0,
        widths.right.0,
        widths.bottom.0,
        widths.left.0,
        radii.top_left.0,
        radii.top_right.0,
        radii.bottom_right.0,
        radii.bottom_left.0,
    ];
    // the shader's interior is only sure for these
    if !edges.iter().all(|edge| *edge >= 0.0 && edge.is_finite()) {
        return None;
    }
    let edge = edges.into_iter().fold(0.0, f32::max);
    let inset = if edge == 0.0 { 0.0 } else { edge + 1.0 };
    let [left, top, right, bottom] = intersect_rects(
        dilate_rect(rect_of(&quad.bounds), -inset),
        rect_of(&quad.content_mask.bounds),
    );
    let (left, top, right, bottom) = (left.ceil(), top.ceil(), right.floor(), bottom.floor());
    if !(right > left && bottom > top) {
        return None;
    }
    Some(Bounds {
        origin: Point {
            x: ScaledPixels(left),
            y: ScaledPixels(top),
        },
        size: Size {
            width: ScaledPixels(right - left),
            height: ScaledPixels(bottom - top),
        },
    })
}

/// Corvene patch: `bounds` as [left, top, right, bottom].
fn rect_of(bounds: &Bounds<ScaledPixels>) -> [f32; 4] {
    let (x, y) = (bounds.origin.x.0, bounds.origin.y.0);
    [x, y, x + bounds.size.width.0, y + bounds.size.height.0]
}

fn intersect_rects(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        a[0].max(b[0]),
        a[1].max(b[1]),
        a[2].min(b[2]),
        a[3].min(b[3]),
    ]
}

fn dilate_rect(rect: [f32; 4], by: f32) -> [f32; 4] {
    [rect[0] - by, rect[1] - by, rect[2] + by, rect[3] + by]
}

/// Corvene patch: the damage scissor's helper pipelines (see
/// `set_damage_scissor` and `shaders_damage.wgsl`).
struct DamagePipelines {
    /// Fills the scissor rectangle with the blend constant.
    clear: wgpu::RenderPipeline,
    /// The same, in a pass with the opaque depth pass's depth buffer.
    clear_with_depth: wgpu::RenderPipeline,
    blit_layout: wgpu::BindGroupLayout,
    /// Copies the retained frame to a target that cannot be copied to.
    blit: wgpu::RenderPipeline,
}

impl DamagePipelines {
    fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
    ) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("damage_shaders"),
            source: wgpu::ShaderSource::Wgsl(DAMAGE_SHADERS.into()),
        });
        let clear_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("damage_clear_layout"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });
        let blit_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("retained_frame_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let blit_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("damage_blit_layout"),
            bind_group_layouts: &[Some(&blit_layout)],
            immediate_size: 0,
        });
        let pipeline = |label: &str,
                        layout: &wgpu::PipelineLayout,
                        fs_entry: &str,
                        blend: Option<wgpu::BlendState>,
                        depth_stencil: Option<wgpu::DepthStencilState>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs_fullscreen"),
                    buffers: &[],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(fs_entry),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let constant = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::Constant,
            dst_factor: wgpu::BlendFactor::Zero,
            operation: wgpu::BlendOperation::Add,
        };
        let clear_blend = Some(wgpu::BlendState {
            color: constant,
            alpha: constant,
        });
        Self {
            clear: pipeline("damage_clear", &clear_layout, "fs_clear", clear_blend, None),
            clear_with_depth: pipeline(
                "damage_clear_with_depth",
                &clear_layout,
                "fs_clear",
                clear_blend,
                Some(wgpu::DepthStencilState {
                    format: depth_format,
                    depth_write_enabled: Some(false),
                    depth_compare: Some(wgpu::CompareFunction::Always),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
            ),
            blit: pipeline("damage_blit", &blit_pipeline_layout, "fs_blit", None, None),
            blit_layout,
        }
    }
}

/// Corvene patch: the retained frame of the damage scissor (see
/// `set_damage_scissor`).
struct DamageState {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    /// What `texture` shows, `None` when that is not known.
    shown: Option<DamageSnapshot>,
}

/// Corvene patch: what else than the scene a frame's pixels depend on; a
/// frame whose key differs from the last one's is drawn whole.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct DamageKey {
    size: Size<DevicePixels>,
    clear_color: wgpu::Color,
    premultiplied_alpha: bool,
    is_bgr: bool,
    /// changes when a tile of the sprite atlas changes, which a sprite that
    /// is the same in the scene would show
    atlas_version: u64,
    opaque_depth_pass: bool,
}

/// Corvene patch: what to draw again of the retained frame.
#[derive(Clone, Copy, Debug, PartialEq)]
enum DamagePlan {
    /// Everything (cleared first).
    Full,
    /// Only this rectangle of whole pixels, as x, y, width and height.
    Partial([u32; 4]),
    /// Nothing: the frame is the same as the last one.
    Unchanged,
}

/// Corvene patch: the scene the retained frame shows, to compare the next
/// one with (copies of its primitives, without the path vertices' extras).
#[derive(Default)]
struct DamageSnapshot {
    key: DamageKey,
    shadows: Vec<Shadow>,
    quads: Vec<Quad>,
    paths: Vec<PathSnapshot>,
    underlines: Vec<Underline>,
    monochrome_sprites: Vec<MonochromeSprite>,
    subpixel_sprites: Vec<SubpixelSprite>,
    polychrome_sprites: Vec<PolychromeSprite>,
}

/// Corvene patch: the frame drawn again whole when the changes cover more
/// than this part of it (loading the retained frame and clearing a
/// rectangle cost about what shading the rest would save).
const MAX_DAMAGE_FRACTION: f32 = 0.75;

impl DamageSnapshot {
    fn fill(&mut self, scene: &Scene, key: DamageKey) {
        self.key = key;
        self.shadows.clear();
        self.shadows.extend_from_slice(&scene.shadows);
        self.quads.clear();
        self.quads.extend_from_slice(&scene.quads);
        self.paths.clear();
        self.paths.extend(scene.paths.iter().map(PathSnapshot::new));
        self.underlines.clear();
        self.underlines.extend_from_slice(&scene.underlines);
        self.monochrome_sprites.clear();
        self.monochrome_sprites
            .extend_from_slice(&scene.monochrome_sprites);
        self.subpixel_sprites.clear();
        self.subpixel_sprites
            .extend_from_slice(&scene.subpixel_sprites);
        self.polychrome_sprites.clear();
        self.polychrome_sprites
            .extend_from_slice(&scene.polychrome_sprites);
    }

    /// What of a frame of `size` showing this to draw again for `scene`.
    ///
    /// A pixel's colour is what the primitives over it paint, in paint
    /// order: by `order`, then by kind, then by place in their kind's list
    /// (the lists are sorted by order, and sprites by their tile within an
    /// order). Each kind's list is compared with the last frame's group by
    /// group of the same sort key: what the two groups start and end with
    /// alike is unchanged, and the rest, the last frame's and this one's,
    /// is damage. Outside the damage, every pixel then has the same
    /// primitives (byte for byte, `order` included) over it in the same
    /// order as before, so it is the same.
    fn plan(&self, scene: &Scene, size: Size<DevicePixels>) -> DamagePlan {
        let mut damage = Damage::default();
        diff_records(&self.shadows, &scene.shadows, &mut damage);
        diff_records(&self.quads, &scene.quads, &mut damage);
        diff_records(&self.underlines, &scene.underlines, &mut damage);
        diff_records(
            &self.monochrome_sprites,
            &scene.monochrome_sprites,
            &mut damage,
        );
        diff_records(&self.subpixel_sprites, &scene.subpixel_sprites, &mut damage);
        diff_records(
            &self.polychrome_sprites,
            &scene.polychrome_sprites,
            &mut damage,
        );
        diff_primitives(&self.paths, &scene.paths, PathSnapshot::same, &mut damage);
        damage.plan(size)
    }
}

/// Corvene patch: the union of the changed primitives' footprints.
#[derive(Default)]
struct Damage {
    /// [left, top, right, bottom]
    rect: Option<[f32; 4]>,
    /// a footprint was not finite
    unbounded: bool,
}

impl Damage {
    fn add(&mut self, footprint: [f32; 4]) {
        if !footprint.iter().all(|edge| edge.is_finite()) {
            self.unbounded = true;
            return;
        }
        let [left, top, right, bottom] = footprint;
        if right <= left || bottom <= top {
            return;
        }
        self.rect = Some(match self.rect {
            Some(rect) => [
                rect[0].min(left),
                rect[1].min(top),
                rect[2].max(right),
                rect[3].max(bottom),
            ],
            None => footprint,
        });
    }

    fn plan(&self, size: Size<DevicePixels>) -> DamagePlan {
        if self.unbounded {
            return DamagePlan::Full;
        }
        let Some(rect) = self.rect else {
            return DamagePlan::Unchanged;
        };
        let (width, height) = (size.width.0.max(0) as f32, size.height.0.max(0) as f32);
        // a pixel more on each side, for antialiasing and rounding
        let [left, top, right, bottom] = dilate_rect(rect, 1.0);
        let left = left.floor().clamp(0.0, width);
        let top = top.floor().clamp(0.0, height);
        let right = right.ceil().clamp(0.0, width);
        let bottom = bottom.ceil().clamp(0.0, height);
        if right <= left || bottom <= top {
            return DamagePlan::Unchanged;
        }
        if (right - left) * (bottom - top) > MAX_DAMAGE_FRACTION * width * height {
            return DamagePlan::Full;
        }
        DamagePlan::Partial([
            left as u32,
            top as u32,
            (right - left) as u32,
            (bottom - top) as u32,
        ])
    }
}

/// Corvene patch: a primitive as the damage scissor compares it.
trait DamageRecord {
    /// What the scene sorts its kind by.
    fn sort_key(&self) -> (u32, u32);
    /// A rectangle (left, top, right, bottom) that holds every pixel it can
    /// change.
    fn footprint(&self) -> [f32; 4];
}

fn masked(rect: [f32; 4], mask: &ContentMask<ScaledPixels>) -> [f32; 4] {
    intersect_rects(rect, rect_of(&mask.bounds))
}

/// The footprint of a sprite drawn with `transformation` (as the shaders
/// apply it: the rows of `rotation_scale` times the position, plus the
/// translation).
fn transformed_footprint(
    bounds: &Bounds<ScaledPixels>,
    transformation: &TransformationMatrix,
    mask: &ContentMask<ScaledPixels>,
) -> [f32; 4] {
    if *transformation == TransformationMatrix::unit() {
        return masked(rect_of(bounds), mask);
    }
    let [left, top, right, bottom] = rect_of(bounds);
    let [[a, b], [c, d]] = transformation.rotation_scale;
    let [tx, ty] = transformation.translation;
    let mut rect = [
        f32::INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
    ];
    for (x, y) in [(left, top), (right, top), (left, bottom), (right, bottom)] {
        let (x, y) = (a * x + b * y + tx, c * x + d * y + ty);
        rect = [
            rect[0].min(x),
            rect[1].min(y),
            rect[2].max(x),
            rect[3].max(y),
        ];
    }
    masked(rect, mask)
}

impl DamageRecord for Shadow {
    fn sort_key(&self) -> (u32, u32) {
        (self.order, 0)
    }

    fn footprint(&self) -> [f32; 4] {
        // the geometry of `vs_shadow`
        let geometry = if self.inset != 0 {
            rect_of(&self.element_bounds)
        } else {
            dilate_rect(rect_of(&self.bounds), 3.0 * self.blur_radius.0.abs())
        };
        masked(geometry, &self.content_mask)
    }
}

impl DamageRecord for Quad {
    fn sort_key(&self) -> (u32, u32) {
        (self.order, 0)
    }

    fn footprint(&self) -> [f32; 4] {
        masked(rect_of(&self.bounds), &self.content_mask)
    }
}

impl DamageRecord for Underline {
    fn sort_key(&self) -> (u32, u32) {
        (self.order, 0)
    }

    fn footprint(&self) -> [f32; 4] {
        masked(rect_of(&self.bounds), &self.content_mask)
    }
}

impl DamageRecord for MonochromeSprite {
    fn sort_key(&self) -> (u32, u32) {
        (self.order, self.tile.tile_id.0)
    }

    fn footprint(&self) -> [f32; 4] {
        transformed_footprint(&self.bounds, &self.transformation, &self.content_mask)
    }
}

impl DamageRecord for SubpixelSprite {
    fn sort_key(&self) -> (u32, u32) {
        (self.order, self.tile.tile_id.0)
    }

    fn footprint(&self) -> [f32; 4] {
        transformed_footprint(&self.bounds, &self.transformation, &self.content_mask)
    }
}

impl DamageRecord for PolychromeSprite {
    fn sort_key(&self) -> (u32, u32) {
        (self.order, self.tile.tile_id.0)
    }

    fn footprint(&self) -> [f32; 4] {
        masked(rect_of(&self.bounds), &self.content_mask)
    }
}

impl DamageRecord for Path<ScaledPixels> {
    fn sort_key(&self) -> (u32, u32) {
        (self.order, 0)
    }

    fn footprint(&self) -> [f32; 4] {
        rect_of(&self.clipped_bounds())
    }
}

/// Corvene patch: what of a path its drawing reads.
struct PathSnapshot {
    order: u32,
    clipped_bounds: Bounds<ScaledPixels>,
    color: Background,
    vertices: Vec<(Point<ScaledPixels>, Point<f32>)>,
}

impl PathSnapshot {
    fn new(path: &Path<ScaledPixels>) -> Self {
        Self {
            order: path.order,
            clipped_bounds: path.clipped_bounds(),
            color: path.color,
            vertices: path
                .vertices
                .iter()
                .map(|vertex| (vertex.xy_position, vertex.st_position))
                .collect(),
        }
    }

    fn same(&self, path: &Path<ScaledPixels>) -> bool {
        self.order == path.order
            && self.clipped_bounds == path.clipped_bounds()
            && self.color == path.color
            && self.vertices.len() == path.vertices.len()
            && self
                .vertices
                .iter()
                .zip(&path.vertices)
                .all(|((xy, st), vertex)| *xy == vertex.xy_position && *st == vertex.st_position)
    }
}

impl DamageRecord for PathSnapshot {
    fn sort_key(&self) -> (u32, u32) {
        (self.order, 0)
    }

    fn footprint(&self) -> [f32; 4] {
        rect_of(&self.clipped_bounds)
    }
}

/// Corvene patch: `diff_primitives` for the primitives that are uploaded
/// as they are, compared byte for byte (the whole list first: most do not
/// change from one frame to the next).
fn diff_records<T: DamageRecord>(old: &[T], new: &[T], damage: &mut Damage) {
    fn bytes<T>(records: &[T]) -> &[u8] {
        unsafe { WgpuRendererCore::instance_bytes(records) }
    }
    if bytes(old) == bytes(new) {
        return;
    }
    diff_primitives(
        old,
        new,
        |old, new| bytes(std::slice::from_ref(old)) == bytes(std::slice::from_ref(new)),
        damage,
    );
}

/// Corvene patch: adds to `damage` the footprints of what differs between
/// two lists of a kind of primitive sorted by `sort_key` (see
/// `DamageSnapshot::plan`).
fn diff_primitives<O: DamageRecord, N: DamageRecord>(
    old: &[O],
    new: &[N],
    same: impl Fn(&O, &N) -> bool,
    damage: &mut Damage,
) {
    fn group_end<T: DamageRecord>(records: &[T], start: usize) -> usize {
        let key = records[start].sort_key();
        records[start..]
            .iter()
            .position(|record| record.sort_key() != key)
            .map_or(records.len(), |length| start + length)
    }

    let (mut old_start, mut new_start) = (0, 0);
    while old_start < old.len() || new_start < new.len() {
        let old_key = old.get(old_start).map(DamageRecord::sort_key);
        let new_key = new.get(new_start).map(DamageRecord::sort_key);
        match (old_key, new_key) {
            (Some(old_key), Some(new_key)) if old_key == new_key => {
                let old_end = group_end(old, old_start);
                let new_end = group_end(new, new_start);
                let (old_group, new_group) = (&old[old_start..old_end], &new[new_start..new_end]);
                let prefix = old_group
                    .iter()
                    .zip(new_group)
                    .take_while(|(old, new)| same(old, new))
                    .count();
                let suffix = old_group[prefix..]
                    .iter()
                    .rev()
                    .zip(new_group[prefix..].iter().rev())
                    .take_while(|(old, new)| same(old, new))
                    .count();
                for record in &old_group[prefix..old_group.len() - suffix] {
                    damage.add(record.footprint());
                }
                for record in &new_group[prefix..new_group.len() - suffix] {
                    damage.add(record.footprint());
                }
                old_start = old_end;
                new_start = new_end;
            }
            (Some(old_key), new_key) if new_key.is_none_or(|new_key| old_key < new_key) => {
                let old_end = group_end(old, old_start);
                for record in &old[old_start..old_end] {
                    damage.add(record.footprint());
                }
                old_start = old_end;
            }
            _ => {
                let new_end = group_end(new, new_start);
                for record in &new[new_start..new_end] {
                    damage.add(record.footprint());
                }
                new_start = new_end;
            }
        }
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
#[path = "wgpu_renderer_corvene_tests.rs"]
mod corvene_tests;

//! Corvene patch: tests of the opaque depth pass (`set_opaque_depth_pass`)
//! and the damage scissor (`set_damage_scissor`): every scene is drawn with
//! them and without, and the images must match. They need a GPU adapter
//! (Metal on macOS, Vulkan or GL elsewhere) and fail, saying so, without.

use super::*;
use gpui::{
    AtlasKey, AtlasTile, BorderStyle, ContentMask, Corners, Edges, FontId, GlyphId, Hsla, ImageId,
    MonochromeSprite, PaddedBool32, Pixels, PlatformAtlas, PolychromeSprite, Quad, Radians,
    RenderGlyphParams, RenderImageParams, RenderSvgParams, Shadow, SharedString, SubpixelSprite,
    TransformationMatrix, Underline, checkerboard, linear_color_stop, linear_gradient,
    pattern_slash, px,
};
use std::borrow::Cow;
use std::time::Instant;

const OFF: FrameOptions = FrameOptions {
    opaque_depth_pass: false,
    damage_scissor: false,
};
const DEPTH: FrameOptions = FrameOptions {
    opaque_depth_pass: true,
    damage_scissor: false,
};
const DAMAGE: FrameOptions = FrameOptions {
    opaque_depth_pass: false,
    damage_scissor: true,
};
const BOTH: FrameOptions = FrameOptions {
    opaque_depth_pass: true,
    damage_scissor: true,
};

fn size(width: i32, height: i32) -> Size<DevicePixels> {
    Size {
        width: DevicePixels(width),
        height: DevicePixels(height),
    }
}

fn bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<ScaledPixels> {
    Bounds {
        origin: Point {
            x: ScaledPixels(x),
            y: ScaledPixels(y),
        },
        size: Size {
            width: ScaledPixels(width),
            height: ScaledPixels(height),
        },
    }
}

fn hsla(h: f32, s: f32, l: f32, a: f32) -> Hsla {
    Hsla { h, s, l, a }
}

fn quad(bounds: Bounds<ScaledPixels>, color: Hsla) -> Quad {
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

fn masked_quad(bounds: Bounds<ScaledPixels>, mask: Bounds<ScaledPixels>, color: Hsla) -> Quad {
    Quad {
        content_mask: ContentMask { bounds: mask },
        ..quad(bounds, color)
    }
}

fn bordered_quad(
    bounds: Bounds<ScaledPixels>,
    color: Hsla,
    border: f32,
    border_color: Hsla,
    radius: f32,
) -> Quad {
    Quad {
        border_color,
        border_widths: Edges {
            top: ScaledPixels(border),
            right: ScaledPixels(border),
            bottom: ScaledPixels(border),
            left: ScaledPixels(border),
        },
        corner_radii: Corners {
            top_left: ScaledPixels(radius),
            top_right: ScaledPixels(radius),
            bottom_right: ScaledPixels(radius),
            bottom_left: ScaledPixels(radius),
        },
        ..quad(bounds, color)
    }
}

/// A headless renderer, or an error that says no GPU adapter was found.
fn renderer() -> anyhow::Result<WgpuHeadlessRenderer> {
    WgpuHeadlessRenderer::new()
        .context("no GPU adapter for the renderer tests (Metal on macOS, Vulkan or GL elsewhere)")
}

/// The atlas tiles the sprites of `kitchen_sink` use.
struct Tiles {
    mono: AtlasTile,
    subpixel: AtlasTile,
    poly: AtlasTile,
}

fn insert_tile(
    atlas: &WgpuAtlas,
    key: AtlasKey,
    tile_size: i32,
    bytes_per_pixel: usize,
) -> anyhow::Result<AtlasTile> {
    let side = tile_size as usize;
    let bytes: Vec<u8> = (0..side * side * bytes_per_pixel)
        .map(|index| {
            let pixel = index / bytes_per_pixel;
            let (x, y) = (pixel % side, pixel / side);
            let channel = index % bytes_per_pixel;
            ((x * 37 + y * 23 + channel * 71) % 256) as u8
        })
        .collect();
    let mut build = || -> anyhow::Result<Option<(Size<DevicePixels>, Cow<'static, [u8]>)>> {
        Ok(Some((
            size(tile_size, tile_size),
            Cow::Owned(bytes.clone()),
        )))
    };
    atlas
        .get_or_insert_with(key, &mut build)?
        .context("the atlas refused the tile")
}

fn tiles(renderer: &WgpuHeadlessRenderer) -> anyhow::Result<Tiles> {
    let atlas = &renderer.core.atlas;
    Ok(Tiles {
        mono: insert_tile(
            atlas,
            AtlasKey::Svg(RenderSvgParams {
                path: SharedString::from("corvene-test"),
                size: size(16, 16),
            }),
            16,
            1,
        )?,
        subpixel: insert_tile(
            atlas,
            AtlasKey::Glyph(RenderGlyphParams {
                font_id: FontId(7),
                glyph_id: GlyphId(11),
                font_size: px(12.0),
                subpixel_variant: Point::default(),
                scale_factor: 1.0,
                is_emoji: false,
                subpixel_rendering: true,
                dilation: 0,
            }),
            12,
            4,
        )?,
        poly: insert_tile(
            atlas,
            AtlasKey::Image(RenderImageParams {
                image_id: ImageId(3),
                frame_index: 0,
            }),
            20,
            4,
        )?,
    })
}

fn mono_sprite(
    tile: AtlasTile,
    bounds: Bounds<ScaledPixels>,
    color: Hsla,
    transformation: TransformationMatrix,
) -> MonochromeSprite {
    MonochromeSprite {
        order: 0,
        pad: 0,
        bounds,
        content_mask: ContentMask { bounds },
        color,
        tile,
        transformation,
    }
}

fn path(points: &[(f32, f32)], color: Hsla, mask: Bounds<ScaledPixels>) -> Path<ScaledPixels> {
    let point = |(x, y): (f32, f32)| Point { x: px(x), y: px(y) };
    let mut path = Path::<Pixels>::new(point(points[0]));
    for pair in points[1..].chunks(2) {
        match pair {
            [to, control] => path.curve_to(point(*to), point(*control)),
            [to] => path.line_to(point(*to)),
            _ => {}
        }
    }
    path.line_to(point(points[0]));
    path.color = color.into();
    path.content_mask = ContentMask {
        bounds: Bounds {
            origin: Point {
                x: px(mask.origin.x.0),
                y: px(mask.origin.y.0),
            },
            size: Size {
                width: px(mask.size.width.0),
                height: px(mask.size.height.0),
            },
        },
    };
    path.scale(1.0)
}

/// Every kind of primitive, over, under and between opaque quads: borders,
/// rounded corners, content masks, fractional bounds, layers (which give
/// overlapping primitives one order), translucency and gradients.
fn kitchen_sink(tiles: &Tiles, extra: impl FnOnce(&mut Scene)) -> Scene {
    let white = hsla(0.0, 0.0, 1.0, 1.0);
    let panel = hsla(0.6, 0.3, 0.2, 1.0);
    let row = hsla(0.1, 0.6, 0.5, 1.0);
    let translucent = hsla(0.9, 0.8, 0.5, 0.45);
    let text = hsla(0.0, 0.0, 0.05, 1.0);
    let mut scene = Scene::default();

    // the window and two panels over it
    scene.insert_primitive(quad(bounds(0.0, 0.0, 320.0, 240.0), white));
    scene.insert_primitive(bordered_quad(
        bounds(4.0, 4.0, 120.0, 232.0),
        panel,
        1.0,
        hsla(0.0, 0.0, 1.0, 0.3),
        0.0,
    ));
    scene.insert_primitive(bordered_quad(
        bounds(130.25, 6.5, 185.5, 228.75),
        hsla(0.3, 0.4, 0.8, 1.0),
        2.0,
        hsla(0.0, 1.0, 0.4, 1.0),
        8.0,
    ));
    scene.insert_primitive(Shadow {
        order: 0,
        blur_radius: ScaledPixels(6.0),
        bounds: bounds(150.0, 20.0, 80.0, 40.0),
        corner_radii: Corners {
            top_left: ScaledPixels(4.0),
            top_right: ScaledPixels(4.0),
            bottom_right: ScaledPixels(4.0),
            bottom_left: ScaledPixels(4.0),
        },
        content_mask: ContentMask {
            bounds: bounds(0.0, 0.0, 320.0, 240.0),
        },
        color: hsla(0.0, 0.0, 0.0, 0.5),
        element_bounds: bounds(150.0, 20.0, 80.0, 40.0),
        element_corner_radii: Corners::default(),
        inset: 0,
        pad: 0,
    });
    scene.insert_primitive(quad(bounds(150.0, 20.0, 80.0, 40.0), row));

    // a list in a layer: its rows, their text and their selection share an
    // order, so they are painted by kind and then in insertion order
    scene.push_layer(bounds(8.0, 8.0, 112.0, 200.0));
    for index in 0..8 {
        let y = 10.0 + index as f32 * 24.5;
        let mask = bounds(8.0, 8.0, 112.0, 180.0);
        let color = if index % 3 == 0 { translucent } else { row };
        scene.insert_primitive(masked_quad(bounds(10.0, y, 108.0, 22.0), mask, color));
        scene.insert_primitive(mono_sprite(
            tiles.mono,
            bounds(14.0, y + 3.0, 16.0, 16.0),
            text,
            TransformationMatrix::unit(),
        ));
        scene.insert_primitive(Underline {
            order: 0,
            pad: 0,
            bounds: bounds(34.0, y + 16.0, 60.0, 4.0),
            content_mask: ContentMask { bounds: mask },
            color: hsla(0.6, 1.0, 0.4, 1.0),
            thickness: ScaledPixels(1.5),
            wavy: PaddedBool32::from(index % 2 == 0),
        });
    }
    // an opaque row painted after a translucent one in the same layer
    scene.insert_primitive(masked_quad(
        bounds(12.0, 20.0, 90.0, 30.0),
        bounds(8.0, 8.0, 112.0, 180.0),
        hsla(0.45, 0.7, 0.35, 1.0),
    ));
    scene.pop_layer();

    // rounded, bordered, dashed, masked, gradient and patterned quads
    scene.insert_primitive(bordered_quad(
        bounds(140.0, 70.0, 70.0, 50.0),
        hsla(0.75, 0.5, 0.5, 1.0),
        3.0,
        hsla(0.0, 0.0, 0.0, 0.6),
        12.0,
    ));
    scene.insert_primitive(Quad {
        border_style: BorderStyle::Dashed,
        ..bordered_quad(
            bounds(220.0, 70.0, 80.0, 50.0),
            hsla(0.15, 0.9, 0.6, 1.0),
            2.0,
            hsla(0.6, 1.0, 0.3, 1.0),
            0.0,
        )
    });
    scene.insert_primitive(masked_quad(
        bounds(135.7, 125.3, 90.0, 60.0),
        bounds(150.5, 130.25, 100.0, 40.6),
        hsla(0.05, 0.8, 0.45, 1.0),
    ));
    scene.insert_primitive(Quad {
        background: linear_gradient(
            45.0,
            linear_color_stop(hsla(0.0, 1.0, 0.5, 1.0), 0.0),
            linear_color_stop(hsla(0.6, 1.0, 0.5, 1.0), 1.0),
        ),
        ..quad(bounds(230.0, 125.0, 80.0, 40.0), white)
    });
    scene.insert_primitive(Quad {
        background: pattern_slash(hsla(0.3, 1.0, 0.3, 1.0), 2.0, 3.0),
        ..quad(bounds(230.0, 170.0, 40.0, 30.0), white)
    });
    scene.insert_primitive(Quad {
        background: checkerboard(hsla(0.8, 1.0, 0.3, 1.0), 4.0),
        ..quad(bounds(272.0, 170.0, 40.0, 30.0), white)
    });

    // an inset shadow, translucent quads over opaque ones
    scene.insert_primitive(Shadow {
        order: 0,
        blur_radius: ScaledPixels(3.0),
        bounds: bounds(145.0, 195.0, 60.0, 30.0),
        corner_radii: Corners::default(),
        content_mask: ContentMask {
            bounds: bounds(0.0, 0.0, 320.0, 240.0),
        },
        color: hsla(0.6, 1.0, 0.2, 0.8),
        element_bounds: bounds(140.0, 190.0, 70.0, 40.0),
        element_corner_radii: Corners {
            top_left: ScaledPixels(5.0),
            top_right: ScaledPixels(5.0),
            bottom_right: ScaledPixels(5.0),
            bottom_left: ScaledPixels(5.0),
        },
        inset: 1,
        pad: 0,
    });
    scene.insert_primitive(quad(bounds(160.0, 90.0, 120.0, 60.0), translucent));

    // paths: one under an opaque quad, one over it
    scene.insert_primitive(path(
        &[
            (215.0, 195.0),
            (300.0, 200.0),
            (260.0, 175.0),
            (250.0, 235.0),
        ],
        hsla(0.55, 0.9, 0.5, 1.0),
        bounds(0.0, 0.0, 320.0, 240.0),
    ));
    scene.insert_primitive(quad(bounds(240.0, 205.0, 40.0, 20.0), panel));
    scene.insert_primitive(path(
        &[(250.0, 200.0), (290.0, 210.0), (270.0, 230.0)],
        hsla(0.95, 0.9, 0.5, 0.7),
        bounds(0.0, 0.0, 320.0, 222.0),
    ));

    // sprites: rotated monochrome, subpixel and polychrome (rounded,
    // translucent, greyscale) over the content
    scene.insert_primitive(mono_sprite(
        tiles.mono,
        bounds(180.0, 30.0, 16.0, 16.0),
        hsla(0.0, 1.0, 0.5, 1.0),
        TransformationMatrix::unit()
            .translate(Point {
                x: ScaledPixels(188.0),
                y: ScaledPixels(38.0),
            })
            .rotate(Radians(0.6))
            .translate(Point {
                x: ScaledPixels(-188.0),
                y: ScaledPixels(-38.0),
            }),
    ));
    scene.insert_primitive(SubpixelSprite {
        order: 0,
        pad: 0,
        bounds: bounds(200.0, 30.0, 12.0, 12.0),
        content_mask: ContentMask {
            bounds: bounds(0.0, 0.0, 320.0, 240.0),
        },
        color: hsla(0.7, 0.8, 0.3, 1.0),
        tile: tiles.subpixel,
        transformation: TransformationMatrix::unit(),
    });
    for (index, (grayscale, opacity)) in [(false, 1.0), (true, 0.7)].into_iter().enumerate() {
        scene.insert_primitive(PolychromeSprite {
            order: 0,
            pad: 0,
            grayscale: PaddedBool32::from(grayscale),
            opacity,
            bounds: bounds(250.0 + index as f32 * 24.0, 20.0, 20.0, 20.0),
            content_mask: ContentMask {
                bounds: bounds(0.0, 0.0, 320.0, 240.0),
            },
            corner_radii: Corners {
                top_left: ScaledPixels(5.0),
                top_right: ScaledPixels(0.0),
                bottom_right: ScaledPixels(5.0),
                bottom_left: ScaledPixels(0.0),
            },
            tile: tiles.poly,
        });
    }
    // and an opaque quad over part of everything
    scene.insert_primitive(bordered_quad(
        bounds(100.0, 150.0, 60.0, 40.0),
        hsla(0.5, 0.2, 0.6, 1.0),
        1.0,
        hsla(0.0, 0.0, 0.0, 1.0),
        4.0,
    ));

    extra(&mut scene);
    scene.finish();
    scene
}

const WIDTH: i32 = 320;
const HEIGHT: i32 = 240;

/// Draws `scene` straight into a new texture.
fn render(
    renderer: &mut WgpuHeadlessRenderer,
    scene: &Scene,
    options: FrameOptions,
) -> anyhow::Result<image::RgbaImage> {
    let frame_size = size(WIDTH, HEIGHT);
    let texture = target_texture(renderer, frame_size, true);
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    renderer.core.render_frame(
        scene,
        FrameTarget::View(&view),
        frame_size,
        false,
        wgpu::Color::BLACK,
        options,
    )?;
    let image = read_texture(&renderer.core, &texture)?;
    renderer.check_gpu_errors()?;
    Ok(image)
}

fn target_texture(
    renderer: &WgpuHeadlessRenderer,
    frame_size: Size<DevicePixels>,
    copy_dst: bool,
) -> wgpu::Texture {
    let mut usage = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC;
    usage.set(wgpu::TextureUsages::COPY_DST, copy_dst);
    renderer
        .core
        .resources
        .device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("test_target"),
            size: wgpu::Extent3d {
                width: frame_size.width.0 as u32,
                height: frame_size.height.0 as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: renderer.core.target_format,
            usage,
            view_formats: &[],
        })
}

/// Draws `scene` through the retained frame (as a window does with the
/// damage scissor) into `presented`, and returns what the retained frame
/// planned to draw again.
fn render_retained(
    renderer: &mut WgpuHeadlessRenderer,
    scene: &Scene,
    options: FrameOptions,
    presented: &wgpu::Texture,
    clear_color: wgpu::Color,
) -> anyhow::Result<(DamagePlan, image::RgbaImage)> {
    let frame_size = size(presented.width() as i32, presented.height() as i32);
    let plan = planned(renderer, scene, frame_size, clear_color, options);
    renderer.core.render_frame(
        scene,
        FrameTarget::Retained(presented),
        frame_size,
        false,
        clear_color,
        options,
    )?;
    let image = read_texture(&renderer.core, presented)?;
    renderer.check_gpu_errors()?;
    Ok((plan, image))
}

/// What `plan_damage` will decide for `scene`.
fn planned(
    renderer: &WgpuHeadlessRenderer,
    scene: &Scene,
    frame_size: Size<DevicePixels>,
    clear_color: wgpu::Color,
    options: FrameOptions,
) -> DamagePlan {
    let core = &renderer.core;
    let key = DamageKey {
        size: frame_size,
        clear_color,
        premultiplied_alpha: false,
        is_bgr: core.is_bgr,
        atlas_version: core.atlas.version(),
        opaque_depth_pass: options.opaque_depth_pass,
    };
    match core
        .resources
        .damage
        .as_ref()
        .and_then(|damage| damage.shown.as_ref())
    {
        Some(shown) if shown.key == key => shown.plan(scene, frame_size),
        _ => DamagePlan::Full,
    }
}

/// The largest difference of a channel, and how many pixels differ.
fn difference(a: &image::RgbaImage, b: &image::RgbaImage) -> (u8, usize) {
    assert_eq!(a.dimensions(), b.dimensions());
    let mut largest = 0;
    let mut pixels = 0;
    for (a, b) in a.pixels().zip(b.pixels()) {
        let channel =
            a.0.iter()
                .zip(b.0)
                .map(|(a, b)| a.abs_diff(b))
                .max()
                .unwrap_or(0);
        largest = largest.max(channel);
        pixels += usize::from(channel > 0);
    }
    (largest, pixels)
}

fn assert_same(what: &str, expected: &image::RgbaImage, actual: &image::RgbaImage) {
    let (largest, pixels) = difference(expected, actual);
    println!("{what}: {pixels} pixels differ, by at most {largest}/255");
    assert!(
        largest <= 1,
        "{what}: {pixels} pixels differ, by up to {largest}/255"
    );
}

#[test]
fn opaque_depth_pass_draws_the_same_image() -> anyhow::Result<()> {
    let mut renderer = renderer()?;
    println!(
        "adapter: {:?} ({:?}), depth format {:?}, subpixel sprites: {}",
        renderer.core.adapter_info.name,
        renderer.core.adapter_info.backend,
        renderer.core.depth_format,
        renderer.core.dual_source_blending,
    );
    let tiles = tiles(&renderer)?;
    let scene = kitchen_sink(&tiles, |_| {});
    let mut depth = DepthScratch::default();
    assert!(
        depth.fill(&scene, &scene.quads),
        "the scene has opaque quads"
    );
    println!(
        "{} quads, {} of them opaque, {} path batches",
        scene.quads.len(),
        depth.opaque_quads.len(),
        depth.path_batches.len()
    );

    let expected = render(&mut renderer, &scene, OFF)?;
    let again = render(&mut renderer, &scene, OFF)?;
    assert_same("without, twice", &expected, &again);
    let with_depth = render(&mut renderer, &scene, DEPTH)?;
    assert_same("opaque depth pass", &expected, &with_depth);
    // and back, with the depth set of pipelines still built
    let after = render(&mut renderer, &scene, OFF)?;
    assert_same("without, after", &expected, &after);
    Ok(())
}

#[test]
fn opaque_depth_pass_handles_each_kind_alone() -> anyhow::Result<()> {
    let mut renderer = renderer()?;
    let tiles = tiles(&renderer)?;
    let full = kitchen_sink(&tiles, |_| {});
    let background = quad(bounds(0.0, 0.0, 320.0, 240.0), hsla(0.0, 0.0, 1.0, 1.0));
    let cover = quad(bounds(40.0, 40.0, 200.0, 120.0), hsla(0.6, 0.5, 0.4, 1.0));
    // each kind of the kitchen sink between a background and a cover
    let kinds: [(&str, Box<dyn Fn(&mut Scene)>); 7] = [
        (
            "shadows",
            Box::new(|scene: &mut Scene| {
                full.shadows.iter().for_each(|p| scene.insert_primitive(*p))
            }),
        ),
        (
            "quads",
            Box::new(|scene: &mut Scene| {
                full.quads.iter().for_each(|p| scene.insert_primitive(*p))
            }),
        ),
        (
            "paths",
            Box::new(|scene: &mut Scene| {
                full.paths
                    .iter()
                    .for_each(|p| scene.insert_primitive(p.clone()))
            }),
        ),
        (
            "underlines",
            Box::new(|scene: &mut Scene| {
                full.underlines
                    .iter()
                    .for_each(|p| scene.insert_primitive(*p))
            }),
        ),
        (
            "monochrome sprites",
            Box::new(|scene: &mut Scene| {
                full.monochrome_sprites
                    .iter()
                    .for_each(|p| scene.insert_primitive(*p))
            }),
        ),
        (
            "subpixel sprites",
            Box::new(|scene: &mut Scene| {
                full.subpixel_sprites
                    .iter()
                    .for_each(|p| scene.insert_primitive(*p))
            }),
        ),
        (
            "polychrome sprites",
            Box::new(|scene: &mut Scene| {
                full.polychrome_sprites
                    .iter()
                    .for_each(|p| scene.insert_primitive(*p))
            }),
        ),
    ];
    for (name, insert) in &kinds {
        let mut scene = Scene::default();
        scene.insert_primitive(background);
        insert(&mut scene);
        scene.insert_primitive(cover);
        insert(&mut scene);
        scene.finish();
        let expected = render(&mut renderer, &scene, OFF)?;
        let actual = render(&mut renderer, &scene, DEPTH)?;
        assert_same(name, &expected, &actual);
    }
    Ok(())
}

#[test]
fn opaque_depth_pass_with_many_overlapping_layers() -> anyhow::Result<()> {
    let mut renderer = renderer()?;
    let mut scene = Scene::default();
    // thousands of overlapping quads, opaque and not, at fractional places
    for index in 0..3000 {
        let x = (index * 37 % 290) as f32 + 0.3 * (index % 4) as f32;
        let y = (index * 53 % 210) as f32 + 0.45 * (index % 3) as f32;
        let alpha = if index % 4 == 0 { 0.5 } else { 1.0 };
        let color = hsla((index % 17) as f32 / 17.0, 0.7, 0.5, alpha);
        let quad = if index % 5 == 0 {
            bordered_quad(
                bounds(x, y, 31.5, 27.25),
                color,
                1.5,
                hsla(0.0, 0.0, 0.0, 0.5),
                3.0,
            )
        } else {
            quad(bounds(x, y, 30.0, 25.5), color)
        };
        scene.insert_primitive(quad);
    }
    scene.finish();
    let expected = render(&mut renderer, &scene, OFF)?;
    let actual = render(&mut renderer, &scene, DEPTH)?;
    assert_same("3000 quads", &expected, &actual);
    Ok(())
}

#[test]
fn damage_scissor_redraws_only_what_changed() -> anyhow::Result<()> {
    let mut renderer = renderer()?;
    let tiles = tiles(&renderer)?;
    let frame_size = size(WIDTH, HEIGHT);
    let caret = quad(bounds(60.0, 34.0, 1.5, 16.0), hsla(0.0, 0.0, 0.0, 1.0));
    let first = kitchen_sink(&tiles, |_| {});
    // frames that each differ from the first in one small place
    let changes: Vec<(&str, Scene)> = vec![
        (
            "a caret",
            kitchen_sink(&tiles, |scene| scene.insert_primitive(caret)),
        ),
        (
            "a hovered row",
            kitchen_sink(&tiles, |scene| {
                scene.insert_primitive(masked_quad(
                    bounds(10.0, 59.0, 108.0, 22.0),
                    bounds(8.0, 8.0, 112.0, 180.0),
                    hsla(0.6, 1.0, 0.5, 0.25),
                ))
            }),
        ),
        (
            "a moved sprite",
            kitchen_sink(&tiles, |scene| {
                if let Some(sprite) = scene.polychrome_sprites.last_mut() {
                    sprite.bounds.origin.x = ScaledPixels(255.0);
                }
            }),
        ),
        (
            "a changed colour",
            kitchen_sink(&tiles, |scene| {
                if let Some(quad) = scene.quads.get_mut(3) {
                    quad.background = hsla(0.33, 1.0, 0.5, 1.0).into();
                }
            }),
        ),
        (
            "a path",
            kitchen_sink(&tiles, |scene| {
                scene.insert_primitive(path(
                    &[(20.0, 200.0), (60.0, 210.0), (40.0, 230.0)],
                    hsla(0.1, 1.0, 0.5, 1.0),
                    bounds(0.0, 0.0, 320.0, 240.0),
                ))
            }),
        ),
    ];

    for (options, copy_dst) in [(DAMAGE, true), (DAMAGE, false), (BOTH, true)] {
        for (name, changed) in &changes {
            let what = format!(
                "{name} (depth {}, {})",
                options.opaque_depth_pass,
                if copy_dst { "copied" } else { "blitted" }
            );
            let presented = target_texture(&renderer, frame_size, copy_dst);
            renderer.core.resources.damage = None;
            let (plan, image) = render_retained(
                &mut renderer,
                &first,
                options,
                &presented,
                wgpu::Color::BLACK,
            )?;
            assert_eq!(plan, DamagePlan::Full, "{what}: the first frame");
            assert_same(
                &format!("{what}: first frame"),
                &render(&mut renderer, &first, OFF)?,
                &image,
            );

            let (plan, image) = render_retained(
                &mut renderer,
                changed,
                options,
                &presented,
                wgpu::Color::BLACK,
            )?;
            let DamagePlan::Partial([_, _, width, height]) = plan else {
                panic!("{what}: expected a partial redraw, planned {plan:?}");
            };
            println!("{what}: redrew {width}×{height} of {WIDTH}×{HEIGHT}");
            let expected = render(&mut renderer, changed, OFF)?;
            assert_same(&what, &expected, &image);

            // nothing changed: nothing drawn, the same image shown
            let (plan, image) = render_retained(
                &mut renderer,
                changed,
                options,
                &presented,
                wgpu::Color::BLACK,
            )?;
            assert_eq!(plan, DamagePlan::Unchanged, "{what}: the same frame again");
            assert_same(&format!("{what}: unchanged"), &expected, &image);

            // and back to the first
            let (plan, image) = render_retained(
                &mut renderer,
                &first,
                options,
                &presented,
                wgpu::Color::BLACK,
            )?;
            assert!(
                matches!(plan, DamagePlan::Partial(_)),
                "{what}: back, planned {plan:?}"
            );
            assert_same(
                &format!("{what}: back"),
                &render(&mut renderer, &first, OFF)?,
                &image,
            );
        }
    }
    Ok(())
}

#[test]
fn damage_scissor_draws_whole_frames_when_it_must() -> anyhow::Result<()> {
    let mut renderer = renderer()?;
    let tiles = tiles(&renderer)?;
    let scene = kitchen_sink(&tiles, |_| {});
    let presented = target_texture(&renderer, size(WIDTH, HEIGHT), true);
    let transparent = wgpu::Color::TRANSPARENT;

    let (plan, _) = render_retained(&mut renderer, &scene, DAMAGE, &presented, transparent)?;
    assert_eq!(plan, DamagePlan::Full);
    // another clear colour
    let (plan, image) = render_retained(
        &mut renderer,
        &scene,
        DAMAGE,
        &presented,
        wgpu::Color::BLACK,
    )?;
    assert_eq!(plan, DamagePlan::Full, "another clear colour");
    assert_same(
        "another clear colour",
        &render(&mut renderer, &scene, OFF)?,
        &image,
    );
    // the atlas changed
    let _ = insert_tile(
        &renderer.core.atlas,
        AtlasKey::Image(RenderImageParams {
            image_id: ImageId(99),
            frame_index: 0,
        }),
        8,
        4,
    )?;
    let (plan, _) = render_retained(
        &mut renderer,
        &scene,
        DAMAGE,
        &presented,
        wgpu::Color::BLACK,
    )?;
    assert_eq!(plan, DamagePlan::Full, "the atlas changed");
    // most of the frame changed
    let covered = kitchen_sink(&tiles, |scene| {
        scene.insert_primitive(quad(
            bounds(0.0, 0.0, 300.0, 230.0),
            hsla(0.0, 0.5, 0.5, 0.5),
        ))
    });
    let (plan, image) = render_retained(
        &mut renderer,
        &covered,
        DAMAGE,
        &presented,
        wgpu::Color::BLACK,
    )?;
    assert_eq!(plan, DamagePlan::Full, "most of the frame changed");
    assert_same(
        "most of the frame",
        &render(&mut renderer, &covered, OFF)?,
        &image,
    );
    // another size
    let larger = target_texture(&renderer, size(WIDTH + 8, HEIGHT), true);
    let (plan, _) = render_retained(&mut renderer, &covered, DAMAGE, &larger, wgpu::Color::BLACK)?;
    assert_eq!(plan, DamagePlan::Full, "another size");
    // the clear colour of a window, with partial redraws
    let (plan, _) = render_retained(&mut renderer, &scene, DAMAGE, &presented, transparent)?;
    assert_eq!(plan, DamagePlan::Full);
    let caret = kitchen_sink(&tiles, |scene| {
        scene.insert_primitive(quad(
            bounds(318.0, 238.0, 4.0, 4.0),
            hsla(0.0, 1.0, 0.5, 0.5),
        ))
    });
    let (plan, image) = render_retained(&mut renderer, &caret, DAMAGE, &presented, transparent)?;
    assert!(matches!(plan, DamagePlan::Partial(_)), "planned {plan:?}");
    let expected = {
        let texture = target_texture(&renderer, size(WIDTH, HEIGHT), true);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        renderer.core.render_frame(
            &caret,
            FrameTarget::View(&view),
            size(WIDTH, HEIGHT),
            false,
            transparent,
            OFF,
        )?;
        read_texture(&renderer.core, &texture)?
    };
    assert_same(
        "a caret in the corner, cleared transparent",
        &expected,
        &image,
    );
    Ok(())
}

#[test]
fn damage_plan_compares_paint_order() {
    let snapshot = |scene: &Scene| {
        let mut snapshot = DamageSnapshot::default();
        snapshot.fill(scene, DamageKey::default());
        snapshot
    };
    let frame = size(100, 100);
    let a = quad(bounds(10.0, 10.0, 20.0, 20.0), hsla(0.0, 1.0, 0.5, 0.5));
    let b = quad(bounds(15.0, 15.0, 20.0, 20.0), hsla(0.5, 1.0, 0.5, 0.5));
    let c = quad(bounds(70.0, 70.0, 10.0, 10.0), hsla(0.3, 1.0, 0.5, 1.0));
    let layered = |first: Quad, second: Quad| {
        let mut scene = Scene::default();
        scene.insert_primitive(c);
        scene.push_layer(bounds(0.0, 0.0, 50.0, 50.0));
        scene.insert_primitive(first);
        scene.insert_primitive(second);
        scene.pop_layer();
        scene.finish();
        scene
    };
    let ab = layered(a, b);
    let ba = layered(b, a);
    assert_eq!(snapshot(&ab).plan(&ab, frame), DamagePlan::Unchanged);
    // the same quads in another order, in one layer (one order): damaged
    // where they are, not where the third is
    let DamagePlan::Partial([x, y, width, height]) = snapshot(&ab).plan(&ba, frame) else {
        panic!("a swap must be damage");
    };
    assert!(x <= 10 && y <= 10 && x + width >= 35 && y + height >= 35);
    assert!(
        x + width <= 40 && y + height <= 40,
        "{x} {y} {width} {height}"
    );
}

#[test]
fn opaque_interior_is_whole_pixels_inside() {
    let opaque = hsla(0.0, 1.0, 0.5, 1.0);
    let interior = |quad: &Quad| opaque_interior(quad).map(|bounds| rect_of(&bounds));
    assert_eq!(
        interior(&quad(bounds(1.5, 2.25, 10.0, 10.0), opaque)),
        Some([2.0, 3.0, 11.0, 12.0])
    );
    assert_eq!(
        interior(&quad(
            bounds(0.0, 0.0, 10.0, 10.0),
            hsla(0.0, 1.0, 0.5, 0.99)
        )),
        None
    );
    // inside the border and the corners, plus a pixel
    assert_eq!(
        interior(&bordered_quad(
            bounds(0.0, 0.0, 40.0, 40.0),
            opaque,
            2.0,
            opaque,
            6.0
        )),
        Some([7.0, 7.0, 33.0, 33.0])
    );
    assert_eq!(
        interior(&masked_quad(
            bounds(0.0, 0.0, 40.0, 40.0),
            bounds(10.5, 0.0, 10.0, 5.0),
            opaque
        )),
        Some([11.0, 0.0, 20.0, 5.0])
    );
    assert_eq!(interior(&quad(bounds(0.0, 0.0, 0.8, 10.0), opaque)), None);
    let mut negative = quad(bounds(0.0, 0.0, 40.0, 40.0), opaque);
    negative.border_widths.left = ScaledPixels(-1.0);
    assert_eq!(interior(&negative), None);
}

/// The bookkeeping's cost on the CPU, for a scene the size of a busy
/// window: run with `--nocapture` to see it.
#[test]
fn bookkeeping_cost() {
    let mut scene = Scene::default();
    let tile = AtlasTile {
        texture_id: AtlasTextureId {
            index: 0,
            kind: gpui::AtlasTextureKind::Monochrome,
        },
        tile_id: gpui::TileId(1),
        padding: 0,
        bounds: Bounds::default(),
    };
    for row in 0..200 {
        let y = row as f32 * 12.0;
        scene.insert_primitive(quad(bounds(0.0, y, 1080.0, 12.0), hsla(0.1, 0.2, 0.9, 1.0)));
        for column in 0..8 {
            scene.insert_primitive(quad(
                bounds(column as f32 * 130.0, y + 1.0, 120.0, 10.0),
                hsla(0.6, 0.3, 0.5, if column % 2 == 0 { 1.0 } else { 0.4 }),
            ));
        }
        for glyph in 0..40 {
            scene.insert_primitive(mono_sprite(
                tile,
                bounds(glyph as f32 * 9.0, y + 2.0, 8.0, 9.0),
                hsla(0.0, 0.0, 0.1, 1.0),
                TransformationMatrix::unit(),
            ));
        }
    }
    scene.finish();
    let mut changed = Scene::default();
    changed.replay(0..scene.len(), &scene);
    changed.insert_primitive(quad(
        bounds(500.0, 600.0, 2.0, 14.0),
        hsla(0.0, 0.0, 0.0, 1.0),
    ));
    changed.finish();
    println!(
        "{} quads, {} sprites",
        scene.quads.len(),
        scene.monochrome_sprites.len()
    );

    let rounds = 50;
    let mut depth = DepthScratch::default();
    let started = Instant::now();
    for _ in 0..rounds {
        depth.fill(&scene, &scene.quads);
    }
    println!(
        "opaque depth pass: {:.0} µs a frame for the depths and {} opaque quads",
        started.elapsed().as_secs_f64() * 1e6 / rounds as f64,
        depth.opaque_quads.len()
    );

    let mut snapshot = DamageSnapshot::default();
    snapshot.fill(&scene, DamageKey::default());
    let started = Instant::now();
    let mut plan = DamagePlan::Full;
    for _ in 0..rounds {
        plan = snapshot.plan(&changed, size(1080, 2400));
    }
    let planning = started.elapsed().as_secs_f64() * 1e6 / rounds as f64;
    let started = Instant::now();
    for _ in 0..rounds {
        snapshot.plan(&scene, size(1080, 2400));
    }
    let unchanged = started.elapsed().as_secs_f64() * 1e6 / rounds as f64;
    let started = Instant::now();
    for _ in 0..rounds {
        snapshot.fill(&scene, DamageKey::default());
    }
    let filling = started.elapsed().as_secs_f64() * 1e6 / rounds as f64;
    println!(
        "damage scissor: {planning:.0} µs to compare a changed frame ({plan:?}), \
         {unchanged:.0} µs an unchanged one, {filling:.0} µs to keep a frame"
    );
    assert!(matches!(plan, DamagePlan::Partial(_)));
}

/// Wall time of whole frames on this machine's GPU at a phone's size, with
/// each switch: run with `--ignored --nocapture`.
#[test]
#[ignore]
fn frame_cost_at_phone_size() -> anyhow::Result<()> {
    let mut renderer = renderer()?;
    let frame_size = size(1080, 2400);
    // a window, two panels, rows of a list and translucent selections: each
    // pixel under four to six quads
    let layers = std::env::var("CORVENE_TEST_LAYERS")
        .ok()
        .and_then(|layers| layers.parse().ok())
        .unwrap_or(12);
    let build = |caret: bool| {
        let mut scene = Scene::default();
        scene.insert_primitive(quad(
            bounds(0.0, 0.0, 1080.0, 2400.0),
            hsla(0.0, 0.0, 1.0, 1.0),
        ));
        scene.insert_primitive(quad(
            bounds(0.0, 100.0, 1080.0, 2300.0),
            hsla(0.6, 0.1, 0.95, 1.0),
        ));
        scene.insert_primitive(bordered_quad(
            bounds(10.0, 110.0, 1060.0, 2280.0),
            hsla(0.6, 0.1, 0.9, 1.0),
            1.0,
            hsla(0.0, 0.0, 0.0, 0.2),
            6.0,
        ));
        // nested panels, as deep as a dialog over a split view
        for depth in 0..layers {
            let inset = 20.0 + depth as f32 * 6.0;
            scene.insert_primitive(quad(
                bounds(
                    inset,
                    100.0 + inset,
                    1080.0 - 2.0 * inset,
                    2300.0 - 2.0 * inset,
                ),
                hsla(0.6, 0.1, 0.9 - depth as f32 * 0.02, 1.0),
            ));
        }
        for row in 0..60 {
            let y = 120.0 + row as f32 * 38.0;
            scene.insert_primitive(quad(
                bounds(20.0, y, 1040.0, 36.0),
                hsla(0.1, 0.1, 0.97, 1.0),
            ));
            scene.insert_primitive(quad(
                bounds(20.0, y, 1040.0, 36.0),
                hsla(0.6, 1.0, 0.5, 0.08),
            ));
        }
        if caret {
            scene.insert_primitive(quad(
                bounds(300.0, 500.0, 2.0, 30.0),
                hsla(0.0, 0.0, 0.0, 1.0),
            ));
        }
        scene.finish();
        scene
    };
    let mut scenes = [build(false), build(true)];
    let time = |renderer: &mut WgpuHeadlessRenderer,
                options: FrameOptions,
                retained: bool,
                scenes: &[Scene; 2]|
     -> anyhow::Result<f64> {
        let presented = target_texture(renderer, frame_size, true);
        let view = presented.create_view(&wgpu::TextureViewDescriptor::default());
        let frames = 40;
        let mut total = 0.0;
        for frame in 0..frames + 2 {
            let scene = &scenes[frame % 2];
            let started = Instant::now();
            let target = if retained {
                FrameTarget::Retained(&presented)
            } else {
                FrameTarget::View(&view)
            };
            let submission = renderer.core.render_frame(
                scene,
                target,
                frame_size,
                false,
                wgpu::Color::BLACK,
                options,
            )?;
            renderer
                .core
                .resources
                .device
                .poll(wgpu::PollType::Wait {
                    submission_index: Some(submission),
                    timeout: Some(std::time::Duration::from_secs(10)),
                })
                .map_err(|error| anyhow::anyhow!("{error}"))?;
            if frame >= 2 {
                total += started.elapsed().as_secs_f64();
            }
        }
        renderer.check_gpu_errors()?;
        Ok(total * 1e3 / frames as f64)
    };
    let plain = time(&mut renderer, OFF, false, &scenes)?;
    let depth = time(&mut renderer, DEPTH, false, &scenes)?;
    renderer.core.resources.damage = None;
    let damage = time(&mut renderer, DAMAGE, true, &scenes)?;
    renderer.core.resources.damage = None;
    let both = time(&mut renderer, BOTH, true, &scenes)?;
    // the same frame each time: only the copy
    renderer.core.resources.damage = None;
    scenes[1] = build(false);
    let copy = time(&mut renderer, DAMAGE, true, &scenes)?;
    let empty = {
        let mut scene = Scene::default();
        scene.finish();
        let scenes = [scene, Scene::default()];
        time(&mut renderer, OFF, false, &scenes)?
    };
    println!(
        "1080×2400, {layers} nested panels, a caret blinking, ms a frame (CPU + GPU, waited \
         for): plain {plain:.2}, opaque depth pass {depth:.2}, damage scissor {damage:.2}, \
         both {both:.2}; unchanged frames (the copy alone) {copy:.2}, an empty scene {empty:.2}"
    );
    Ok(())
}

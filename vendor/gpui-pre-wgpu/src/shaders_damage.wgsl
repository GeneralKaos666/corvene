// Corvene patch: the two helpers of the damage scissor (`set_damage_scissor`),
// which redraws only the part of a frame that changed into a texture that
// keeps the previous frame, and then shows that texture.

// A triangle that covers the whole target (the scissor rectangle limits it).
@vertex
fn vs_fullscreen(@builtin(vertex_index) vertex_id: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32((vertex_id << 1u) & 2u), f32(vertex_id & 2u));
    return vec4<f32>(corner * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
}

// Fills the damaged rectangle with the frame's clear colour: the pipeline
// blends with the blend constant (set to that colour) as the source factor
// and zero as the destination's, so what is written is the constant itself.
@fragment
fn fs_clear() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0);
}

@group(0) @binding(0) var t_retained: texture_2d<f32>;

// Copies the retained frame texel for texel, for targets that cannot be the
// destination of a texture copy.
@fragment
fn fs_blit(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    return textureLoad(t_retained, vec2<i32>(position.xy), 0);
}

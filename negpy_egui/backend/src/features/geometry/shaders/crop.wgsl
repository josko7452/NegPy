// Crop: copy a window of the rotated frame at its own size.

struct CropUniforms {
    offset: vec2<u32>,
};

@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var output_tex: texture_storage_2d<rgba32float, write>;
@group(0) @binding(2) var<uniform> params: CropUniforms;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = textureDimensions(output_tex);
    if (gid.x >= dims.x || gid.y >= dims.y) {
        return;
    }
    let coords = vec2<i32>(i32(gid.x), i32(gid.y));
    let source = vec2<i32>(i32(params.offset.x) + coords.x, i32(params.offset.y) + coords.y);
    textureStore(output_tex, coords, textureLoad(input_tex, source, 0));
}

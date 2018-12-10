#include <metal_stdlib>

using namespace metal;

typedef struct {
    float2 position;
} vertex_t;

typedef struct
{
    float2 viewport;
} uniforms_t;

struct VertexOutFragmentIn {
    float4 position [[position]];
};

// vertex shader function
vertex VertexOutFragmentIn vs(device vertex_t* vertex_array [[ buffer(0) ]],
                                   unsigned int vid [[ vertex_id ]])
{
    VertexOutFragmentIn out;

    out.position = float4(float2(vertex_array[vid].position), 0.0, 1.0);

    return out;
}

// fragment shader function
fragment float4 ps(VertexOutFragmentIn in [[stage_in]],
                         device uniforms_t* uniforms [[ buffer(1) ]])
{
    float2 viewport = uniforms->viewport;
    return float4(in.position.x / viewport.x, 1.0-in.position.y / viewport.y, 0.0, 1.0);
};

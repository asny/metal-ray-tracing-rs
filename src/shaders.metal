#include <metal_stdlib>

using namespace metal;

struct VertexOutFragmentIn {
    float4 position [[position]];
    float2 coords;
};

constant constexpr static const float4 fullscreenTrianglePositions[3]
{
    {-1.0, -1.0, 0.0, 1.0},
    { 3.0, -1.0, 0.0, 1.0},
    {-1.0,  3.0, 0.0, 1.0}
};

// vertex shader function
vertex VertexOutFragmentIn vs(unsigned int vid [[ vertex_id ]])
{
    VertexOutFragmentIn out;

    out.position = fullscreenTrianglePositions[vid];
    out.coords = out.position.xy * 0.5 + 0.5;

    return out;
}

// fragment shader function
fragment float4 ps(VertexOutFragmentIn in [[stage_in]])
{
    return float4(in.coords, 0.0, 1.0);
};

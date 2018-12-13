
#include <metal_stdlib>

using namespace metal;

constant float EPSILON = 0.000001;
constant uint NOISE_BLOCK_SIZE = 128;

struct Ray {
    packed_float3 origin;
    float minDistance;
    packed_float3 direction;
    float maxDistance;
};

struct Intersection {
    float distance;
    uint primitiveIndex;
    float2 coordinates;
};

enum MaterialType
{
    Diffuse = 0,
    Metal  = 1,
    Dielectric = 2
};

struct Material
{
    packed_float3 diffuse;
};

struct Triangle
{
    uint materialIndex;
};

kernel void generateRays(device Ray* rays [[buffer(0)]],
                         device packed_float4* noise [[buffer(1)]],
                         uint2 coordinates [[thread_position_in_grid]],
                         uint2 size [[threads_per_grid]])
{
    const float3 origin = float3(0.0f, 1.0f, 2.1f);

    uint noiseSampleIndex = (coordinates.x % NOISE_BLOCK_SIZE) +
        NOISE_BLOCK_SIZE * (coordinates.y % NOISE_BLOCK_SIZE);

    device const packed_float4& noiseSample = noise[noiseSampleIndex];
    float2 rnd = (noiseSample.xy * 2.0 - 1.0) / float2(size - 1);

    float aspect = float(size.x) / float(size.y);
    float2 uv = float2(coordinates) / float2(size - 1) * 2.0f - 1.0f;

    float3 direction = float3(aspect * (uv.x + rnd.x), (uv.y + rnd.y), -1.0f);

    uint rayIndex = coordinates.x + coordinates.y * size.x;
    rays[rayIndex].origin = origin;
    rays[rayIndex].direction = normalize(direction);
    rays[rayIndex].minDistance = EPSILON;
    rays[rayIndex].maxDistance = INFINITY;
}

kernel void handleIntersections(texture2d<float, access::write> image [[texture(0)]],
                                device const Intersection* intersections [[buffer(0)]],
                                device const Material* materials [[buffer(1)]],
                                device const Triangle* triangles [[buffer(2)]],
                                uint2 coordinates [[thread_position_in_grid]],
                                uint2 size [[threads_per_grid]])
{
    uint rayIndex = coordinates.x + coordinates.y * size.x;
    device const Intersection& intersection = intersections[rayIndex];
    if (intersection.distance < EPSILON)
        return;

    device const Triangle& triangle = triangles[intersection.primitiveIndex];
    device const Material& material = materials[triangle.materialIndex];
    image.write(float4(material.diffuse, 1.0), coordinates);

    //image.write(float4(intersection.coordinates, 1.0 - intersection.coordinates.x - intersection.coordinates.y, 1.0), coordinates);
    //image.write(float4(float3(triangle.materialIndex/7.0), 1.0), coordinates);
}

#include <metal_stdlib>

using namespace metal;

constant float PI = 3.1415926535897932384626433832795;
constant float EPSILON = 0.000001;
constant uint NOISE_BLOCK_SIZE = 128;

struct Ray {
    packed_float3 origin;
    float minDistance;
    packed_float3 direction;
    float maxDistance;
    packed_float3 color;
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

struct EmitterTriangle
{
    uint primitiveIndex;
    packed_float3 emissive;
    float area;
};

struct ApplicationData
{
    uint frameIndex;
    uint emitterTrianglesCount;
    float emitterTotalArea;
};

device const EmitterTriangle& sampleEmitterTriangle(device const EmitterTriangle* triangles, uint triangleCount, float totalArea, float xi)
{
    float cfd = 0.0;
    for (uint index = 0; index < triangleCount-1; index++)
    {
        float pdf = triangles[index].area / totalArea;
        cfd += pdf;
        if (xi < cfd)
        {
            return triangles[index];
        }
    }
    return triangles[triangleCount-1];
}

float3 barycentric(float2 smp)
{
    float r1 = sqrt(smp.x);
    float r2 = smp.y;
    return float3(1.0f - r1, r1 * (1.0f - r2), r1 * r2);
}

kernel void generateRays(device Ray* rays [[buffer(0)]],
                         device const packed_float3* noise [[buffer(1)]],
                         uint2 coordinates [[thread_position_in_grid]],
                         uint2 size [[threads_per_grid]])
{
    const float3 origin = float3(0.0f, 1.0f, 2.1f);

    uint noiseSampleIndex = (coordinates.x % NOISE_BLOCK_SIZE) + NOISE_BLOCK_SIZE * (coordinates.y % NOISE_BLOCK_SIZE);

    device const packed_float3& noiseSample = noise[noiseSampleIndex];
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

kernel void handleIntersections(device const Intersection* intersections [[buffer(0)]],
                                device const Material* materials [[buffer(1)]],
                                device const Triangle* triangles [[buffer(2)]],
                                device Ray* rays [[buffer(3)]],
                                device const packed_float3* vertices [[buffer(4)]],
                                device const packed_uint3* indices [[buffer(5)]],
                                device const EmitterTriangle* emitterTriangles [[buffer(6)]],
                                device const ApplicationData& appData [[buffer(7)]],
                                device const packed_float3* noise [[buffer(8)]],
                                uint2 coordinates [[thread_position_in_grid]],
                                uint2 size [[threads_per_grid]])
{
    uint rayIndex = coordinates.x + coordinates.y * size.x;
    device const Intersection& intersection = intersections[rayIndex];
    if (intersection.distance < EPSILON)
        return;

    device const Triangle& triangle = triangles[intersection.primitiveIndex];
    device const Material& material = materials[triangle.materialIndex];

    // Find intersection point
    device const packed_uint3& triangleIndices = indices[intersection.primitiveIndex];
    device const packed_float3& a = vertices[triangleIndices.x];
    device const packed_float3& b = vertices[triangleIndices.y];
    device const packed_float3& c = vertices[triangleIndices.z];
    float3 intersection_point = intersection.coordinates.x * a + intersection.coordinates.y * b + (1.0 - intersection.coordinates.x - intersection.coordinates.y) * c;

    // Find normal
    float3 normal = normalize(cross(b-a, c-a));

    // Sample light
    uint noiseSampleIndex = (coordinates.x % NOISE_BLOCK_SIZE) + NOISE_BLOCK_SIZE * (coordinates.y % NOISE_BLOCK_SIZE);
    device const packed_float3& noiseSample = noise[noiseSampleIndex];
    device const EmitterTriangle& emitterTriangle = sampleEmitterTriangle(emitterTriangles, appData.emitterTrianglesCount, appData.emitterTotalArea, noiseSample.x);

    // Light attributes
    float3 lightTriangleBarycentric = barycentric(noiseSample.yz);
    device const packed_uint3& lightTriangleIndices = indices[emitterTriangle.primitiveIndex];
    device const packed_float3& d = vertices[lightTriangleIndices.x];
    device const packed_float3& e = vertices[lightTriangleIndices.y];
    device const packed_float3& f = vertices[lightTriangleIndices.z];
    float3 light_position = lightTriangleBarycentric.x * d + lightTriangleBarycentric.y * e + lightTriangleBarycentric.z * f;
    float3 light_normal = normalize(cross(e-d, f-d));
    float light_pdf = emitterTriangle.area / appData.emitterTotalArea;
    float3 light_dir = light_position - intersection_point;
    float light_dist = length(light_dir);
    light_dir /= light_dist;

    // Find color
    float materialBsdf = (1.0 / PI) * dot(light_dir, normal);
    float cosTheta = -dot(light_dir, light_normal);
    float pointSamplePdf = (light_dist * light_dist) / (emitterTriangle.area * cosTheta);
    float lightSamplePdf = light_pdf * pointSamplePdf;
    rays[rayIndex].color = emitterTriangle.emissive * material.diffuse * (materialBsdf / lightSamplePdf);

    // Set shadow ray
    rays[rayIndex].origin = intersection_point;
    rays[rayIndex].direction = light_dir;
    rays[rayIndex].minDistance = EPSILON;
    rays[rayIndex].maxDistance = light_dist - EPSILON;
}

kernel void handleShadows(device Ray* rays [[buffer(0)]],
                         device const Intersection* intersections [[buffer(1)]],
                         uint2 coordinates [[thread_position_in_grid]],
                         uint2 size [[threads_per_grid]])
{
    uint rayIndex = coordinates.x + coordinates.y * size.x;

    float intersectionDistance = intersections[rayIndex].distance;

    if (rays[rayIndex].maxDistance < 0.0f || intersectionDistance >= 0.0f) {
        rays[rayIndex].color = float3(0.0);
    }
}

kernel void accumulateImage(
    texture2d<float, access::read_write> image [[texture(0)]],
    device Ray* rays [[buffer(0)]],
    device const ApplicationData& appData [[buffer(1)]],
    uint2 coordinates [[thread_position_in_grid]],
    uint2 size [[threads_per_grid]])
{
    uint rayIndex = coordinates.x + coordinates.y * size.x;
    float4 outputColor = float4(rays[rayIndex].color, 1.0);
    if (appData.frameIndex > 0)
    {
        float4 storedColor = image.read(coordinates);
        float t = float(appData.frameIndex) / float(appData.frameIndex + 1);
        outputColor = mix(outputColor, storedColor, t);
    }
    image.write(outputColor, coordinates);
}
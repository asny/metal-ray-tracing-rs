
#include <metal_stdlib>

using namespace metal;

constant float EPSILON = 0.000001;
constant uint NOISE_BLOCK_SIZE = 16;

struct Ray {
    packed_float3 origin;
    float minDistance;
    packed_float3 direction;
    float maxDistance;
    packed_float3 color;
    uint surfacePrimitiveIndex;
    uint emitterPrimitiveIndex;
    packed_float3 throughput;
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
    packed_float3 emissive;
};

struct Triangle
{
    uint materialIndex;
    packed_float3 normal;
};

struct EmitterTriangle
{
    uint primitiveIndex;
    float area;
    float pdf;
};

struct ApplicationData
{
    uint frameIndex;
    uint bounceIndex;
    uint emitterTrianglesCount;
};

uint sampleEmitterTriangle(device const EmitterTriangle* triangles, uint triangleCount, float xi)
{
    float cfd = 0.0;
    for (uint index = 0; index < triangleCount-1; index++)
    {
        cfd += triangles[index].pdf;
        if (xi < cfd)
        {
            return index;
        }
    }
    return triangleCount-1;
}

void buildOrthonormalBasis(float3 n, thread float3& u, thread float3& v)
{
    float s = (n.z < 0.0 ? -1.0f : 1.0f);
    float a = -1.0f / (s + n.z);
    float b = n.x * n.y * a;
    u = float3(1.0f + s * n.x * n.x * a, s * b, -s * n.x);
    v = float3(b, s + n.y * n.y * a, -n.y);
}

float3 barycentric(float2 smp)
{
    float r1 = sqrt(smp.x);
    float r2 = smp.y;
    return float3(1.0f - r1, r1 * (1.0f - r2), r1 * r2);
}

float3 alignToDirection(float3 n, float cosTheta, float phi)
{
    float3 u;
    float3 v;
    buildOrthonormalBasis(n, u, v);
    float sinTheta = sqrt(1.0f - cosTheta * cosTheta);
    return (u * cos(phi) + v * sin(phi)) * sinTheta + n * cosTheta;
}

float3 sampleCosineWeightedHemisphere(float3 n, float2 xi)
{
    float cosTheta = sqrt(xi.x);
    return alignToDirection(n, cosTheta, xi.y * 2.0 * M_PI_F);
}

float3 pointOnTriangle(device const packed_uint3* indices, device const packed_float3* vertices, uint primitiveIndex, float3 coordinates)
{
    device const packed_uint3& triangleIndices = indices[primitiveIndex];
    device const packed_float3& a = vertices[triangleIndices.x];
    device const packed_float3& b = vertices[triangleIndices.y];
    device const packed_float3& c = vertices[triangleIndices.z];
    return coordinates.x * a + coordinates.y * b + coordinates.z * c;
}

device const packed_float4& sampleNoise(device const packed_float4* noise, uint2 coordinates, uint bounceIndex)
{
    uint noiseSampleIndex = (coordinates.x % NOISE_BLOCK_SIZE)
        + NOISE_BLOCK_SIZE * (coordinates.y % NOISE_BLOCK_SIZE)
        + bounceIndex * NOISE_BLOCK_SIZE * NOISE_BLOCK_SIZE;
    return noise[noiseSampleIndex];
}

kernel void generateRays(device Ray* rays [[buffer(0)]],
                         device const packed_float4* noise [[buffer(1)]],
                         uint2 coordinates [[thread_position_in_grid]],
                         uint2 size [[threads_per_grid]])
{
    uint rayIndex = coordinates.x + coordinates.y * size.x;
    device Ray& ray = rays[rayIndex];

    const float3 origin = float3(0.0f, 1.0f, 2.1f);
    device const packed_float4& noiseSample = sampleNoise(noise, coordinates, 0);
    float2 rnd = (noiseSample.xy * 2.0 - 1.0) / float2(size - 1);

    float aspect = float(size.x) / float(size.y);
    float2 uv = float2(coordinates) / float2(size - 1) * 2.0f - 1.0f;

    float3 direction = float3(aspect * (uv.x + rnd.x), (uv.y + rnd.y), -1.0f);

    ray.origin = origin;
    ray.direction = normalize(direction);
    ray.minDistance = EPSILON;
    ray.maxDistance = INFINITY;
    ray.color = float3(0.0);
    ray.throughput = float3(1.0);
}

kernel void handleIntersections(device Ray* rays [[buffer(0)]],
                                device const Intersection* intersections [[buffer(1)]],
                                device const packed_float3* vertices [[buffer(2)]],
                                device const packed_uint3* indices [[buffer(3)]],
                                device const ApplicationData& appData [[buffer(4)]],
                                device const packed_float4* noise [[buffer(5)]],
                                device const EmitterTriangle* emitterTriangles [[buffer(6)]],
                                uint2 coordinates [[thread_position_in_grid]],
                                uint2 size [[threads_per_grid]])
{
    uint rayIndex = coordinates.x + coordinates.y * size.x;
    device const Intersection& intersection = intersections[rayIndex];
    device Ray& ray = rays[rayIndex];
    device const packed_float4& noiseSample = sampleNoise(noise, coordinates, appData.bounceIndex);

    if (intersection.distance < EPSILON) // No intersection => No surface is hit
    {
        ray.maxDistance = -1.0;
        return;
    }

    // Find intersection point
    float3 surface_position = pointOnTriangle(indices, vertices, intersection.primitiveIndex, float3(intersection.coordinates, 1.0 - intersection.coordinates.x - intersection.coordinates.y));

    // Sample light
    uint emitterPrimitiveIndex = sampleEmitterTriangle(emitterTriangles, appData.emitterTrianglesCount, noiseSample.x);

    // Light attributes
    float3 lightTriangleBarycentric = barycentric(noiseSample.yz);
    float3 light_position = pointOnTriangle(indices, vertices, emitterTriangles[emitterPrimitiveIndex].primitiveIndex, lightTriangleBarycentric);
    float3 light_dir = light_position - surface_position;
    float light_dist = length(light_dir);
    light_dir /= light_dist;

    // Setup shadow ray
    ray.surfacePrimitiveIndex = intersection.primitiveIndex;
    ray.emitterPrimitiveIndex = emitterPrimitiveIndex;
    ray.origin = surface_position;
    ray.direction = light_dir;
    ray.minDistance = EPSILON;
    ray.maxDistance = light_dist - EPSILON;
}

kernel void handleShadows(device Ray* rays [[buffer(0)]],
                         device const Intersection* intersections [[buffer(1)]],
                         device const packed_float3* vertices [[buffer(2)]],
                         device const packed_uint3* indices [[buffer(3)]],
                         device const ApplicationData& appData [[buffer(4)]],
                         device const packed_float4* noise [[buffer(5)]],
                         device const EmitterTriangle* emitterTriangles [[buffer(6)]],
                         device const Material* materials [[buffer(7)]],
                         device const Triangle* triangles [[buffer(8)]],
                         uint2 coordinates [[thread_position_in_grid]],
                         uint2 size [[threads_per_grid]])
{
    uint rayIndex = coordinates.x + coordinates.y * size.x;
    device Ray& ray = rays[rayIndex];

    if (ray.maxDistance < 0.0f) // Previously no surface is hit
    {
        return;
    }

    device const Intersection& intersection = intersections[rayIndex];
    device const packed_float4& noiseSample = sampleNoise(noise, coordinates, appData.bounceIndex);
    device const Triangle& surface_triangle = triangles[ray.surfacePrimitiveIndex];
    device const Material& surface_material = materials[surface_triangle.materialIndex];
    float3 surface_normal = surface_triangle.normal;

    // Handle the case where the ray hit an emitter
    if (length_squared(surface_material.emissive) > EPSILON)
    {
        ray.color += surface_material.emissive * ray.throughput;
    }

    ray.throughput *= surface_material.diffuse;

    // Calculate color contribution
    if (intersection.distance < 0.0f) // No intersection => Nothing blocking the light
    {
        // light
        device const EmitterTriangle& emitter_triangle = emitterTriangles[ray.emitterPrimitiveIndex];
        device const Triangle& light_triangle = triangles[emitter_triangle.primitiveIndex];
        device const Material& light_material = materials[light_triangle.materialIndex];

        float light_area = emitter_triangle.area;
        float light_pdf = emitter_triangle.pdf;
        float3 light_normal = light_triangle.normal;
        float3 light_dir = ray.direction;
        float light_dist = ray.maxDistance + EPSILON;

        float materialBsdf = (1.0 / M_PI_F) * dot(light_dir, surface_normal);
        float cosTheta = -dot(light_dir, light_normal);
        float pointSamplePdf = (light_dist * light_dist) / (light_area * cosTheta);
        float lightSamplePdf = light_pdf * pointSamplePdf;

        ray.color += light_material.emissive * ray.throughput * (materialBsdf / lightSamplePdf);
    }

    // Setup next ray bounce
    ray.direction = sampleCosineWeightedHemisphere(surface_normal, noiseSample.wx);
    ray.minDistance = EPSILON;
    ray.maxDistance = INFINITY;
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
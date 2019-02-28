
#include <metal_stdlib>

using namespace metal;

constant float EPSILON = 0.000001;
constant uint NOISE_BLOCK_SIZE = 64;

struct Ray {
    packed_float3 origin;
    float minDistance;
    packed_float3 direction;
    float maxDistance;
    packed_float3 color;
    uint surfacePrimitiveIndex;
    uint emitterPrimitiveIndex;
    packed_float3 throughput;
    uint hitBackOfTriangle;
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

struct Camera
{
    packed_float3 position;
    packed_float3 direction;
    packed_float3 up;
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

float3 buildOrthonormalBasis(float3 n, float3 sample)
{
    /*float s = (n.z < 0.0 ? -1.0f : 1.0f);
    float a = -1.0f / (s + n.z);
    float b = n.x * n.y * a;
    u = float3(1.0f + s * n.x * n.x * a, s * b, -s * n.x);
    v = float3(b, s + n.y * n.y * a, -n.y);*/

    float3 x = normalize(n);
    float3 temp = normalize(float3(1.0, 1.0, -1.0));
    float3 z = normalize(cross(x, temp));
    float3 y = normalize(cross(z, x));


    return normalize(x * sample.x + y * sample.y + z * sample.z);
}

float3 barycentric(float2 smp)
{
    float r1 = sqrt(smp.x);
    float r2 = smp.y;
    return float3(1.0f - r1, r1 * (1.0f - r2), r1 * r2);
}

/*float3 alignToDirection(float3 n, float cosTheta, float phi)
{
    float3 u;
    float3 v;
    buildOrthonormalBasis(n, u, v);
    float sinTheta = sqrt(1.0f - cosTheta * cosTheta);
    return (u * cos(phi) + v * sin(phi)) * sinTheta + n * cosTheta;
}*/

/*float3 sampleCosineWeightedHemisphere(float3 n, float2 xi)
{
    float cosTheta = sqrt(xi.x);
    return alignToDirection(n, cosTheta, xi.y * 2.0 * M_PI_F);
}*/

float3 sampleCosineWeightedHemisphere(float2 u) {
    //return float3(1.0, 0.0, 0.0);
    float phi = 2.0f * M_PI_F * u.x;

    float cos_phi = cos(phi);
    float sin_phi = sin(phi);

    float theta = M_PI_2_F * u.y;
    float cos_theta = cos(theta);
    float sin_theta = sin(theta);
    //float cos_theta = sqrt(u.y);
    //float sin_theta = sqrt(1.0f - cos_theta * cos_theta);

    if(phi > 2.0f * M_PI_F || phi < 0.0 || theta > 0.5 * M_PI_F || theta < 0.0)
    {
        //return float3(0.0, 0.0, 0.0);
    }

    return float3(sin_theta, cos_phi * cos_theta, sin_phi * cos_theta);


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
                         device const Camera& camera [[buffer(2)]],
                         uint2 coordinates [[thread_position_in_grid]],
                         uint2 size [[threads_per_grid]])
{
    uint rayIndex = coordinates.x + coordinates.y * size.x;
    device Ray& ray = rays[rayIndex];
    device const packed_float4& noiseSample = sampleNoise(noise, coordinates, 0);

    float2 rnd = (noiseSample.xy * 2.0 - 1.0) / float2(size - 1);

    float aspect = float(size.x) / float(size.y);
    float2 uv = float2(coordinates) / float2(size - 1) * 2.0f - 1.0f;

    float3 eye = camera.position;;
    float3 view_dir = camera.direction;
    float3 up_dir = camera.up;
    float3 right_dir = normalize(cross(view_dir, up_dir));

    float3 direction = normalize(aspect * (uv.x + rnd.x) * right_dir + (uv.y + rnd.y) * up_dir + view_dir);

    ray.origin = eye;
    ray.minDistance = EPSILON;
    ray.direction = direction;
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
                                device const Material* materials [[buffer(7)]],
                                device const Triangle* triangles [[buffer(8)]],
                                uint2 coordinates [[thread_position_in_grid]],
                                uint2 size [[threads_per_grid]])
{
    uint rayIndex = coordinates.x + coordinates.y * size.x;
    device const Intersection& intersection = intersections[rayIndex];
    device Ray& ray = rays[rayIndex];

    if (ray.maxDistance < 0.0f || intersection.distance < EPSILON) // No intersection => No surface is hit
    {
        ray.maxDistance = -1.0;
        return;
    }

    // Hit surface attributes
    device const Triangle& surface_triangle = triangles[intersection.primitiveIndex];
    device const Material& surface_material = materials[surface_triangle.materialIndex];
    float3 surface_normal = float3(surface_triangle.normal);
    float3 surface_position = pointOnTriangle(indices, vertices, intersection.primitiveIndex, float3(intersection.coordinates, 1.0 - intersection.coordinates.x - intersection.coordinates.y));
    //ray.color = float3((surface_position));

    // Handle the case where the ray hit an emitter
    bool hitBackOfTriangle = dot(float3(ray.direction), surface_normal) > EPSILON;
    //ray.color = 0.5 * surface_normal + float3(0.5);
    if (length_squared(surface_material.emissive) > EPSILON && !hitBackOfTriangle)
    {
        ray.color += ray.origin;//surface_material.emissive * ray.throughput;
        ray.maxDistance = -1.0;
    }
    else {

    ray.throughput *= surface_material.diffuse;

    // Sample light
    device const packed_float4& noiseSample = sampleNoise(noise, coordinates, appData.bounceIndex);
    uint emitterPrimitiveIndex = sampleEmitterTriangle(emitterTriangles, appData.emitterTrianglesCount, noiseSample.x);

    // Light attributes
    float3 lightTriangleBarycentric = barycentric(noiseSample.yz);
    float3 light_position = pointOnTriangle(indices, vertices, emitterTriangles[emitterPrimitiveIndex].primitiveIndex, lightTriangleBarycentric);
    float3 light_dir = light_position - surface_position;
    float light_dist = length(light_dir);
    light_dir /= light_dist;

    // Setup shadow ray
    ray.origin = packed_float3(surface_position);
    ray.minDistance = EPSILON;
    ray.direction = packed_float3(light_dir);
    ray.maxDistance = light_dist - EPSILON;

    ray.hitBackOfTriangle = hitBackOfTriangle ? 1 : 0;
    ray.surfacePrimitiveIndex = intersection.primitiveIndex;
    ray.emitterPrimitiveIndex = emitterPrimitiveIndex;
    }
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

    device const Triangle& surface_triangle = triangles[ray.surfacePrimitiveIndex];
    float3 surface_normal = normalize(ray.hitBackOfTriangle ? - float3(surface_triangle.normal) : float3(surface_triangle.normal));
    device const Intersection& intersection = intersections[rayIndex];

    // Calculate shadow ray color contribution
    if (intersection.distance < 0.0f) // No intersection between the shadow ray and the geometry => Nothing blocking the light
    {
        // light
        device const EmitterTriangle& emitter_triangle = emitterTriangles[ray.emitterPrimitiveIndex];
        device const Triangle& light_triangle = triangles[emitter_triangle.primitiveIndex];
        device const Material& light_material = materials[light_triangle.materialIndex];

        float light_area = emitter_triangle.area;
        float light_pdf = emitter_triangle.pdf;
        float3 light_normal = float3(light_triangle.normal);
        float3 light_dir = ray.direction;
        float light_dist = ray.maxDistance + EPSILON;

        float cosTheta = -dot(light_dir, light_normal);
        bool hitBackOfLightTriangle = cosTheta < EPSILON;
        bool shadowRayGoesThroughSurfaceTriangle = dot(surface_normal, light_dir) < EPSILON;
        if(!hitBackOfLightTriangle && !shadowRayGoesThroughSurfaceTriangle)
        {
            float materialBsdf = (1.0 / M_PI_F) * dot(light_dir, surface_normal);
            float pointSamplePdf = (light_dist * light_dist) / (light_area * cosTheta);
            float lightSamplePdf = light_pdf * pointSamplePdf;
            //ray.color += light_material.emissive * ray.throughput * (materialBsdf / lightSamplePdf);
        }
    }

    // Setup next ray bounce
    device const packed_float4& noiseSample = sampleNoise(noise, coordinates, appData.bounceIndex);

    float3 ray_sample = sampleCosineWeightedHemisphere(noiseSample.wx);

    //float3 ray_sample = normalize(noiseSample.xyz * 2.0 - 1.0);
    //ray.color = float3(ray.origin.x, ray.origin.y, ray.origin.z);

    ray.origin = packed_float3(float3(0.0, 1.0, 0.0));
    ray.minDistance = EPSILON;
    ray.direction = surface_normal;//buildOrthonormalBasis(surface_normal, ray_sample);
    ray.maxDistance = INFINITY;

    /*ray.color = float3(0.0);
    ray.throughput = float3(1.0);
    ray.hitBackOfTriangle = 0;
    ray.surfacePrimitiveIndex = 0;
    ray.emitterPrimitiveIndex = 0;*/
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
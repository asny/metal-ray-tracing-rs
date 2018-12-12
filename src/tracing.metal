//
//  base-ray-tracing.metal
//  Metal ray-tracer
//
//  Created by Sergey Reznik on 9/15/18.
//  Copyright © 2018 Serhii Rieznik. All rights reserved.
//

#include <metal_stdlib>

using namespace metal;

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

kernel void generateRays(device Ray* rays [[buffer(0)]],
                         uint2 coordinates [[thread_position_in_grid]],
                         uint2 size [[threads_per_grid]])
{
    uint rayIndex = coordinates.x + coordinates.y * size.x;
    float2 uv = float2(coordinates) / float2(size - 1);
    rays[rayIndex].origin = packed_float3(1.0 + 5.0*uv.x - 3.0, 2.5 + 5.0*uv.y - 3.0, 5.0);
    rays[rayIndex].direction = normalize(packed_float3(-0.1, -0.5, -1.0));
    rays[rayIndex].minDistance = 0.0f;
    rays[rayIndex].maxDistance = 10.0f;
}

kernel void handleIntersections(texture2d<float, access::write> image [[texture(0)]],
                                device const Intersection* intersections [[buffer(0)]],
                                uint2 coordinates [[thread_position_in_grid]],
                                uint2 size [[threads_per_grid]])
{
    uint rayIndex = coordinates.x + coordinates.y * size.x;
    device const Intersection& i = intersections[rayIndex];
    if (i.distance > 0.0f)
    {
        float w = 1.0 - i.coordinates.x - i.coordinates.y;
        image.write(float4(i.coordinates, w, 1.0), coordinates);
    }
}
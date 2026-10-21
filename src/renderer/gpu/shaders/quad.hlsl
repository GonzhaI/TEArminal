// Quads instanciados del renderer GPU (Shader Model 4.0: vale desde Feature Level 10_0).
// Una instancia = un rectángulo en píxeles del destino. Reproduce las primitivas del rasterizador
// CPU de referencia (src/renderer/raster_cpu.rs); el diseño de `Instance` está en pipeline.rs.
// Tras editar este archivo: `pwsh tools/compile-shaders.ps1` (regenera los .cso y el hash).

cbuffer Frame : register(b0)
{
    float2 target_size; // píxeles del destino
    float2 _pad;
};

Texture2D<float>  atlas : register(t0); // R8: cobertura de glifos
Texture2D<float4> image : register(t1); // BGRA: imagen (el muestreo devuelve RGBA)

static const uint KIND_SOLID = 0;
static const uint KIND_ROUNDED_FILL = 1;
static const uint KIND_ROUNDED_STROKE = 2;
static const uint KIND_GLYPH = 3;
static const uint KIND_IMAGE = 4;
static const uint KIND_CARD_CORNER = 5;

struct VsIn
{
    int4 rect : RECT;     // x, y, w, h
    uint4 clip : CLIP;    // x0, y0, x1, y1 (límite superior exclusivo)
    uint color : COLOR;   // 0xAARRGGBB
    uint2 uv : UV;        // origen en el atlas (Glyph)
    uint kr : KINDRADIUS; // bits 0..3 tipo, 8..15 radio, 16..23 extra (esquina de CardCorner)
    uint vid : SV_VertexID;
};

struct VsOut
{
    float4 pos : SV_Position;
    nointerpolation int4 rect : TEXCOORD0;
    nointerpolation uint4 clip : TEXCOORD1;
    nointerpolation uint color : TEXCOORD2;
    nointerpolation uint2 uv : TEXCOORD3;
    nointerpolation uint kr : TEXCOORD4;
};

VsOut vs_main(VsIn i)
{
    // Tira de triángulos: 0=(0,0) 1=(1,0) 2=(0,1) 3=(1,1)
    int2 corner = int2(i.vid & 1, i.vid >> 1);
    int2 p0 = i.rect.xy;
    int2 p1 = i.rect.xy + i.rect.zw;
    // El quad se recorta al `clip` (menos píxeles que rellenar); el PS recorta igualmente
    int2 c0 = int2(i.clip.xy);
    int2 c1 = int2(i.clip.zw);
    int2 a = max(p0, c0);
    int2 b = max(min(p1, c1), a);
    float2 p = float2(lerp(a, b, corner));
    VsOut o;
    o.pos = float4(p.x / target_size.x * 2.0 - 1.0, 1.0 - p.y / target_size.y * 2.0, 0.0, 1.0);
    o.rect = i.rect;
    o.clip = i.clip;
    o.color = i.color;
    o.uv = i.uv;
    o.kr = i.kr;
    return o;
}

// Distancia con signo al rectángulo redondeado (ui/paint.rs: draw_rounded_rect_clipped).
float rounded_dist(float2 pc, int4 rect, float radius)
{
    float2 half_size = float2(rect.zw) * 0.5;
    float2 c = float2(rect.xy) + half_size;
    float r = min(radius, min(half_size.x, half_size.y));
    float2 q = abs(pc - c) - (half_size - r);
    return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;
}

float4 ps_main(VsOut i) : SV_Target
{
    int2 pix = int2(floor(i.pos.xy));
    if (uint(pix.x) < i.clip.x || uint(pix.y) < i.clip.y || uint(pix.x) >= i.clip.z || uint(pix.y) >= i.clip.w)
        discard;

    uint kind = i.kr & 15u;
    uint radius = (i.kr >> 8) & 255u;
    float3 rgb = float3((i.color >> 16) & 255u, (i.color >> 8) & 255u, i.color & 255u) / 255.0;
    uint ca = (i.color >> 24) & 255u;
    float2 pc = float2(pix) + 0.5; // centro del píxel, como el rasterizador CPU
    int2 off = pix - i.rect.xy;

    if (kind == KIND_SOLID)
    {
        // fill_rect escribe XRGB opaco: se ignora el alfa del color
        return float4(rgb, 1.0);
    }
    else if (kind == KIND_ROUNDED_FILL || kind == KIND_ROUNDED_STROKE)
    {
        float dist = rounded_dist(pc, i.rect, float(radius));
        // Relleno: cobertura lineal de 1 px; contorno de 1 px alineado hacia el interior
        float d = (kind == KIND_ROUNDED_FILL) ? dist : abs(dist + 0.5) - 0.5;
        float a8 = floor(saturate(0.5 - d) * float(ca) + 0.5); // round() de la CPU
        if (a8 < 1.0)
            discard;
        return float4(rgb, a8 / 255.0);
    }
    else if (kind == KIND_GLYPH)
    {
        float cov = atlas.Load(int3(int2(i.uv) + off, 0));
        uint c8 = uint(cov * 255.0 + 0.5);
        if (c8 == 0u)
            discard; // blit_glyph: cobertura 0 no toca el destino
        // blit_glyph ignora el alfa del color: solo manda la cobertura (255 sobrescribe, el resto mezcla)
        return float4(rgb, float(c8) / 255.0);
    }
    else if (kind == KIND_IMAGE)
    {
        uint iw, ih;
        image.GetDimensions(iw, ih);
        // Vecino más próximo con la misma aritmética entera que la CPU: src = off * src_size / target_size
        uint2 src = (uint2(off) * uint2(iw, ih)) / uint2(i.rect.zw);
        float4 t = image.Load(int3(int2(src), 0));
        uint a8 = uint(t.a * 255.0 + 0.5);
        if (a8 == 0u)
            discard;
        return float4(t.rgb, float(a8) / 255.0);
    }
    else
    {
        // CardCorner (ui/paint.rs: frame_card): `color` es `outer`; cuarto de círculo de radio `radius`
        uint corner = (i.kr >> 16) & 3u;
        float2 centre = float2(i.rect.xy) + float2((corner & 1u) != 0u ? 0.0 : float(radius),
                                                   (corner & 2u) != 0u ? 0.0 : float(radius));
        float outside = saturate(length(pc - centre) - float(radius) + 0.5);
        float a8 = floor(outside * 255.0); // `as u8` trunca
        if (a8 < 1.0)
            discard;
        return float4(rgb, a8 / 255.0);
    }
}

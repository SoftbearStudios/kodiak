attribute vec2 position;
attribute vec2 uv;
attribute float alpha;

uniform mat3 uView;

varying vec2 vPosition;
varying vec2 vUv;
varying float vAlpha;

void main() {
    vPosition = position;
    vUv = uv;
    vAlpha = alpha;
    gl_Position = vec4(uView * vec3(position.xy, 1.0), 1.0);
}

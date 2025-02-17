attribute vec2 position;
uniform mat4 uModelViewProjection;
varying vec2 vUv;

void main() {
    gl_Position = uModelViewProjection * vec4(position, 0.0, 1.0);
    vUv = vec2(position.x + 0.5, 0.5 - position.y);
}

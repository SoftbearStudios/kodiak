attribute vec2 position;
uniform mat4 uMatrix;
varying vec3 vUv;

void main() {
    // Set z to 1.
    gl_Position = vec4(position, 1.0, 1.0);
    vUv = (uMatrix * vec4(position, 0.0, 1.0)).xyz;
}

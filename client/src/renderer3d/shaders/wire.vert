attribute vec4 position;
attribute vec3 color;
uniform mat4 uViewProjection;
varying vec3 vColor;

void main() {
    gl_Position = uViewProjection * position;
    gl_Position.z -= 0.0001;
    vColor = color;
}
